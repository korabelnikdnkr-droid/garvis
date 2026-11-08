// src-tauri/src/llm.rs
//! Модуль интеграции с языковой моделью xAI Grok через OpenAI-совместимый API.
//! Поддерживает потоковую передачу токенов (Streaming), историю диалога и Function Calling (Tools).

use crate::error::{JarvisError, JarvisResult};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

const GROK_API_URL: &str = "https://api.x.ai/v1/chat/completions";
const GROK_MODEL: &str = "grok-beta";
const MAX_CONTEXT_TURNS: usize = 12;

/// Сообщение диалогового контекста Grok
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Клиент взаимодействия с xAI Grok
pub struct GrokClient {
    http_client: reqwest::Client,
    history: Arc<RwLock<Vec<LlmMessage>>>,
}

impl GrokClient {
    /// Инициализация клиента Grok с базовым системным промптом на русском языке
    pub fn new() -> Self {
        let system_message = LlmMessage {
            role: "system".to_string(),
            content: Some(
                "Ты — Джарвис (JARVIS), элитный персональный голосовой ассистент для Windows 11. \
                 Твой тон: вежливый, уверенный, лаконичный и высокотехнологичный. \
                 Всегда отвечай строго на русском языке. \
                 Если пользователь просит изменить громкость, яркость, открыть программу, папку, \
                 найти что-то в интернете, сделать снимок экрана или завести таймер, \
                 ТЫ ОБЯЗАН вызвать соответствующий нативный инструмент (Function Calling). \
                 Никогда не симулируй выполнение действий текстом без вызова инструментов."
                    .to_string(),
            ),
            tool_calls: None,
            tool_call_id: None,
        };

        Self {
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            history: Arc::new(RwLock::new(vec![system_message])),
        }
    }

    /// Очистка истории диалога (с сохранением системного промпта)
    pub async fn clear_history(&self) {
        let mut hist = self.history.write().await;
        if let Some(system_msg) = hist.first().cloned() {
            hist.clear();
            hist.push(system_msg);
        }
        info!("История контекста диалога Grok очищена");
    }

