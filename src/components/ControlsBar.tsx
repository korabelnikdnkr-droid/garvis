// src/components/ControlsBar.tsx
/**
 * Нижняя панель управления ассистентом:
 * - Текстовое поле ввода для ручного ввода команд без микрофона (Enter для отправки).
 * - Кнопка переключения микрофона (активация голосового слушания).
 * - Кнопка вызова журнала диалога и выполненных системных инструментов.
 */

import React, { useState } from 'react';
import { Mic, MicOff, Send, MessageSquareText, Terminal } from 'lucide-react';
import { AssistantState } from '../types';
import { useTheme } from '../context/ThemeContext';

interface ControlsBarProps {
  state: AssistantState;
  onToggleMic: () => void;
  onSendMessage: (text: string) => void;
  onToggleChat: () => void;
  isChatOpen: boolean;
  chatCount: number;
}

export const ControlsBar: React.FC<ControlsBarProps> = ({
  state,
  onToggleMic,
  onSendMessage,
  onToggleChat,
  isChatOpen,
  chatCount,
}) => {
  const [inputText, setInputText] = useState('');
  const { theme } = useTheme();
  const isJarvis = theme === 'jarvis';
  const isListening = state === 'listening';

  const handleSend = () => {
    if (!inputText.trim()) return;
    onSendMessage(inputText.trim());
    setInputText('');
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div
      className={`p-3 border-t transition-colors duration-500 ${
        isJarvis
          ? 'bg-slate-950/90 border-cyan-900/30'
          : 'bg-white/10 border-white/20 backdrop-blur-lg'
      }`}
    >
      <div className="flex items-center space-x-2">
        {/* Кнопка открытия истории диалога / журнала команд */}
        <button
          onClick={onToggleChat}
          title={isChatOpen ? 'Закрыть историю' : 'Открыть историю диалога'}
          className={`relative p-2.5 rounded-xl border transition-all duration-300 hover:scale-105 active:scale-95 ${
            isChatOpen
              ? isJarvis
                ? 'bg-cyan-950 text-cyan-300 border-cyan-500/50 shadow-[0_0_12px_rgba(6,182,212,0.4)]'
                : 'bg-white/30 text-white border-white/50'
              : isJarvis
              ? 'bg-slate-900/80 text-slate-400 hover:text-slate-200 border-slate-800'
              : 'bg-white/10 text-white/70 hover:text-white border-white/10'
          }`}
        >
          <MessageSquareText className="w-4 h-4" />
          {chatCount > 0 && (
            <span className="absolute -top-1 -right-1 w-4 h-4 rounded-full bg-cyan-500 text-[9px] font-bold text-slate-950 flex items-center justify-center">
              {chatCount}
            </span>
          )}
        </button>

        {/* Текстовое поле ввода команд */}
        <div className="relative flex-1">
          <input
            type="text"
            value={inputText}
            onChange={(e) => setInputText(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={
              state === 'thinking'
                ? 'Джарвис обрабатывает запрос...'
                : 'Введите команду или вопрос...'
            }
            disabled={state === 'thinking'}
            className={`w-full py-2 pl-3 pr-9 text-xs rounded-xl border transition-all duration-300 focus:outline-none ${
              isJarvis
                ? 'bg-slate-900/90 border-slate-800 text-slate-100 placeholder-slate-500 focus:border-cyan-500 focus:shadow-[0_0_15px_rgba(6,182,212,0.3)]'
                : 'bg-white/15 border-white/25 text-white placeholder-white/50 focus:border-white/60 focus:bg-white/20'
            }`}
          />
          {inputText.trim() && (
            <button
              onClick={handleSend}
              className="absolute right-1.5 top-1/2 -translate-y-1/2 p-1 rounded-lg text-cyan-400 hover:text-cyan-200 transition-all hover:scale-110 active:scale-90"
              title="Отправить команду"
            >
              <Send className="w-3.5 h-3.5" />
            </button>
          )}
        </div>

        {/* Кнопка микрофона */}
        <button
          onClick={onToggleMic}
          title={isListening ? 'Выключить микрофон' : 'Включить голосовой ввод'}
          className={`relative p-3 rounded-xl border transition-all duration-300 hover:scale-110 active:scale-90 ${
            isListening
              ? isJarvis
                ? 'bg-cyan-500 text-slate-950 border-cyan-400 shadow-[0_0_20px_rgba(6,182,212,0.8)] animate-pulse'
                : 'bg-white text-slate-900 border-white shadow-[0_0_20px_rgba(255,255,255,0.8)] animate-pulse'
              : isJarvis
              ? 'bg-slate-900/80 text-slate-300 hover:text-cyan-300 border-slate-800 hover:border-cyan-500/50'
              : 'bg-white/10 text-white/80 hover:text-white border-white/20 hover:border-white/40'
          }`}
        >
          {isListening ? (
            <Mic className="w-4 h-4 animate-bounce" />
          ) : (
            <MicOff className="w-4 h-4" />
          )}
        </button>
      </div>
    </div>
  );
};
