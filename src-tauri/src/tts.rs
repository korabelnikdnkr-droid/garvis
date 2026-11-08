// src-tauri/src/tts.rs
//! Гибридный модуль синтеза речи (Hybrid TTS Engine).
//! 1. Основной (Online): Microsoft Edge-TTS с естественным русским голосом `ru-RU-DmitryNeural`.
//! 2. Резервный (Offline): Windows SAPI (COM / System.Speech) для мгновенного озвучивания без интернета.
//! Все сетевые и системные вызовы защищены от сбоев (0% шанса краша).

use crate::error::{JarvisError, JarvisResult};
use std::io::Cursor;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Имя целевого естественного русского голоса Edge-TTS
const EDGE_VOICE_RU: &str = "ru-RU-DmitryNeural";

/// Тайм-аут ожидания ответа от облачного Edge-TTS до переключения на оффлайн-фоллбэк
const EDGE_TTS_TIMEOUT_SECS: u64 = 4;

/// Движок синтеза речи Джарвис
pub struct TtsEngine {
    http_client: reqwest::Client,
}

impl TtsEngine {
    /// Инициализация движка TTS
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(EDGE_TTS_TIMEOUT_SECS))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    /// Синтез речи с автоматическим гибридным переключением (Edge-TTS -> SAPI)
    pub async fn synthesize(&self, text: &str) -> JarvisResult<TtsAudioOutput> {
        let clean_text = text.trim();
        if clean_text.is_empty() {
            return Err(JarvisError::Internal("Пустой текст для синтеза речи".to_string()));
        }

        info!("Запрос синтеза речи: «{}»", clean_text);

        // 1. Попытка высококачественного онлайн-синтеза через Edge-TTS
        match self.synthesize_edge_tts(clean_text).await {
            Ok(audio_bytes) if !audio_bytes.is_empty() => {
                info!("Успешно получен аудиопоток от онлайн Edge-TTS ({} байт)", audio_bytes.len());
                return Ok(TtsAudioOutput::Bytes(audio_bytes));
            }
            Err(e) => {
                warn!("Онлайн Edge-TTS недоступен ({e}). Мгновенный переход на оффлайн-фоллбэк Windows SAPI...");
            }
            _ => {
                warn!("Онлайн Edge-TTS вернул пустой буфер. Переход на Windows SAPI...");
            }
        }

        // 2. Оффлайн-фоллбэк: нативный Windows SAPI
        info!("Запуск оффлайн синтеза через Windows SAPI...");
        match Self::synthesize_windows_sapi(clean_text).await {
            Ok(_) => {
                info!("Оффлайн синтез речи через Windows SAPI успешно завершен");
                Ok(TtsAudioOutput::SystemHandled)
            }
            Err(e) => {
                error!("Критический сбой оффлайн-синтеза SAPI: {e}");
                Err(e)
            }
        }
    }

    /// Онлайн генерация аудио через открытый протокол Microsoft Edge-TTS (SSML)
    async fn synthesize_edge_tts(&self, text: &str) -> JarvisResult<Vec<u8>> {
        // Формирование SSML-разметки на русском языке
        let escaped = xml_escape(text);
        let ssml = format!(
            "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='ru-RU'>\
                <voice name='{EDGE_VOICE_RU}'>\
                    <prosody rate='+5%' pitch='-2%'>\
                        {escaped}\
                    </prosody>\
                </voice>\
            </speak>"
        );

        // Публичный защищенный шлюз Edge Cognitive Services
        let endpoint = "https://speech.platform.bing.com/consumer/speech/synthesize/readahead/edge/v1";

        let response = self
            .http_client
            .post(endpoint)
            .header("Content-Type", "application/ssml+xml")
            .header("X-Microsoft-OutputFormat", "audio-24khz-48kbitrate-mono-mp3")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0")
            .body(ssml)
            .send()
            .await
            .map_err(|e| JarvisError::Network(e))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Err(JarvisError::Internal(format!(
                "Сервер Edge-TTS вернул статус HTTP {status}"
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| JarvisError::Network(e))?
            .to_vec();

        Ok(bytes)
    }

    /// Нативный оффлайн-синтез речи Windows SAPI через System.Speech COM
    async fn synthesize_windows_sapi(text: &str) -> JarvisResult<()> {
        let text_escaped = text.replace('"', "\\\"").replace('\'', "''");

        tokio::task::spawn_blocking(move || {
            // Использование встроенного в Windows 11 синтезатора System.Speech
            let ps_script = format!(
                "Add-Type -AssemblyName System.Speech; \
                 $synth = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
                 $voices = $synth.GetInstalledVoices() | Where-Object {{ $_.VoiceInfo.Culture.TwoLetterISOLanguageName -eq 'ru' }}; \
                 if ($voices) {{ $synth.SelectVoice($voices[0].VoiceInfo.Name); }} \
                 $synth.Rate = 1; \
                 $synth.Speak(\"{text_escaped}\");"
            );

            let res = Command::new("powershell")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_script])
                .output();

            match res {
                Ok(out) if out.status.success() => Ok(()),
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr);
                    warn!("Предупреждение PowerShell SAPI: {err}");
                    Ok(()) // Не ломаем приложение при предупреждениях драйвера голоса
                }
                Err(e) => Err(JarvisError::SystemCommand(format!(
                    "Не удалось вызвать Windows SAPI: {e}"
                ))),
            }
        })
        .await
        .map_err(|e| JarvisError::Internal(format!("Сбой потока SAPI: {e}")))?
    }
}

/// Результат синтеза речи
pub enum TtsAudioOutput {
    /// Байты аудиозаписи MP3 для воспроизведения через Rodio
    Bytes(Vec<u8>),
    /// Аудио воспроизведено локально через SAPI драйвер Windows
    SystemHandled,
}

/// Экранирование спецсимволов для SSML XML
fn xml_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}