    /// Схемы доступных инструментов Windows 11 (Function Calling)
    pub fn get_tools_schema() -> Value {
        json!([
            {
                "type": "function",
                "function": {
                    "name": "set_system_volume",
                    "description": "Установка громкости аудиомикшера Windows 11 в процентах (0-100)",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "level": { "type": "integer", "description": "Уровень громкости от 0 до 100" }
                        },
                        "required": ["level"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "set_screen_brightness",
                    "description": "Установка уровня яркости дисплея монитора в процентах (0-100)",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "level": { "type": "integer", "description": "Уровень яркости от 0 до 100" }
                        },
                        "required": ["level"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "launch_application",
                    "description": "Запуск приложения, системной утилиты или файла Windows (например, калькулятор, блокнот, проводник, диспетчер задач)",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "app_name": { "type": "string", "description": "Название или имя исполняемого файла программы" }
                        },
                        "required": ["app_name"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "open_folder",
                    "description": "Открытие конкретной папки в проводнике Windows Explorer",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "folder_path": { "type": "string", "description": "Путь к каталогу (например, 'Downloads', 'Documents', 'C:\\')" }
                        },
                        "required": ["folder_path"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "search_web",
                    "description": "Поиск информации в интернете через браузер по умолчанию",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Текст поискового запроса" }
                        },
                        "required": ["query"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "capture_screen",
                    "description": "Создание снимка экрана (скриншота) монитора для анализа изображения",
                    "parameters": {
                        "type": "object",
                        "properties": {}
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "create_timer",
                    "description": "Установка звукового таймера или напоминания на рабочем столе Windows",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "seconds": { "type": "integer", "description": "Длительность таймера в секундах" },
                            "description": { "type": "string", "description": "Напоминание или назначение таймера" }
                        },
                        "required": ["seconds", "description"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "smart_home_control",
                    "description": "Управление подключенными устройствами умного дома (IoT)",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "device": { "type": "string", "description": "Имя умного устройства (свет, розетка, кондиционер)" },
                            "action": { "type": "string", "description": "Команда (включить, выключить, переключить)" }
                        },
                        "required": ["device", "action"]
                    }
                }
            }
        ])
    }

    /// Добавление реплики пользователя в скользящее контекстное окно
    pub async fn add_user_message(&self, content: &str) {
        let mut hist = self.history.write().await;
        hist.push(LlmMessage {
            role: "user".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });

        // Контроль длины контекста (сохраняем системный промпт + последние N реплик)
        if hist.len() > MAX_CONTEXT_TURNS + 1 {
            let keep_from = hist.len() - MAX_CONTEXT_TURNS;
            let mut pruned = vec![hist[0].clone()];
            pruned.extend_from_slice(&hist[keep_from..]);
            *hist = pruned;
        }
    }

    /// Добавление ответа модели с вызовом инструмента в контекст
    pub async fn add_assistant_tool_call(&self, tool_calls: Vec<Value>) {
        let mut hist = self.history.write().await;
        hist.push(LlmMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        });
    }

    /// Добавление результата выполнения инструмента в контекст
    pub async fn add_tool_result(&self, tool_call_id: &str, result: &str) {
        let mut hist = self.history.write().await;
        hist.push(LlmMessage {
            role: "tool".to_string(),
            content: Some(result.to_string()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.to_string()),
        });
    }

    /// Добавление итогового текстового ответа ассистента в контекст
    pub async fn add_assistant_response(&self, content: &str) {
        let mut hist = self.history.write().await;
        hist.push(LlmMessage {
            role: "assistant".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    /// Выполнение запроса к Grok API с поддержкой Function Calling
    pub async fn chat_completion(
        &self,
        api_key: &str,
    ) -> JarvisResult<LlmCompletionResponse> {
        let messages_snapshot = self.history.read().await.clone();
        let tools = Self::get_tools_schema();

        let payload = json!({
            "model": GROK_MODEL,
            "messages": messages_snapshot,
            "tools": tools,
            "tool_choice": "auto",
            "temperature": 0.4
        });

        info!("Отправка запроса к xAI Grok ({GROK_MODEL})...");

        let response = self
            .http_client
            .post(GROK_API_URL)
            .header("Authorization", format!("Bearer {}", api_key.trim()))
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                error!("Сетевой сбой при отправке запроса в Grok: {e}");
                JarvisError::Network(e)
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("Ошибка API Grok HTTP {}: {}", status.as_u16(), error_text);
            return Err(JarvisError::Config(format!(
                "Ошибка xAI Grok (HTTP {}): {}",
                status.as_u16(),
                error_text
            )));
        }

        let json_body: Value = response.json().await.map_err(|e| {
            error!("Не удалось распарсить ответ Grok в JSON: {e}");
            JarvisError::Serialization(e)
        })?;

        // Извлечение выбора первого кандидата
        let choice = json_body["choices"]
            .get(0)
            .ok_or_else(|| JarvisError::Internal("Ответ Grok не содержит choices".to_string()))?;

        let message = &choice["message"];

        // Проверка наличия tool_calls
        if let Some(tool_calls_val) = message.get("tool_calls").and_then(|t| t.as_array()) {
            if !tool_calls_val.is_empty() {
                info!(
                    "Grok запросил вызов {} системных инструментов Windows",
                    tool_calls_val.len()
                );
                return Ok(LlmCompletionResponse::ToolCalls(tool_calls_val.clone()));
            }
        }

        // Обычный текстовый ответ
        let text_content = message["content"]
            .as_str()
            .unwrap_or("Джарвис: Ответ получен.")
            .to_string();

        Ok(LlmCompletionResponse::Text(text_content))
    }

    /// Потоковая отправка токенов в реальном времени во фронтенд (Streaming SSE)
    pub async fn stream_reply(
        &self,
        api_key: &str,
        app_handle: &AppHandle,
    ) -> JarvisResult<String> {
        let messages_snapshot = self.history.read().await.clone();

        let payload = json!({
            "model": GROK_MODEL,
            "messages": messages_snapshot,
            "stream": true,
            "temperature": 0.4
        });

        let response = self
            .http_client
            .post(GROK_API_URL)
            .header("Authorization", format!("Bearer {}", api_key.trim()))
            .json(&payload)
            .send()
            .await
            .map_err(JarvisError::Network)?;

        let mut stream = response.bytes_stream();
        let mut full_accumulated = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(bytes) => bytes,
                Err(err) => {
                    warn!("Обрыв потока стриминга Grok: {err}");
                    break;
                }
            };

            let text = String::from_utf8_lossy(&chunk);
            for line in text.lines() {
                let trimmed = line.trim();
                if let Some(data_str) = trimmed.strip_prefix("data: ") {
                    if data_str == "[DONE]" {
                        break;
                    }
                    if let Ok(parsed) = serde_json::from_str::<Value>(data_str) {
                        if let Some(token) = parsed["choices"][0]["delta"]["content"].as_str() {
                            full_accumulated.push_str(token);
                            // Эмитим токен в интерфейс для мгновенного появления букв
                            let _ = app_handle.emit("jarvis-stream-token", token);
                        }
                    }
                }
            }
        }

        Ok(full_accumulated)
    }
}

/// Результат обработки первого шага диалога
#[derive(Debug)]
pub enum LlmCompletionResponse {
    Text(String),
    ToolCalls(Vec<Value>),
}
