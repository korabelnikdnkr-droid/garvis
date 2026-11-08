// src-tauri/src/lib.rs
//! Главный библиотечный модуль ассистента Джарвис.
//! Инициализирует подсистемы логирования (tracing), асинхронный рантайм Tokio,
//! управление системным треем, микрофонным потоком CPAL, движком Vosk STT,
//! клиентом xAI Grok, агентным оркестратором, очередью TTS с Barge-in
//! и корректным завершением работы (Graceful Shutdown) в Windows 11.

pub mod audio;
pub mod audio_queue;
pub mod config;
pub mod error;
pub mod llm;
pub mod orchestrator;
pub mod stt;
pub mod tools;
pub mod tts;

use audio::AudioManager;
use audio_queue::AudioQueueManager;
use config::{AppConfig, ConfigManager};
use error::JarvisError;
use orchestrator::Orchestrator;
use std::sync::Arc;
use stt::VoskEngine;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Listener, Manager, RunEvent, State,
};
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Глобальное состояние приложения, доступное обработчикам команд
pub struct AppState {
    pub config_manager: ConfigManager,
    pub audio_manager: Arc<AudioManager>,
    pub vosk_engine: Arc<VoskEngine>,
    pub orchestrator: Arc<Orchestrator>,
    pub audio_queue: Arc<AudioQueueManager>,
}

/// Инициализация структурированного логирования через tracing
pub fn setup_logging() -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,jarvis_assistant_lib=debug"));

    let formatting_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .compact();

    tracing_subscriber::registry()
        .with(env_filter)
        .with(formatting_layer)
        .try_init()?;

    info!("Система логирования Tracing успешно инициализирована");
    Ok(())
}

// ---------------------------------------------------------
// Tauri IPC Команды для взаимодействия с фронтендом
// ---------------------------------------------------------

/// Команда проверки доступности и валидности ключа xAI Grok
#[tauri::command]
async fn ping_grok_key(
    state: State<'_, Arc<AppState>>,
    test_key: String,
) -> Result<bool, JarvisError> {
    info!("Получен запрос на валидацию ключа Grok API");
    state.config_manager.ping_test_grok_key(&test_key).await
}

/// Команда безопасного сохранения проверенного ключа в Credential Manager
#[tauri::command]
async fn save_grok_key(
    state: State<'_, Arc<AppState>>,
    api_key: String,
) -> Result<(), JarvisError> {
    info!("Получен запрос на сохранение ключа Grok API");
    state.config_manager.save_grok_key(&api_key).await
}

/// Команда получения текущего состояния конфигурации приложения
#[tauri::command]
async fn get_app_config(state: State<'_, Arc<AppState>>) -> Result<AppConfig, JarvisError> {
    Ok(state.config_manager.get_config().await)
}

/// Команда переключения режима "Поверх всех окон" (Always on Top)
#[tauri::command]
async fn set_always_on_top(
    app_handle: AppHandle,
    always_on_top: bool,
) -> Result<(), JarvisError> {
    if let Some(window) = app_handle.get_webview_window("main") {
        window
            .set_always_on_top(always_on_top)
            .map_err(|e| JarvisError::Internal(format!("Не удалось изменить режим AlwaysOnTop: {e}")))?;
        info!("Режим AlwaysOnTop изменен на: {}", always_on_top);
        Ok(())
    } else {
        Err(JarvisError::Internal("Главное окно не найдено".to_string()))
    }
}

/// Команда принудительного прерывания текущей речи ассистента (Barge-in вручную)
#[tauri::command]
async fn stop_speech(state: State<'_, Arc<AppState>>) -> Result<(), JarvisError> {
    info!("Принудительная остановка воспроизведения речи");
    state.audio_queue.interrupt().await;
    Ok(())
}

/// Команда отправки текстового запроса через оркестратор (ручной ввод в интерфейсе)
#[tauri::command]
async fn send_text_command(
    state: State<'_, Arc<AppState>>,
    message: String,
) -> Result<String, JarvisError> {
    info!("Получена текстовая команда: «{}»", message);
    state.orchestrator.process_user_command(&message).await
}

