// src/types.ts
/**
 * Определение типов данных для клиентской части ассистента Джарвис.
 * Все комментарии и структуры строго документированы на русском языке.
 */

/// Состояния активности голосового ассистента
export type AssistantState = 'idling' | 'listening' | 'thinking' | 'speaking';

/// Доступные визуальные темы оформления
export type ThemeMode = 'jarvis' | 'fluid-glass';

/// Тип роли в диалоге
export type MessageRole = 'user' | 'assistant' | 'tool' | 'system';

/// Структура записи в истории диалога
export interface ChatMessage {
  id: string;
  role: MessageRole;
  text: string;
  timestamp: string;
  toolCall?: {
    name: string;
    description: string;
    status: 'pending' | 'success' | 'failed';
    result?: string;
  };
}

/// Конфигурация приложения, синхронизируемая с Rust-бэкендом
export interface AppConfig {
  always_on_top: boolean;
  theme: ThemeMode;
  language: string;
  has_grok_key: boolean;
}

/// Событие изменения состояния звукового потока от бэкенда
export interface AudioLevelEvent {
  level: number; // Нормализованное значение от 0.0 до 1.0
  isVoiceActive: boolean;
}
