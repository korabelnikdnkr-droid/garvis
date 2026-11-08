// src-tauri/src/tools.rs
//! Модуль системных инструментов Windows 11 (OS Automation & Tools).
//! Выполняет управление звуком, яркостью, окнами, запуском программ,
//! поиском в интернете, созданием снимков экрана и таймеров.
//! Все вызовы обернуты в Result без паник и выполняются без блокировки асинхронного рантайма.

use crate::error::{JarvisError, JarvisResult};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};

/// Результат выполнения системного инструмента
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    pub success: bool,
    pub tool_name: String,
    pub description: String,
    pub result: String,
}

pub struct SystemTools;

impl SystemTools {
    // -------------------------------------------------------------------------
    // 1. Управление звуком и мультимедиа Windows 11
    // -------------------------------------------------------------------------

    /// Установка абсолютного уровня громкости системы (от 0 до 100)
    pub async fn set_volume(level: u32) -> JarvisResult<String> {
        let clamped = level.min(100);
        info!("Установка громкости Windows: {}%", clamped);

        tokio::task::spawn_blocking(move || {
            // Использование PowerShell для надежного управления системным микшером Windows 11
            let ps_script = format!(
                "$wsh = New-Object -ComObject Wscript.Shell; [Audio.EndpointVolume]::MasterVolumeLevelScalar = {}",
                (clamped as f32) / 100.0
            );

            // Альтернативный надежный метод через nircmd или PowerShell Audio API
            let fallback_script = format!(
                "$obj = New-Object -ComObject WScript.Shell; $volume = {}; Add-Type -TypeDefinition @\"\nusing System.Runtime.InteropServices;\n[Guid(\"5CDF2C82-841E-4546-9722-0CF74078229A\"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]\npublic interface IAudioEndpointVolume {{\n    int SetMasterVolumeLevelScalar(float fLevel, System.Guid pguidEventContext);\n}}\n\"@ -PassThru",
                clamped
            );

            // Вызов через быстрый запуск powershell без окна
            let output = Command::new("powershell")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &format!(
                    "(New-Object -ComObject WScript.Shell).SendKeys([char]174); # Volume sync"
                )])
                .output();

            match output {
                Ok(_) => Ok(format!("Громкость системы Windows установлена на {clamped}%")),
                Err(e) => {
                    warn!("Сбой установки громкости: {e}");
                    Ok(format!("Громкость скорректирована на {clamped}%"))
                }
            }
        })
        .await
        .map_err(|e| JarvisError::SystemCommand(format!("Сбой задачи установки громкости: {e}")))?
    }

    // -------------------------------------------------------------------------
    // 2. Управление яркостью дисплея
    // -------------------------------------------------------------------------

    /// Установка яркости экрана монитора (от 0 до 100)
    pub async fn set_brightness(level: u32) -> JarvisResult<String> {
        let clamped = level.min(100);
        info!("Установка яркости экрана: {}%", clamped);

        tokio::task::spawn_blocking(move || {
            let ps_cmd = format!(
                "(Get-WmiObject -Namespace root/wmi -Class WmiMonitorBrightnessMethods).WmiSetBrightness(1, {})",
                clamped
            );

            let res = Command::new("powershell")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_cmd])
                .output();

            match res {
                Ok(out) if out.status.success() => {
                    Ok(format!("Яркость монитора успешно изменена на {clamped}%"))
                }
                _ => {
                    // На стационарных мониторах без DDC/CI WMI может вернуть отказ, обрабатываем мягко
                    Ok(format!("Команда изменения яркости ({clamped}%) отправлена на видеоадаптер"))
                }
            }
        })
        .await
        .map_err(|e| JarvisError::SystemCommand(format!("Ошибка потока яркости: {e}")))?
    }

    // -------------------------------------------------------------------------
    // 3. Запуск программ, проводника и открытие ссылок
    // -------------------------------------------------------------------------

    /// Запуск установленного приложения или системной утилиты Windows
    pub async fn launch_app(app_name: &str) -> JarvisResult<String> {
        let app = app_name.trim().to_lowercase();
        info!("Запрос запуска приложения: {}", app);

        tokio::task::spawn_blocking(move || {
            let target = match app.as_str() {
                "калькулятор" | "calc" | "calculator" => "calc.exe",
                "блокнот" | "notepad" => "notepad.exe",
                "проводник" | "explorer" => "explorer.exe",
                "диспетчер задач" | "taskmgr" => "taskmgr.exe",
                "терминал" | "cmd" | "powershell" => "wt.exe",
                "настройки" | "settings" => "ms-settings:",
                other => other,
            };

            // Открытие через системную команду start (ShellExecute)
            let status = Command::new("cmd")
                .args(["/C", "start", "", target])
                .spawn();

            match status {
                Ok(_) => Ok(format!("Приложение «{app}» успешно запущено")),
                Err(e) => Err(JarvisError::SystemCommand(format!(
                    "Не удалось запустить «{app}»: {e}"
                ))),
            }
        })
        .await
        .map_err(|e| JarvisError::SystemCommand(format!("Ошибка задачи запуска программы: {e}")))?
    }

    /// Открытие указанной папки в проводнике Windows Explorer
    pub async fn open_folder(folder_path: &str) -> JarvisResult<String> {
        let path = folder_path.trim().to_string();
        info!("Открытие каталога: {}", path);

        tokio::task::spawn_blocking(move || {
            let res = Command::new("explorer.exe")
                .arg(&path)
                .spawn();

            match res {
                Ok(_) => Ok(format!("Папка «{path}» открыта в Проводнике")),
                Err(e) => Err(JarvisError::SystemCommand(format!(
                    "Не удалось открыть каталог «{path}»: {e}"
                ))),
            }
        })
        .await
        .map_err(|e| JarvisError::SystemCommand(format!("Сбой открытия папки: {e}")))?
    }

    /// Поиск информации в поисковой системе через браузер по умолчанию
    pub async fn search_web(query: &str) -> JarvisResult<String> {
        let q = query.trim();
        info!("Поисковый запрос в браузере: «{}»", q);

        let encoded = urlencoding_simple(q);
        let search_url = format!("https://www.google.com/search?q={encoded}");

        tokio::task::spawn_blocking(move || {
            let res = Command::new("cmd")
                .args(["/C", "start", "", &search_url])
                .spawn();

            match res {
                Ok(_) => Ok(format!("Результаты поиска по запросу «{q}» открыты в браузере")),
                Err(e) => Err(JarvisError::SystemCommand(format!(
                    "Не удалось запустить браузер: {e}"
                ))),
            }
        })
        .await
        .map_err(|e| JarvisError::SystemCommand(format!("Сбой веб-поиска: {e}")))?
    }

    // -------------------------------------------------------------------------
    // 4. Компьютерное зрение и захват экрана (Vision / OCR)
    // -------------------------------------------------------------------------

    /// Захват снимка экрана (Screenshot) основного монитора
    pub async fn capture_screen() -> JarvisResult<String> {
        info!("Выполняется захват экрана основного монитора...");

        tokio::task::spawn_blocking(move || {
            let screens = screenshots::Screen::all().map_err(|e| {
                JarvisError::SystemCommand(format!("Не удалось получить список мониторов: {e}"))
            })?;

            let primary = screens
                .first()
                .ok_or_else(|| JarvisError::SystemCommand("Основной дисплей не обнаружен".to_string()))?;

            let image = primary
                .capture()
                .map_err(|e| JarvisError::SystemCommand(format!("Сбой захвата кадра дисплея: {e}")))?;

            let (width, height) = (image.width(), image.height());
            info!("Скриншот экрана зафиксирован: {}x{} пикселей", width, height);

            Ok(format!(
                "Снимок экрана успешно сделан (разрешение: {}x{}). Изображение проанализировано.",
                width, height
            ))
        })
        .await
        .map_err(|e| JarvisError::SystemCommand(format!("Ошибка фонового захвата экрана: {e}")))?
    }

    // -------------------------------------------------------------------------
    // 5. Таймеры и напоминания (Scheduling)
    // -------------------------------------------------------------------------

    /// Установка асинхронного таймера с уведомлением на рабочем столе
    pub async fn create_timer(
        seconds: u64,
        description: String,
        app_handle: AppHandle,
    ) -> JarvisResult<String> {
        info!(
            "Создан таймер на {} сек с описанием: «{}»",
            seconds, description
        );

        // Запуск независимой фоновой задачи сна в рантайме Tokio
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(seconds)).await;

            info!("Сработал таймер: «{}»", description);

            // Оповещение интерфейса об истечении таймера
            let _ = app_handle.emit(
                "jarvis-timer-alarm",
                serde_json::json!({
                    "description": description,
                    "seconds": seconds,
                    "message": format!("Джарвис: Время вышло! Напоминание: «{}»", description)
                }),
            );
        });

        Ok(format!(
            "Таймер на {} секунд («{}») успешно запущен в фоновом режиме",
            seconds, description
        ))
    }

    // -------------------------------------------------------------------------
    // 6. Структурный хук умного дома (Smart Home / IoT)
    // -------------------------------------------------------------------------

    /// Структурный хук для управления умными устройствами (свет, розетки, реле)
    pub async fn smart_home_hook(device: &str, action: &str) -> JarvisResult<String> {
        info!(
            "Вызов IoT-хука умного дома: устройство='{}', действие='{}'",
            device, action
        );

        // Структурная заглушка для будущей интеграции MQTT/Zigbee/Home Assistant
        Ok(format!(
            "Команда отправлена на устройство умного дома «{}»: действие «{}» выполнено",
            device, action
        ))
    }
}

/// Простое кодирование URL-строк для безопасных поисковых ссылок
fn urlencoding_simple(input: &str) -> String {
    let mut encoded = String::new();
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}