/// Точка входа в логику приложения Tauri
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 1. Инициализация системы логирования
    if let Err(e) = setup_logging() {
        eprintln!("Критический сбой инициализации логов: {e}");
    }

    info!("Запуск голосового ассистента Джарвис (Фаза 5: Финальная полировка)...");

    // 2. Сборка контекста Tauri v2
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            ping_grok_key,
            save_grok_key,
            get_app_config,
            set_always_on_top,
            stop_speech,
            send_text_command
        ])
        .setup(|app| {
            info!("Инициализация подсистем аудио, оркестратора и трея Windows 11...");

            let app_handle = app.handle().clone();

            // 3. Создание каналов связи и менеджеров
            let (pcm_tx, pcm_rx) = mpsc::channel::<Vec<i16>>(128);

            let config_manager = ConfigManager::new();
            let audio_manager = Arc::new(AudioManager::new(app_handle.clone(), pcm_tx));
            let vosk_engine = Arc::new(VoskEngine::new(app_handle.clone()));
            let orchestrator = Arc::new(Orchestrator::new(app_handle.clone(), config_manager.clone()));
            let audio_queue = Arc::new(AudioQueueManager::new(app_handle.clone()));

            // Запуск Vosk STT в фоновом потоке
            vosk_engine.start_processing(pcm_rx);

            // Запуск захвата микрофона
            audio_manager.start();

            // 4. Подписка: передача команд Vosk в оркестратор + мгновенный Barge-in
            let orch_clone = orchestrator.clone();
            let queue_for_bargein = audio_queue.clone();

            app.listen("jarvis-command-detected", move |event| {
                let orch = orch_clone.clone();
                let queue = queue_for_bargein.clone();
                let payload = event.payload().to_string();

                tokio::spawn(async move {
                    // Если ассистент говорил в момент появления команды пользователя — прерываем речь
                    queue.interrupt().await;

                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&payload) {
                        if let Some(cmd) = json_val["command"].as_str() {
                            let _ = orch.process_user_command(cmd).await;
                        }
                    }
                });
            });

            // 5. Подписка: передача текста из оркестратора в очередь воспроизведения TTS
            let queue_for_speech = audio_queue.clone();
            app.listen("jarvis-tts-speak", move |event| {
                let queue = queue_for_speech.clone();
                let speech_text = event.payload().trim_matches('"').to_string();

                tokio::spawn(async move {
                    let _ = queue.enqueue_speech(&speech_text).await;
                });
            });

            // Регистрация AppState в Tauri
            let app_state = Arc::new(AppState {
                config_manager,
                audio_manager,
                vosk_engine,
                orchestrator,
                audio_queue,
            });
            app.manage(app_state);

            // 6. Системный трей Windows 11
            let quit_item = MenuItem::with_id(app, "quit", "Выход из Джарвис", true, None<String>)
                .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

            let toggle_item = MenuItem::with_id(app, "toggle", "Показать/Скрыть", true, None<String>)
                .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

            let menu = Menu::with_items(app, &[&toggle_item, &quit_item])
                .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Джарвис — Голосовой ассистент")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        info!("Завершение работы по команде из трея");
                        app.exit(0);
                    }
                    "toggle" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let is_visible = window.is_visible().unwrap_or(false);
                            if is_visible {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app);

            if let Err(e) = _tray {
                error!("Не удалось построить системный трей: {e}");
            } else {
                info!("Системный трей Windows 11 успешно подключен");
            }

            Ok(())
        })
        .build(tauri::generate_context!());

    let app = match app {
        Ok(a) => a,
        Err(e) => {
            error!("Критический сбой сборки Tauri контекста: {e}");
            return;
        }
    };

    // 7. Корректное завершение работы (Graceful Shutdown)
    app.run(|app_handle, event| match event {
        RunEvent::ExitRequested { .. } => {
            info!("Получен сигнал завершения работы приложения. Очистка ресурсов...");
            if let Some(state) = app_handle.try_state::<Arc<AppState>>() {
                state.audio_manager.stop();
                state.vosk_engine.stop();
            }
            info!("Все аудиопотоки и потоки захвата остановлены. Выход без зомби-процессов.");
        }
        _ => {}
    });
}
