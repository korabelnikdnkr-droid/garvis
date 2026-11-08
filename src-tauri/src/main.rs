// src-tauri/src/main.rs
//! Точка входа исполняемого файла приложения Джарвис для Windows 11.
//! Отключает появление консольного окна в релизной сборке для чистого GUI.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Делегирование запуска библиотечному модулю с инициализацией всех систем
    jarvis_assistant_lib::run();
}
