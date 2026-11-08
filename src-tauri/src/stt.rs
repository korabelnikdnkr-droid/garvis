// src-tauri/src/stt.rs
//! Модуль распознавания речи (STT) на базе Vosk FFI.
//! Обеспечивает 100% оффлайн детекцию ключевого слова «Джарвис» (Wake Word)
//! и потоковую транскрипцию голосовых команд с детекцией пауз (VAD).

use crate::audio::TARGET_SAMPLE_RATE;
use crate::error::{JarvisError, JarvisResult};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::Receiver;
use tracing::{debug, error, info, warn};
use vosk::{Model, Recognizer};

/// Имя ключевого слова для пробуждения ассистента
const WAKE_WORD_PRIMARY: &str = "джарвис";
const WAKE_WORD_SECONDARY: &str = "jarvis";

/// Таймаут тишины (в секундах) после начала команды для фиксации завершения фразы
const SILENCE_TIMEOUT_SECS: f32 = 1.3;

/// Максимальная длительность ожидания команды после активации (в секундах)
const MAX_LISTENING_DURATION_SECS: f32 = 10.0;

/// Состояния внутренней машины распознавания
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecognizerState {
    /// Ожидание кодового слова «Джарвис» (UI: idling)
    WaitingForWakeWord,
    /// Прослушивание и накопление команды (UI: listening)
    ListeningForCommand,
    /// Завершение команды и передача на обработку (UI: thinking)
    Processing,
}

/// JSON-структура промежуточного распознавания Vosk
#[derive(Deserialize, Debug)]
struct VoskPartialResult {
    partial: String,
}

/// JSON-структура финального распознавания Vosk
#[derive(Deserialize, Debug)]
struct VoskFinalResult {
    text: String,
}

/// Процессор распознавания речи Vosk
pub struct VoskEngine {
    app_handle: AppHandle,
    model_path: PathBuf,
    is_active: Arc<AtomicBool>,
}

