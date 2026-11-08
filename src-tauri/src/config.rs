// src-tauri/src/config.rs
//! Модуль управления конфигурацией и безопасного хранения ключей доступа.
//! Использует Windows Credential Manager через библиотеку `keyring`.

use crate::error::{JarvisError, JarvisResult};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

const SERVICE_NAME: &str = "JarvisVoiceAssistant";
const USERNAME_GROK_KEY: &str = "grok_api_key";
const GROK_API_PING_URL: &str = "https://api.x.ai/v1/models";

/// Настройки приложения, передаваемые между бэкендом и фронтендом
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Флаг закрепления окна поверх всех остальных
    pub always_on_top: bool,
    /// Выбранная визуальная тема: "jarvis" или "fluid-glass"
    pub theme: String,
    /// Язык голосового взаимодействия
    pub language: String,
    /// Наличие сохраненного ключа Grok API (сам ключ в целях безопасности не передается в открытом виде)
    pub has_grok_key: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            always_on_top: true,
            theme: "jarvis".to_string(),
            language: "ru".to_string(),
            has_grok_key: false,
        }
    }
}

/// Потокобезопасный менеджер конфигурации приложения
#[derive(Clone)]
pub struct ConfigManager {
    config: Arc<RwLock<AppConfig>>,
    http_client: reqwest::Client,
}

impl ConfigManager {
    /// Инициализация менеджера конфигурации с чтением текущего состояния ключей
    pub fn new() -> Self {
        let has_key = Self::has_stored_grok_key();
        let mut config = AppConfig::default();
        config.has_grok_key = has_key;

        Self {
            config: Arc::new(RwLock::new(config)),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(8))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    /// Проверка наличия сохраненного ключа в защищенном хранилище
    fn has_stored_grok_key() -> bool {
        match Entry::new(SERVICE_NAME, USERNAME_GROK_KEY) {
            Ok(entry) => entry.get_password().is_ok(),
            Err(_) => false,
        }
    }

    /// Безопасное получение ключа Grok API из Windows Credential Manager
    pub fn get_grok_key(&self) -> JarvisResult<String> {
        let entry = Entry::new(SERVICE_NAME, USERNAME_GROK_KEY)
            .map_err(|e| JarvisError::Keyring(format!("Не удалось получить доступ к хранилищу: {e}")))?;

        match entry.get_password() {
            Ok(key) => Ok(key),
            Err(e) => {
                warn!("Ключ Grok API не найден в защищенном хранилище: {e}");
                Err(JarvisError::Config("Ключ Grok API не задан в настройках".to_string()))
            }
        }
    }

    /// Сохранение ключа Grok API в Windows Credential Manager
    pub async fn save_grok_key(&self, api_key: &str) -> JarvisResult<()> {
        let trimmed_key = api_key.trim();
        if trimmed_key.is_empty() {
            return Err(JarvisError::Config("Ключ API не может быть пустым".to_string()));
        }

        let entry = Entry::new(SERVICE_NAME, USERNAME_GROK_KEY)
            .map_err(|e| JarvisError::Keyring(format!("Ошибка инициализации хранилища: {e}")))?;

        entry
            .set_password(trimmed_key)
            .map_err(|e| JarvisError::Keyring(format!("Не удалось записать ключ: {e}")))?;

        let mut cfg = self.config.write().await;
        cfg.has_grok_key = true;
        info!("Ключ Grok API успешно сохранен в защищенное хранилище Windows");

        Ok(())
    }

    /// Асинхронная валидация ключа через облегченный запрос (Ping test к xAI API)
    pub async fn ping_test_grok_key(&self, test_key: &str) -> JarvisResult<bool> {
        let trimmed = test_key.trim();
        if trimmed.is_empty() {
            return Err(JarvisError::Config("Ключ пуст, проверка отклонена".to_string()));
        }

        info!("Выполняется проверочный ping-запрос к API xAI Grok...");

        let response = self
            .http_client
            .get(GROK_API_PING_URL)
            .header("Authorization", format!("Bearer {trimmed}"))
            .send()
            .await
            .map_err(|err| {
                error!("Сетевой сбой при проверке ключа Grok: {err}");
                JarvisError::Network(err)
            })?;

        let status = response.status();
        if status.is_success() {
            info!("Проверка ключа xAI Grok завершена успешно (HTTP 200)");
            Ok(true)
        } else {
            let status_code = status.as_u16();
            warn!("Проверка ключа xAI Grok отклонена сервером с кодом: {status_code}");
            Err(JarvisError::Config(format!(
                "Ошибка авторизации API: сервер вернул HTTP {status_code}. Проверьте корректность ключа."
            )))
        }
    }

    /// Получение текущего снимка конфигурации для фронтенда
    pub async fn get_config(&self) -> AppConfig {
        self.config.read().await.clone()
    }
}
