// src-tauri/src/audio.rs
//! Модуль захвата звука с микрофона в реальном времени с использованием библиотеки `cpal`.
//! Преобразует аудиопоток в моно PCM 16-бит с частотой 16000 Гц для распознавателя Vosk.
//! Обеспечивает отказоустойчивость: автоматическое переподключение при отключении устройства.

use crate::error::{JarvisError, JarvisResult};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::Sender;
use tracing::{debug, error, info, warn};

/// Целевая частота дискретизации для Vosk (16 кГц)
pub const TARGET_SAMPLE_RATE: u32 = 16000;

/// Менеджер аудиозахвата
pub struct AudioManager {
    app_handle: AppHandle,
    pcm_sender: Sender<Vec<i16>>,
    is_running: Arc<AtomicBool>,
}

impl AudioManager {
    /// Создание нового экземпляра менеджера захвата звука
    pub fn new(app_handle: AppHandle, pcm_sender: Sender<Vec<i16>>) -> Self {
        Self {
            app_handle,
            pcm_sender,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Запуск непрерывного цикла аудиозахвата в фоновой задаче Tokio
    pub fn start(&self) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            warn!("Аудиопоток уже запущен");
            return;
        }

        let app_handle = self.app_handle.clone();
        let pcm_tx = self.pcm_sender.clone();
        let is_running = self.is_running.clone();

        // Фоновый поток, не блокирующий главный цикл Tauri
        tokio::spawn(async move {
            info!("Инициализация подсистемы аудиозахвата CPAL...");

            while is_running.load(Ordering::SeqCst) {
                match Self::run_capture_session(&app_handle, &pcm_tx, &is_running).await {
                    Ok(_) => {
                        info!("Сессия аудиозахвата завершена корректно");
                    }
                    Err(e) => {
                        error!("Сбой аудиозахвата микрофона: {e}. Попытка переподключения через 2 сек...");
                        // Оповещение интерфейса о проблеме с микрофоном
                        let _ = app_handle.emit("jarvis-audio-error", format!("Микрофон недоступен: {e}"));
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }

            info!("Аудиопоток CPAL полностью остановлен");
        });
    }

    /// Остановка аудиозахвата
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        info!("Запрошена остановка аудиопотока");
    }

    /// Запуск отдельной сессии захвата для текущего устройства по умолчанию
    async fn run_capture_session(
        app_handle: &AppHandle,
        pcm_tx: &Sender<Vec<i16>>,
        is_running: &Arc<AtomicBool>,
    ) -> JarvisResult<()> {
        let host = cpal::default_host();

        // Получение микрофона по умолчанию
        let device = host
            .default_input_device()
            .ok_or_else(|| JarvisError::Audio("Микрофон по умолчанию не обнаружен в системе".to_string()))?;

        let device_name = device
            .name()
            .unwrap_or_else(|_| "Неизвестное аудиоустройство".to_string());
        info!("Выбрано устройство ввода: {device_name}");

        // Получение конфигурации по умолчанию
        let default_config = device
            .default_input_config()
            .map_err(|e| JarvisError::Audio(format!("Ошибка получения конфигурации микрофона: {e}")))?;

        let sample_rate = default_config.sample_rate().0;
        let channels = default_config.channels();
        let sample_format = default_config.sample_format();

        info!(
            "Параметры микрофона: частота={sample_rate} Гц, каналов={channels}, формат={:?}",
            sample_format
        );

        let tx = pcm_tx.clone();
        let app = app_handle.clone();
        let running_flag = is_running.clone();

        // Построение потока в зависимости от исходного формата сэмплов (F32 или I16)
        let stream = match sample_format {
            SampleFormat::F32 => Self::build_input_stream::<f32>(
                &device,
                &default_config.into(),
                channels,
                sample_rate,
                tx,
                app,
                running_flag,
            )?,
            SampleFormat::I16 => Self::build_input_stream::<i16>(
                &device,
                &default_config.into(),
                channels,
                sample_rate,
                tx,
                app,
                running_flag,
            )?,
            _ => {
                return Err(JarvisError::Audio(format!(
                    "Неподдерживаемый формат сэмплов аудиоустройства: {:?}",
                    sample_format
                )));
            }
        };

        // Старт прослушивания
        stream
            .play()
            .map_err(|e| JarvisError::Audio(format!("Не удалось активировать аудиопоток: {e}")))?;

        info!("Аудиопоток микрофона успешно активирован и передает сэмплы");

        // Удерживаем поток активным, пока работает флаг
        while is_running.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        Ok(())
    }

    /// Универсальный построитель входного аудиопотока с ресемплингом в 16 кГц Mono PCM
    fn build_input_stream<T>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        channels: u16,
        source_sample_rate: u32,
        pcm_tx: Sender<Vec<i16>>,
        app_handle: AppHandle,
        is_running: Arc<AtomicBool>,
    ) -> JarvisResult<Stream>
    where
        T: cpal::Sample + cpal::SizedSample + Into<f32>,
    {
        let channels_count = channels as usize;
        let step = (source_sample_rate as f64) / (TARGET_SAMPLE_RATE as f64);

        let error_callback = {
            let running = is_running.clone();
            move |err: cpal::StreamError| {
                error!("Критическая ошибка аудиопотока CPAL: {err}");
                running.store(false, Ordering::SeqCst);
            }
        };

        let data_callback = move |data: &[T], _: &cpal::InputCallbackInfo| {
            if !is_running.load(Ordering::SeqCst) {
                return;
            }

            // 1. Извлечение первого канала (микширование в моно) и перевод в f32
            let mut mono_samples: Vec<f32> = Vec::with_capacity(data.len() / channels_count);
            for chunk in data.chunks_exact(channels_count) {
                let sample_f32: f32 = chunk[0].into();
                mono_samples.push(sample_f32);
            }

            // 2. Расчет среднеквадратичного уровня громкости (RMS) для анимации сферы фронтенда
            let mut sum_squares = 0.0f32;
            for &s in &mono_samples {
                sum_squares += s * s;
            }
            let rms = if !mono_samples.is_empty() {
                (sum_squares / mono_samples.len() as f32).sqrt()
            } else {
                0.0
            };
            // Нормализация уровня (диапазон 0.0 - 1.0)
            let normalized_level = (rms * 4.0).min(1.0);

            // Отправка уровня громкости во фронтенд (не блокируя)
            let _ = app_handle.emit("jarvis-audio-level", serde_json::json!({ "level": normalized_level }));

            // 3. Простой линейный ресемплинг к 16000 Гц
            let resampled_len = (mono_samples.len() as f64 / step).floor() as usize;
            let mut pcm16_chunk: Vec<i16> = Vec::with_capacity(resampled_len);

            for i in 0..resampled_len {
                let src_idx = (i as f64 * step).floor() as usize;
                if src_idx < mono_samples.len() {
                    let float_val = mono_samples[src_idx].clamp(-1.0, 1.0);
                    // Перевод f32 (-1.0..1.0) в i16 (-32768..32767)
                    let pcm16_val = (float_val * 32767.0) as i16;
                    pcm16_chunk.push(pcm16_val);
                }
            }

            // 4. Передача нормализованных PCM-сэмплов в канал для STT
            if !pcm16_chunk.is_empty() {
                let _ = pcm_tx.try_send(pcm16_chunk);
            }
        };

        let stream = device
            .build_input_stream(config, data_callback, error_callback, None)
            .map_err(|e| JarvisError::Audio(format!("Не удалось сконфигурировать входной поток: {e}")))?;

        Ok(stream)
    }
}
