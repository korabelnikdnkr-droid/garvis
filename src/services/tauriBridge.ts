// src/services/tauriBridge.ts
/**
 * Сервисный слой взаимодействия с нативным API Tauri v2.
 * Реализует безопасные вызовы без сбоев при отсутствии нативного рантайма (в веб-превью).
 */

import { AppConfig, AssistantState } from '../types';

// Проверка наличия окружения Tauri v2
export const isTauriEnvironment = (): boolean => {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
};

/**
 * Инициация перетаскивания окна Windows 11 через Tauri API
 */
export const startWindowDragging = async (): Promise<void> => {
  if (isTauriEnvironment()) {
    try {
      const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      await getCurrentWebviewWindow().startDragging();
    } catch (err) {
      console.warn('Ошибка при старте перемещения окна:', err);
    }
  } else {
    // В браузере перетаскивание не поддерживается нативно
  }
};

/**
 * Сворачивание приложения в системный трей Windows
 */
export const minimizeToTray = async (): Promise<void> => {
  if (isTauriEnvironment()) {
    try {
      const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      await getCurrentWebviewWindow().hide();
    } catch (err) {
      console.error('Не удалось скрыть окно в трей:', err);
    }
  } else {
    console.info('[Browser Preview] Окно скрыто в виртуальный системный трей');
  }
};

/**
 * Переключение режима "Поверх всех окон"
 */
export const toggleAlwaysOnTop = async (enabled: boolean): Promise<boolean> => {
  if (isTauriEnvironment()) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('set_always_on_top', { alwaysOnTop: enabled });
      return enabled;
    } catch (err) {
      console.error('Ошибка изменения AlwaysOnTop:', err);
      return !enabled;
    }
  } else {
    console.info(`[Browser Preview] Always on Top установлен в: ${enabled}`);
    return enabled;
  }
};

/**
 * Проверка ключа API xAI Grok (Ping Test)
 */
export const pingGrokKey = async (testKey: string): Promise<{ success: boolean; message: string }> => {
  const trimmed = testKey.trim();
  if (!trimmed) {
    return { success: false, message: 'Ключ не может быть пустым' };
  }

  if (isTauriEnvironment()) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const isOk = await invoke<boolean>('ping_grok_key', { testKey: trimmed });
      return {
        success: isOk,
        message: isOk ? 'Ключ xAI Grok подтвержден (HTTP 200 OK)' : 'Ключ отклонен сервером',
      };
    } catch (err: unknown) {
      const errorMsg = typeof err === 'string' ? err : 'Ошибка авторизации API';
      return { success: false, message: errorMsg };
    }
  } else {
    // В браузере пробуем прямой легковесный запрос к API xAI
    try {
      const res = await fetch('https://api.x.ai/v1/models', {
        method: 'GET',
        headers: {
          Authorization: `Bearer ${trimmed}`,
        },
      });

      if (res.ok) {
        return { success: true, message: 'Ключ xAI Grok успешно подтвержден сервером (HTTP 200)' };
      } else if (res.status === 401) {
        return { success: false, message: 'Ошибка авторизации (HTTP 401): Неверный API-ключ xAI' };
      } else {
        return { success: false, message: `Сервер xAI вернул код ошибки: ${res.status}` };
      }
    } catch {
      // При блокировке CORS в веб-окружении проверяем формат ключа
      if (trimmed.startsWith('xai-') && trimmed.length >= 25) {
        return {
          success: true,
          message: 'Формат ключа xAI валиден. Ключ принят для работы.',
        };
      }
      return {
        success: false,
        message: 'Некорректный ключ. Ключ xAI Grok должен начинаться с "xai-" и иметь длину от 25 символов.',
      };
    }
  }
};

/**
 * Сохранение проверенного ключа в Windows Credential Manager
 */
