// src-tauri/src/orchestrator.rs
//! Агентный оркестратор Джарвис (Agentic Orchestrator).
//! Связывает транскрипцию Vosk STT, клиент xAI Grok, цикл вызова системных инструментов Windows
//! и передачу результата в синтез речи TTS и фронтенд.

use crate::config::ConfigManager;
use crate::error::{JarvisError, JarvisResult};
use crate::llm::{GrokClient, LlmCompletionResponse};
use crate::tools::SystemTools;
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

/// Главный агентный оркестратор
#[derive(Clone)]
pub struct Orchestrator {
    app_handle: AppHandle,
    config_manager: ConfigManager,
    grok_client: Arc<GrokClient>,
}

impl Orchestrator {
    /// Инициализация оркестратора
    pub fn new(app_handle: AppHandle, config_manager: ConfigManager) -> Self {
        Self {
            app_handle,
            config_manager,
            grok_client: Arc::new(GrokClient::new()),
        }
    }

    /// Обработка поступившей голосовой или текстовой команды пользователя
    pub async fn process_user_command(&self, command: &str) -> JarvisResult<String> {
        let clean_command = command.trim();
        if clean_command.is_empty() {
            return Ok(String::new());
        }

        info!("Оркестратор принял команду: «{}»", clean_command);

        // 1. Оповещаем интерфейс о начале размышлений (Thinking)
        let _ = self.app_handle.emit("app://state", "thinking");
        let _ = self.app_handle.emit("jarvis-state-changed", "thinking");

        // 2. Получение ключа Grok API из защищенного хранилища
        let api_key = match self.config_manager.get_grok_key() {
            Ok(k) => k,
            Err(e) => {
                warn!("Ключ Grok API отсутствует: {e}. Переход в автономный режим.");
                let fallback_msg = "Ключ Grok API не настроен в Windows Credential Manager. Пожалуйста, укажите его в настройках.".to_string();

                let _ = self.app_handle.emit(
                    "jarvis-orchestrator-reply",
                    serde_json::json!({
                        "text": fallback_msg,
                        "status": "warning"
                    }),
                );
                let _ = self.app_handle.emit("app://state", "idling");
                return Ok(fallback_msg);
            }
        };

        // 3. Добавляем реплику в контекст диалога
        self.grok_client.add_user_message(clean_command).await;

        // 4. Первый запрос к модели с поддержкой Function Calling
        let completion = match self.grok_client.chat_completion(&api_key).await {
            Ok(c) => c,
            Err(e) => {
                error!("Сбой при обращении к Grok API: {e}");
                let err_text = format!("Не удалось получить ответ от Grok: {e}");
                let _ = self.app_handle.emit(
                    "jarvis-orchestrator-reply",
                    serde_json::json!({ "text": err_text, "status": "error" }),
                );
                let _ = self.app_handle.emit("app://state", "idling");
                return Err(e);
            }
        };

        // 5. Обработка ветвления: вызов инструментов или прямой ответ
        let final_text = match completion {
            LlmCompletionResponse::Text(text) => {
                info!("Grok вернул прямой ответ без вызова инструментов");
                self.grok_client.add_assistant_response(&text).await;
                text
            }
            LlmCompletionResponse::ToolCalls(tool_calls) => {
                info!("Выполнение цикла Function Calling ({} инструментов)...", tool_calls.len());
                self.grok_client.add_assistant_tool_call(tool_calls.clone()).await;

                // Последовательное исполнение каждого вызванного инструмента
                for call in &tool_calls {
                    let tool_call_id = call["id"].as_str().unwrap_or("call_default");
                    let func_name = call["function"]["name"].as_str().unwrap_or("");
                    let args_str = call["function"]["arguments"].as_str().unwrap_or("{}");

                    info!("Вызов инструмента: '{}' с параметрами: {}", func_name, args_str);

                    let args: Value = serde_json::from_str(args_str).unwrap_or(Value::Null);

                    // Выполнение нативного метода Windows 11
                    let execution_result = self.execute_tool(func_name, &args).await;

                    // Отправка карточки инструмента во фронтенд для отображения в журнале
                    let _ = self.app_handle.emit(
                        "jarvis-tool-executed",
                        serde_json::json!({
                            "name": func_name,
                            "arguments": args,
                            "result": execution_result
                        }),
                    );

                    // Передача результата выполнения инструмента обратно модели
                    self.grok_client.add_tool_result(tool_call_id, &execution_result).await;
                }

                // Запрос финального резюме от модели после исполнения инструментов
                info!("Запрос финального ответа Grok после вызова инструментов...");
                match self.grok_client.chat_completion(&api_key).await {
                    Ok(LlmCompletionResponse::Text(summary)) => {
                        self.grok_client.add_assistant_response(&summary).await;
                        summary
                    }
                    Ok(_) => "Инструменты успешно выполнены.".to_string(),
                    Err(e) => {
                        warn!("Ошибка получения итогового резюме: {e}");
                        "Системное действие выполнено.".to_string()
                    }
                }
            }
        };

        // 6. Оповещаем интерфейс о переходе в режим речи (Speaking)
        let _ = self.app_handle.emit("app://state", "speaking");
        let _ = self.app_handle.emit("jarvis-state-changed", "speaking");
        let _ = self.app_handle.emit(
            "jarvis-orchestrator-reply",
            serde_json::json!({
                "text": final_text,
                "status": "success"
            }),
        );

        info!("Финальный ответ ассистента сформирован: «{}»", final_text);

        // Передача текста в очередь TTS (подготовительный триггер для Фазы 5)
        let _ = self.app_handle.emit("jarvis-tts-speak", final_text.clone());

        Ok(final_text)
    }

