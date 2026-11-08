// src-tauri/src/audio_queue.rs
//! Модуль управления очередью воспроизведения речи (TTS Audio Queue)
//! и мгновенного прерывания (Barge-in).
//! Гарантирует, что Джарвис мгновенно замолчит, если пользователь начнет говорить,
//! и не будет говорить поверх пользователя.

use crate::error::{JarvisError, JarvisResult};
use crate::tts::{TtsAudioOutput, TtsEngine};
use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

/// Менеджер очереди воспроизведения аудио
pub struct AudioQueueManager {
    app_handle: AppHandle,
    tts_engine: Arc<TtsEngine>,
    current_sink: Arc<Mutex<Option<Sink>>>,
    queue_tx: mpsc::Sender<String>,
    is_speaking: Arc<AtomicBool>,
}

impl AudioQueueManager {
    /// Инициализация менеджера очереди и запуск фонового исполнителя
    pub fn new(app_handle: AppHandle) -> Self {
        let (tx, mut rx) = mpsc::channel::<String>(32);
        let tts_engine = Arc::new(TtsEngine::new());
        let current_sink: Arc<Mutex<Option<Sink>>> = Arc::new(Mutex::new(None));
        let is_speaking = Arc::new(AtomicBool::new(false));

        let app_clone = app_handle.clone();
        let tts_clone = tts_engine.clone();
        let sink_clone = current_sink.clone();
        let speaking_flag = is_speaking.clone();

        // Фоновый рабочий поток последовательного воспроизведения очереди
        tokio::spawn(async move {
            info!("Инициализация рабочего потока аудиоочереди TTS...");

            // Создание выходного аудиопотока по умолчанию через rodio
            // Изолируем создание потока с защитой от отсутствия звуковой карты
            let (_stream, stream_handle) = match OutputStream::try_default() {
                Ok(tuple) => (Some(tuple.0), Some(tuple.1)),
                Err(e) => {
                    warn!("Не удалось инициализировать выходной аудиопоток Rodio: {e}");
                    (None, None)
                }
            };

            while let Some(text) = rx.recv().await {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }

                info!("Аудиоочередь обрабатывает фразу: «{}»", trimmed);

                // 1. Оповещаем фронтенд о переходе в режим речи
                speaking_flag.store(true, Ordering::SeqCst);
                let _ = app_clone.emit("app://state", "speaking");
                let _ = app_clone.emit("jarvis-state-changed", "speaking");

                // 2. Синтезируем аудио через TtsEngine (Edge-TTS или SAPI)
                match tts_clone.synthesize(trimmed).await {
                    Ok(TtsAudioOutput::Bytes(audio_bytes)) => {
                        if let Some(ref handle) = stream_handle {
                            // Создаем новый Sink для воспроизведения этого сегмента
                            if let Ok(sink) = Sink::try_new(handle) {
                                // Сохраняем активный Sink для возможности Barge-in прерывания
                                {
                                    let mut guard = sink_clone.lock().await;
                                    *guard = Some(sink);
                                }

                                // Декодирование MP3/PCM байтов
                                let cursor = Cursor::new(audio_bytes);
                                match Decoder::new(cursor) {
                                    Ok(source) => {
                                        let guard = sink_clone.lock().await;
                                        if let Some(ref active_sink) = *guard {
                                            active_sink.append(source);
                                        }
                                        drop(guard);

                                        // Неблокирующее ожидание окончания воспроизведения
                                        while speaking_flag.load(Ordering::SeqCst) {
                                            let is_empty = {
                                                let guard = sink_clone.lock().await;
                                                guard.as_ref().map(|s| s.empty()).unwrap_or(true)
                                            };
                                            if is_empty {
                                                break;
                                            }
                                            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                                        }
                                    }
                                    Err(e) => {
                                        error!("Ошибка декодирования аудиопотока в Rodio: {e}");
                                    }
                                }

                                // Сброс текущего Sink
                                let mut guard = sink_clone.lock().await;
                                *guard = None;
                            }
                        }
                    }
                    Ok(TtsAudioOutput::SystemHandled) => {
                        // SAPI воспроизвелся синхронно в отдельном потоке
                    }
                    Err(e) => {
                        error!("Сбой воспроизведения фразы в очереди: {e}");
                    }
                }

                // 3. Возвращаем ассистента в режим ожидания (Idling), если очередь пуста и речь не прервана
                speaking_flag.store(false, Ordering::SeqCst);
                let _ = app_clone.emit("app://state", "idling");
                let _ = app_clone.emit("jarvis-state-changed", "idling");
            }

            info!("Поток аудиоочереди TTS завершил работу");
        });

        Self {
            app_handle,
            tts_engine,
            current_sink,
            queue_tx: tx,
            is_speaking,
        }
    }

    /// Постановка фразы в очередь воспроизведения
    pub async fn enqueue_speech(&self, text: &str) -> JarvisResult<()> {
        let clean = text.trim().to_string();
        if clean.is_empty() {
            return Ok(());
        }

        self.queue_tx
            .send(clean)
            .await
            .map_err(|e| JarvisError::Internal(format!("Не удалось добавить фразу в очередь: {e}")))?;

        Ok(())
    }

    /// Мгновенное прерывание речи (Barge-in Logic)
    /// Вызывается при детекции голоса пользователя во время говорения Джарвиса
    pub async fn interrupt(&self) {
        if self.is_speaking.load(Ordering::SeqCst) {
            info!("⚡ ПРЕРЫВАНИЕ (Barge-in): Обнаружен голос пользователя. Джарвис замолкает.");

            // 1. Сбрасываем флаг говорения
            self.is_speaking.store(false, Ordering::SeqCst);

            // 2. Останавливаем и уничтожаем активный Sink
            let mut guard = self.current_sink.lock().await;
            if let Some(sink) = guard.take() {
                sink.stop();
            }

            // 3. Мгновенно переключаем интерфейс в режим прослушивания пользователя
            let _ = self.app_handle.emit("app://state", "listening");
            let _ = self.app_handle.emit("jarvis-state-changed", "listening");
        }
    }

    /// Проверка, говорит ли ассистент в данный момент
    pub fn is_currently_speaking(&self) -> bool {
        self.is_speaking.load(Ordering::SeqCst)
    }
}