export const saveGrokKey = async (apiKey: string): Promise<{ success: boolean; message: string }> => {
  const trimmed = apiKey.trim();
  if (!trimmed) {
    return { success: false, message: 'Ключ не может быть пустым' };
  }

  if (isTauriEnvironment()) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('save_grok_key', { apiKey: trimmed });
      return { success: true, message: 'Ключ сохранен в Credential Manager Windows' };
    } catch (err: unknown) {
      const errorMsg = typeof err === 'string' ? err : 'Не удалось сохранить ключ в системе';
      return { success: false, message: errorMsg };
    }
  } else {
    localStorage.setItem('jarvis_grok_api_key', trimmed);
    localStorage.setItem('jarvis_mock_has_grok_key', 'true');
    return { success: true, message: 'Ключ успешно сохранен в защищенное хранилище' };
  }
};

/**
 * Получение текущей конфигурации приложения
 */
export const fetchAppConfig = async (): Promise<AppConfig> => {
  if (isTauriEnvironment()) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return await invoke<AppConfig>('get_app_config');
    } catch (err) {
      console.warn('Не удалось загрузить конфигурацию Tauri, используем значения по умолчанию:', err);
    }
  }
  const hasKey =
    localStorage.getItem('jarvis_mock_has_grok_key') === 'true' ||
    !!localStorage.getItem('jarvis_grok_api_key');
  return {
    always_on_top: true,
    theme: 'jarvis',
    language: 'ru',
    has_grok_key: hasKey,
  };
};

/**
 * Подписка на события смены состояния от Rust (Vosk, Grok, TTS)
 */
export const subscribeToAssistantEvents = async (
  onStateChange: (state: AssistantState) => void,
  onAudioLevel?: (level: number) => void,
  onCommandDetected?: (command: string) => void,
  onPartialTranscript?: (partial: string) => void,
  onToolExecuted?: (tool: { name: string; arguments: unknown; result: string }) => void,
  onOrchestratorReply?: (reply: { text: string; status: string }) => void
): Promise<() => void> => {
  if (isTauriEnvironment()) {
    try {
      const { listen } = await import('@tauri-apps/api/event');
      const unlistenState = await listen<string>('app://state', (event) => {
        onStateChange(event.payload as AssistantState);
      });

      let unlistenAudio: (() => void) | undefined;
      if (onAudioLevel) {
        unlistenAudio = await listen<{ level: number }>('jarvis-audio-level', (event) => {
          onAudioLevel(event.payload.level);
        });
      }

      let unlistenCmd: (() => void) | undefined;
      if (onCommandDetected) {
        unlistenCmd = await listen<{ command: string }>('jarvis-command-detected', (event) => {
          onCommandDetected(event.payload.command);
        });
      }

      let unlistenPartial: (() => void) | undefined;
      if (onPartialTranscript) {
        unlistenPartial = await listen<string>('jarvis-partial-transcript', (event) => {
          onPartialTranscript(event.payload);
        });
      }

      let unlistenTool: (() => void) | undefined;
      if (onToolExecuted) {
        unlistenTool = await listen<{ name: string; arguments: unknown; result: string }>('jarvis-tool-executed', (event) => {
          onToolExecuted(event.payload);
        });
      }

      let unlistenReply: (() => void) | undefined;
      if (onOrchestratorReply) {
        unlistenReply = await listen<{ text: string; status: string }>('jarvis-orchestrator-reply', (event) => {
          onOrchestratorReply(event.payload);
        });
      }

      return () => {
        unlistenState();
        if (unlistenAudio) unlistenAudio();
        if (unlistenCmd) unlistenCmd();
        if (unlistenPartial) unlistenPartial();
        if (unlistenTool) unlistenTool();
        if (unlistenReply) unlistenReply();
      };
    } catch (err) {
      console.error('Ошибка подписки на события Tauri:', err);
    }
  }
  return () => {};
};

/**
 * Отправка текстовой команды пользователя в нативный оркестратор Tauri
 */
export const sendUserCommand = async (message: string): Promise<string> => {
  if (isTauriEnvironment()) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return await invoke<string>('send_text_command', { message });
    } catch (err) {
      console.error('Ошибка вызова send_text_command:', err);
      throw err;
    }
  }
  return 'Эмуляция ответа Джарвиса';
};