impl VoskEngine {
    /// Создание процессора с автоматическим поиском русской модели Vosk
    pub fn new(app_handle: AppHandle) -> Self {
        let model_path = Self::find_russian_model_path();
        Self {
            app_handle,
            model_path,
            is_active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Поиск директории с русской моделью Vosk в типичных каталогах
    fn find_russian_model_path() -> PathBuf {
        let candidates = [
            PathBuf::from("models/vosk-model-small-ru"),
            PathBuf::from("models/vosk-model-ru"),
            PathBuf::from("src-tauri/models/vosk-model-small-ru"),
            PathBuf::from("../models/vosk-model-small-ru"),
            PathBuf::from("models"),
        ];

        for path in &candidates {
            if path.exists() && path.is_dir() {
                info!("Найдена модель Vosk по пути: {:?}", path);
                return path.clone();
            }
        }

        // По умолчанию используем models/vosk-model-small-ru
        PathBuf::from("models/vosk-model-small-ru")
    }

    /// Запуск обработчика входящего PCM-потока в выделенном системном потоке
    pub fn start_processing(&self, mut pcm_rx: Receiver<Vec<i16>>) {
        if self.is_active.swap(true, Ordering::SeqCst) {
            warn!("Движок Vosk уже активен");
            return;
        }

        let app = self.app_handle.clone();
        let model_path = self.model_path.clone();
        let is_active = self.is_active.clone();

        // Vosk выполняет синхронные вычисления на CPU, поэтому запускаем отдельный рабочий поток
        std::thread::spawn(move || {
            info!("Запуск рабочего потока распознавания Vosk STT...");

            // 1. Проверка наличия модели на диске
            if !model_path.exists() {
                warn!(
                    "Каталог модели Vosk '{:?}' не найден. Для оффлайн-распознавания загрузите vosk-model-small-ru-0.22 в папку models/.",
                    model_path
                );
                let _ = app.emit(
                    "jarvis-stt-status",
                    serde_json::json!({
                        "ready": false,
                        "error": "Модель Vosk отсутствует в каталоге models/. Ожидание загрузки..."
                    }),
                );
                // Поток ожидает данные, но не падает при отсутствии модели
            }

            // 2. Инициализация модели Vosk с безопасной обработкой
            let model = match Model::new(model_path.to_str().unwrap_or("")) {
                Some(m) => {
                    info!("Русская модель Vosk успешно загружена в память");
                    let _ = app.emit("jarvis-stt-status", serde_json::json!({ "ready": true }));
                    Some(m)
                }
                None => {
                    warn!("Не удалось загрузить модель Vosk. Режим голосового пробуждения переведен в режим ожидания.");
                    None
                }
            };

            // 3. Создание инстанса распознавателя на 16 кГц
            let mut recognizer = model.as_ref().and_then(|m| Recognizer::new(m, TARGET_SAMPLE_RATE as f32));

            // Состояние машины распознавания
            let mut state = RecognizerState::WaitingForWakeWord;
            let mut speech_detected_time: Option<Instant> = None;
            let mut listening_start_time: Option<Instant> = None;
            let mut accumulated_command = String::new();

            // 4. Потоковый цикл чтения сэмплов из буфера
            while is_active.load(Ordering::SeqCst) {
                // Блокирующее чтение из асинхронного канала через tokio::runtime::Handle или blocking_recv
                let pcm_chunk = match pcm_rx.blocking_recv() {
                    Some(chunk) => chunk,
                    None => {
                        debug!("Канал передачи PCM закрыт, остановка STT потока");
                        break;
                    }
                };

                let rec = match recognizer.as_mut() {
                    Some(r) => r,
                    None => continue, // Пропускаем обработку при отсутствии модели
                };

                // Передача блока сэмплов в движок Vosk
                let is_final = rec.accept_waveform(&pcm_chunk);

                match state {
                    // -----------------------------------------------------------------
                    // Фаза A: Ожидание кодового слова «Джарвис» (WaitingForWakeWord)
                    // -----------------------------------------------------------------
                    RecognizerState::WaitingForWakeWord => {
                        let text_to_check = if is_final {
                            let json_str = rec.result().to_string();
                            serde_json::from_str::<VoskFinalResult>(&json_str)
                                .map(|r| r.text.to_lowercase())
                                .unwrap_or_default()
                        } else {
                            let json_str = rec.partial_result().to_string();
                            serde_json::from_str::<VoskPartialResult>(&json_str)
                                .map(|r| r.partial.to_lowercase())
                                .unwrap_or_default()
                        };

                        if Self::contains_wake_word(&text_to_check) {
                            info!("Обнаружено кодовое слово пробуждения: «ДЖАРВИС»!");

                            // Переводим машину в режим прослушивания команды
                            state = RecognizerState::ListeningForCommand;
                            listening_start_time = Some(Instant::now());
                            speech_detected_time = None;
                            accumulated_command.clear();

                            // Оповещаем фронтенд о переходе в статус "Listening"
                            let _ = app.emit("app://state", "listening");
                            let _ = app.emit("jarvis-state-changed", "listening");
                            let _ = app.emit("jarvis-wake-detected", true);

                            // Сброс буфера распознавателя для чистой транскрипции команды
                            rec.reset();
                        }
                    }

                    // -----------------------------------------------------------------
                    // Фаза B: Прием и транскрипция команды с детекцией пауз (VAD)
                    // -----------------------------------------------------------------
                    RecognizerState::ListeningForCommand => {
                        let now = Instant::now();

                        // Проверка превышения максимального времени ожидания
                        if let Some(start) = listening_start_time {
                            if now.duration_since(start).as_secs_f32() > MAX_LISTENING_DURATION_SECS {
                                warn!("Таймаут ожидания команды после пробуждения");
                                state = Self::finalize_command(
                                    &app,
                                    &accumulated_command,
                                    rec,
                                );
                                continue;
                            }
                        }

                        if is_final {
                            let json_str = rec.result().to_string();
                            if let Ok(res) = serde_json::from_str::<VoskFinalResult>(&json_str) {
                                let phrase = res.text.trim();
                                if !phrase.is_empty() {
                                    if !accumulated_command.is_empty() {
                                        accumulated_command.push(' ');
                                    }
                                    accumulated_command.push_str(phrase);
                                    speech_detected_time = Some(now);
                                    debug!("Финальный сегмент речи: «{}»", phrase);
                                }
                            }
                        } else {
                            let json_str = rec.partial_result().to_string();
                            if let Ok(part) = serde_json::from_str::<VoskPartialResult>(&json_str) {
                                let partial_text = part.partial.trim();
                                if !partial_text.is_empty() {
                                    speech_detected_time = Some(now);
                                    // Отправка промежуточного текста для живого отображения в UI
                                    let _ = app.emit("jarvis-partial-transcript", partial_text);
                                }
                            }
                        }

                        // Проверка детектора тишины (VAD silence timeout)
                        if let Some(last_speech) = speech_detected_time {
                            let silence_elapsed = now.duration_since(last_speech).as_secs_f32();
                            if silence_elapsed >= SILENCE_TIMEOUT_SECS && !accumulated_command.is_empty() {
                                info!(
                                    "Зафиксирована пауза речи ({:.1}с). Финализация команды...",
                                    silence_elapsed
                                );
                                state = Self::finalize_command(&app, &accumulated_command, rec);
                            }
                        }
                    }

                    // -----------------------------------------------------------------
                    // Фаза C: Обработка (Processing)
                    // -----------------------------------------------------------------
                    RecognizerState::Processing => {
                        // Состояние удерживается до ответа LLM и TTS (управляется через внешние события)
                    }
                }
            }

            info!("Поток Vosk STT успешно завершил работу");
        });
    }

    /// Проверка строки на наличие ключевого слова «Джарвис»
    fn contains_wake_word(text: &str) -> bool {
        let normalized = text.to_lowercase();
        normalized.contains(WAKE_WORD_PRIMARY)
            || normalized.contains(WAKE_WORD_SECONDARY)
            || normalized.contains("жарвис")
            || normalized.contains("джарвиса")
    }

    /// Финализация накопленной команды и переключение состояния в "Thinking"
    fn finalize_command(
        app: &AppHandle,
        command_text: &str,
        recognizer: &mut Recognizer,
    ) -> RecognizerState {
        let clean_command = command_text.trim();
        info!("Команда пользователя распознана: «{}»", clean_command);

        // Переводим интерфейс в состояние "Thinking"
        let _ = app.emit("app://state", "thinking");
        let _ = app.emit("jarvis-state-changed", "thinking");

        // Эмитим распознанную команду для обработки LLM-модулем (Фаза 4)
        let _ = app.emit(
            "jarvis-command-detected",
            serde_json::json!({
                "command": clean_command,
                "timestamp": chrono_now_string()
            }),
        );

        recognizer.reset();
        RecognizerState::WaitingForWakeWord
    }

    /// Остановка обработчика
    pub fn stop(&self) {
        self.is_active.store(false, Ordering::SeqCst);
        info!("Запрошена остановка потока Vosk STT");
    }
}

/// Вспомогательная функция формирования текущего времени
fn chrono_now_string() -> String {
    // Безопасное локальное время без паники
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    format!("{:02}:{:02}", hours, mins)
}
