// src-tauri/src/error.rs
//! Модуль централизованной обработки ошибок ассистента Джарвис.
//! Реализует строгую обработку результатов без вызовов unwrap() или expect().

use thiserror::Error;

/// Единый перечень ошибок приложения Джарвис.
#[derive(Error, Debug)]
pub enum JarvisError {
    /// Ошибки файлового ввода-вывода или системных путей
    #[error("Ошибка файловой системы: {0}")]
    Io(#[from] std::io::Error),

    /// Сетевые ошибки или сбои HTTP-запросов (Grok API, Edge-TTS)
    #[error("Сетевая ошибка: {0}")]
    Network(#[from] reqwest::Error),

    /// Ошибки сериализации/десериализации JSON
    #[error("Ошибка сериализации JSON: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Ошибки безопасного хранилища ключей (Windows Credential Manager)
    #[error("Ошибка хранилища учетных данных: {0}")]
    Keyring(String),

    /// Ошибки конфигурации (например, отсутствие ключа API)
    #[error("Ошибка конфигурации: {0}")]
    Config(String),

    /// Ошибки аудиопотока или микрофона
    #[error("Ошибка аудиоустройства: {0}")]
    Audio(String),

    /// Ошибки движка распознавания речи Vosk
    #[error("Ошибка распознавания речи: {0}")]
    SpeechRecognition(String),

    /// Ошибки выполнения системных команд Windows 11
    #[error("Ошибка выполнения системной команды: {0}")]
    SystemCommand(String),

    /// Внутренняя ошибка состояния приложения
    #[error("Внутренняя ошибка сервиса: {0}")]
    Internal(String),
}

/// Стандартный псевдоним результата для всех внутренних вызовов
pub type JarvisResult<T> = Result<T, JarvisError>;

// Реализация сериализации для передачи ошибок во фронтенд через Tauri commands
impl serde::Serialize for JarvisError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