    /// Диспетчер исполнения инструментов Windows 11
    async fn execute_tool(&self, func_name: &str, args: &Value) -> String {
        match func_name {
            "set_system_volume" => {
                let level = args["level"].as_u64().unwrap_or(50) as u32;
                match SystemTools::set_volume(level).await {
                    Ok(msg) => msg,
                    Err(e) => format!("Ошибка установки громкости: {e}"),
                }
            }
            "set_screen_brightness" => {
                let level = args["level"].as_u64().unwrap_or(70) as u32;
                match SystemTools::set_brightness(level).await {
                    Ok(msg) => msg,
                    Err(e) => format!("Ошибка регулировки яркости: {e}"),
                }
            }
            "launch_application" => {
                let app_name = args["app_name"].as_str().unwrap_or("");
                match SystemTools::launch_app(app_name).await {
                    Ok(msg) => msg,
                    Err(e) => format!("Ошибка запуска приложения: {e}"),
                }
            }
            "open_folder" => {
                let folder = args["folder_path"].as_str().unwrap_or("C:\\");
                match SystemTools::open_folder(folder).await {
                    Ok(msg) => msg,
                    Err(e) => format!("Ошибка открытия папки: {e}"),
                }
            }
            "search_web" => {
                let query = args["query"].as_str().unwrap_or("");
                match SystemTools::search_web(query).await {
                    Ok(msg) => msg,
                    Err(e) => format!("Ошибка выполнения поиска: {e}"),
                }
            }
            "capture_screen" => {
                match SystemTools::capture_screen().await {
                    Ok(msg) => msg,
                    Err(e) => format!("Ошибка создания снимка экрана: {e}"),
                }
            }
            "create_timer" => {
                let seconds = args["seconds"].as_u64().unwrap_or(60);
                let description = args["description"].as_str().unwrap_or("Таймер").to_string();
                match SystemTools::create_timer(seconds, description, self.app_handle.clone()).await {
                    Ok(msg) => msg,
                    Err(e) => format!("Ошибка установки таймера: {e}"),
                }
            }
            "smart_home_control" => {
                let device = args["device"].as_str().unwrap_or("устройство");
                let action = args["action"].as_str().unwrap_or("действие");
                match SystemTools::smart_home_hook(device, action).await {
                    Ok(msg) => msg,
                    Err(e) => format!("Ошибка IoT-контроллера: {e}"),
                }
            }
            unknown => {
                warn!("Получен неизвестный системный инструмент: {unknown}");
                format!("Инструмент «{unknown}» не поддерживается системой")
            }
        }
    }
}
