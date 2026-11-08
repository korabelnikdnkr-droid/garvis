// src/App.tsx
/**
 * Главный контейнер интерфейса ассистента Джарвис.
 * Объединяет безрамочное окно Windows 11, голосовую сферу, выдвижную историю и панель управления.
 * Оснащен автономным интеллектом JarvisBrain, Web Speech API и голосовым синтезом TTS.
 */

import React, { useState, useEffect, useRef } from 'react';
import { ThemeProvider, useTheme } from './context/ThemeContext';
import { WindowHeader } from './components/WindowHeader';
import { VocalSphere } from './components/VocalSphere';
import { ControlsBar } from './components/ControlsBar';
import { ChatOverlay } from './components/ChatOverlay';
import { SettingsModal } from './components/SettingsModal';
import { AssistantState, ChatMessage } from './types';
import {
  fetchAppConfig,
  subscribeToAssistantEvents,
} from './services/tauriBridge';
import { JarvisBrain } from './services/aiBrain';
import { voiceService } from './services/voiceService';
import { Timer, BellRing } from 'lucide-react';

function JarvisApp() {
  const { theme } = useTheme();
  const isJarvis = theme === 'jarvis';

  // Состояния ассистента
  const [state, setState] = useState<AssistantState>('idling');
  const [audioLevel, setAudioLevel] = useState<number>(0.25);
  const [alwaysOnTop, setAlwaysOnTop] = useState<boolean>(true);
  const [hasGrokKey, setHasGrokKey] = useState<boolean>(false);

  // Оверлеи и модальные окна
  const [isChatOpen, setIsChatOpen] = useState<boolean>(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);

  // Активный таймер обратного отсчета
  const [activeTimer, setActiveTimer] = useState<{ secondsLeft: number; label: string } | null>(null);
  const timerIntervalRef = useRef<NodeJS.Timeout | null>(null);

  // История диалога с начальным приветствием
  const [messages, setMessages] = useState<ChatMessage[]>([
    {
      id: 'welcome',
      role: 'assistant',
      text: 'Системы инициализированы. Я готов к работе. Скажите «Джарвис», нажмите на микрофон или введите команду.',
      timestamp: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
    },
  ]);

  // Загрузка конфигурации из бэкенда при монтировании
  useEffect(() => {
    let isMounted = true;
    fetchAppConfig().then((cfg) => {
      if (isMounted) {
        setAlwaysOnTop(cfg.always_on_top);
        setHasGrokKey(cfg.has_grok_key);
      }
    });

    // Подписка на события нативного Tauri (если запущен в Windows 11)
    const unsubscribePromise = subscribeToAssistantEvents(
      (newState) => setState(newState),
      (level) => setAudioLevel(level),
      (command) => {
        if (command && command.trim()) {
          handleSendMessage(command.trim());
        }
      },
      undefined,
      (tool) => {
        const toolCardMsg: ChatMessage = {
          id: `tool-${Date.now()}-${Math.random()}`,
          role: 'assistant',
          text: `Выполнен инструмент: ${tool.name}`,
          timestamp: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
          toolCall: {
            name: tool.name,
            description: JSON.stringify(tool.arguments),
            status: 'success',
            result: tool.result,
          },
        };
        setMessages((prev) => [...prev, toolCardMsg]);
      },
      (reply) => {
        const botMsg: ChatMessage = {
          id: `reply-${Date.now()}`,
          role: 'assistant',
          text: reply.text,
          timestamp: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
        };
        setMessages((prev) => [...prev, botMsg]);
        voiceService.speak(
          reply.text,
          () => setState('speaking'),
          () => setState('idling')
        );
      }
    );

    return () => {
      isMounted = false;
      unsubscribePromise.then((unsub) => unsub());
      voiceService.stopListening();
      voiceService.stopSpeaking();
    };
  }, []);

  // Таймер обратного отсчета
  useEffect(() => {
    if (!activeTimer) return;

    timerIntervalRef.current = setInterval(() => {
      setActiveTimer((prev) => {
        if (!prev) return null;
        if (prev.secondsLeft <= 1) {
          playChime();
          const alarmMsg: ChatMessage = {
            id: `alarm-${Date.now()}`,
            role: 'assistant',
            text: `⏰ Джарвис: Время вышло! Напоминание: «${prev.label}».`,
            timestamp: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
          };
          setMessages((m) => [...m, alarmMsg]);
          setIsChatOpen(true);
          voiceService.speak(`Сэр, время вышло! Напоминание: ${prev.label}.`);
          return null;
        }
        return { ...prev, secondsLeft: prev.secondsLeft - 1 };
      });
    }, 1000);

    return () => {
      if (timerIntervalRef.current) clearInterval(timerIntervalRef.current);
    };
  }, [activeTimer]);

  // Звуковой сигнал завершения таймера
  const playChime = () => {
    try {
      const AudioCtx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
      const ctx = new AudioCtx();
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = 'sine';
      osc.frequency.setValueAtTime(587.33, ctx.currentTime);
      osc.frequency.setValueAtTime(880, ctx.currentTime + 0.15);
      gain.gain.setValueAtTime(0.3, ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.6);
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.start();
      osc.stop(ctx.currentTime + 0.6);
    } catch {
      // Игнорируем
    }
  };

  // Обработка отправки команды (текстом или голосом)
  const handleSendMessage = async (text: string) => {
    if (!text || !text.trim()) return;

    // Прерываем текущую речь при новом запросе (Barge-in)
    voiceService.stopSpeaking();

    const timeNow = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    const userMsg: ChatMessage = {
      id: `usr-${Date.now()}`,
      role: 'user',
      text: text.trim(),
      timestamp: timeNow,
    };

    setMessages((prev) => [...prev, userMsg]);
    setIsChatOpen(true);
    setState('thinking');

    // Если работаем в нативном приложении Tauri v2 Windows
    try {
      const { isTauriEnvironment, sendUserCommand } = await import('./services/tauriBridge');
      if (isTauriEnvironment()) {
        await sendUserCommand(text);
        return;
      }
    } catch (e) {
      console.warn('Работа в режиме веб-клиента:', e);
    }

    // Обработка интеллектуальным ядром JarvisBrain
    try {
      const brainResult = await JarvisBrain.process(text);

      // Проверка на создание таймера
      if (brainResult.toolCall?.name === 'create_timer') {
        const match = text.match(/\d+/);
        const secs = match ? parseInt(match[0], 10) : 60;
        setActiveTimer({ secondsLeft: secs, label: text });
      }

      const botMsg: ChatMessage = {
        id: `bot-${Date.now()}`,
        role: 'assistant',
        text: brainResult.replyText,
        timestamp: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
        toolCall: brainResult.toolCall,
      };

      setMessages((prev) => [...prev, botMsg]);

      // Озвучивание ответа настоящим русским голосом (TTS) с анимацией сферы
      voiceService.speak(
        brainResult.replyText,
        () => setState('speaking'),
        () => setState('idling')
      );
    } catch (err) {
      console.error('Ошибка обработки запроса Джарвисом:', err);
      const errMsg: ChatMessage = {
        id: `err-${Date.now()}`,
        role: 'assistant',
        text: 'Произошла непредвиденная ошибка при обработке команды. Попробуйте еще раз.',
        timestamp: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
      };
      setMessages((prev) => [...prev, errMsg]);
      setState('idling');
    }
  };

  // Переключение микрофона
  const handleToggleMic = () => {
    // Если Джарвис говорит — останавливаем его речь (Barge-in)
    if (state === 'speaking') {
      voiceService.stopSpeaking();
      setState('idling');
      return;
    }

    // Если микрофон уже включен — выключаем
    if (state === 'listening') {
      voiceService.stopListening();
      setState('idling');
      return;
    }

    // Активируем реальный микрофон и распознавание речи
    setState('listening');
    voiceService.startAudioLevelTracking((level) => {
      setAudioLevel(level);
    });

    const success = voiceService.startListening(
      (recognizedSpeech) => {
        handleSendMessage(recognizedSpeech);
      },
      (interim) => {
        // Промежуточная речь
        console.debug('Промежуточная транскрипция:', interim);
      },
      (error) => {
        console.warn('Микрофон отклонен или недоступен:', error);
        setState('idling');
      }
    );

    if (!success) {
      // Имитируем прослушивание на короткое время, если микрофон заблокирован
      setTimeout(() => {
        setState('idling');
      }, 3000);
    }
  };

  // Клик по сфере: управление речью и микрофоном
  const handleSphereClick = () => {
    if (state === 'speaking') {
      voiceService.stopSpeaking();
      setState('idling');
      return;
    }
    if (state === 'listening') {
      voiceService.stopListening();
      setState('idling');
      return;
    }
    // Если в покое — включаем микрофон
    handleToggleMic();
  };

  return (
    <div className="w-full h-screen flex items-center justify-center p-3 bg-slate-950/60 overflow-hidden font-sans">
      {/* Главное безрамочное окно (420x660) с эффектом стекла и скруглением */}
      <main
        className={`relative w-[400px] h-[640px] max-w-full max-h-full rounded-3xl border flex flex-col justify-between overflow-hidden shadow-2xl transition-all duration-700 ${
          isJarvis
            ? 'bg-slate-950/90 border-cyan-800/40 shadow-[0_0_50px_rgba(6,182,212,0.25)]'
            : 'bg-slate-900/40 border-white/30 backdrop-blur-3xl shadow-[0_8px_40px_rgba(255,255,255,0.15)]'
        }`}
      >
        {/* Декоративные угловые маркеры в стиле интерфейса Jarvis */}
        {isJarvis && (
          <>
            <div className="absolute top-2 left-2 w-2 h-2 border-t-2 border-l-2 border-cyan-400 pointer-events-none" />
            <div className="absolute top-2 right-2 w-2 h-2 border-t-2 border-r-2 border-cyan-400 pointer-events-none" />
            <div className="absolute bottom-2 left-2 w-2 h-2 border-b-2 border-l-2 border-cyan-400 pointer-events-none" />
            <div className="absolute bottom-2 right-2 w-2 h-2 border-b-2 border-r-2 border-cyan-400 pointer-events-none" />
          </>
        )}

        {/* 1. Верхняя панель окна (Header & Draggable Region) */}
        <WindowHeader
          alwaysOnTop={alwaysOnTop}
          onToggleAlwaysOnTop={setAlwaysOnTop}
          onOpenSettings={() => setIsSettingsOpen(true)}
          hasGrokKey={hasGrokKey}
        />

        {/* Индикатор активного таймера (если запущен) */}
        {activeTimer && (
          <div className="mx-4 mt-2 px-3 py-1.5 rounded-xl bg-cyan-950/80 border border-cyan-500/40 flex items-center justify-between text-xs font-mono text-cyan-300 animate-pulse">
            <div className="flex items-center space-x-2">
              <Timer className="w-3.5 h-3.5 text-cyan-400" />
              <span>ТАЙМЕР: {activeTimer.secondsLeft} сек</span>
            </div>
            <button
              onClick={() => setActiveTimer(null)}
              className="text-[10px] text-slate-400 hover:text-white underline cursor-pointer"
            >
              Сбросить
            </button>
          </div>
        )}

        {/* 2. Центральная зона: Визуальный якорь (Vocal Sphere) */}
        <div className="relative flex-1 flex flex-col items-center justify-center px-4">
          <VocalSphere
            state={state}
            audioLevel={audioLevel}
            onClick={handleSphereClick}
          />

          {/* Интерактивный переключатель состояний для демонстрации */}
          <div className="mt-2 flex items-center space-x-1.5 p-1 rounded-full bg-white/5 border border-white/10 text-[10px] font-mono">
            {(['idling', 'listening', 'thinking', 'speaking'] as AssistantState[]).map((s) => (
              <button
                key={s}
                onClick={() => {
                  if (s === 'speaking') {
                    voiceService.speak('Джарвис к вашим услугам, сэр.');
                    setState('speaking');
                  } else if (s === 'listening') {
                    handleToggleMic();
                  } else {
                    voiceService.stopSpeaking();
                    voiceService.stopListening();
                    setState(s);
                  }
                }}
                className={`px-2 py-0.5 rounded-full capitalize transition-all duration-200 ${
                  state === s
                    ? 'bg-cyan-500 text-slate-950 font-bold shadow-[0_0_8px_rgba(6,182,212,0.6)]'
                    : 'text-slate-400 hover:text-white'
                }`}
              >
                {s === 'idling'
                  ? 'Покой'
                  : s === 'listening'
                  ? 'Слух'
                  : s === 'thinking'
                  ? 'Мысль'
                  : 'Речь'}
              </button>
            ))}
          </div>
        </div>

        {/* 3. Выдвижная панель истории диалога */}
        <ChatOverlay
          isOpen={isChatOpen}
          onClose={() => setIsChatOpen(false)}
          messages={messages}
          onClear={() => setMessages([])}
        />

        {/* 4. Нижняя панель управления */}
        <ControlsBar
          state={state}
          onToggleMic={handleToggleMic}
          onSendMessage={handleSendMessage}
          onToggleChat={() => setIsChatOpen(!isChatOpen)}
          isChatOpen={isChatOpen}
          chatCount={messages.length}
        />

        {/* 5. Модальное окно настроек Grok API */}
        <SettingsModal
          isOpen={isSettingsOpen}
          onClose={() => setIsSettingsOpen(false)}
          hasGrokKey={hasGrokKey}
          onKeySaved={() => setHasGrokKey(true)}
        />
      </main>
    </div>
  );
}

export default function App() {
  return (
    <ThemeProvider>
      <JarvisApp />
    </ThemeProvider>
  );
}
