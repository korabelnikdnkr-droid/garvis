// src/components/ChatOverlay.tsx
/**
 * Выдвижная панель истории диалога и журнала выполненных системных команд.
 * Отображает реплики пользователя, ответы Grok и плашки выполнения системных инструментов Windows.
 */

import React, { useRef, useEffect } from 'react';
import { motion } from 'motion/react';
import { X, Trash2, Cpu, User, Bot, CheckCircle2, AlertCircle } from 'lucide-react';
import { ChatMessage } from '../types';
import { useTheme } from '../context/ThemeContext';

interface ChatOverlayProps {
  isOpen: boolean;
  onClose: () => void;
  messages: ChatMessage[];
  onClear: () => void;
}

export const ChatOverlay: React.FC<ChatOverlayProps> = ({
  isOpen,
  onClose,
  messages,
  onClear,
}) => {
  const { theme } = useTheme();
  const isJarvis = theme === 'jarvis';
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, isOpen]);

  if (!isOpen) return null;

  return (
    <motion.div
      initial={{ opacity: 0, y: 30 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 30 }}
      transition={{ duration: 0.25, ease: 'easeOut' }}
      className={`absolute inset-x-2 top-14 bottom-16 z-30 rounded-2xl border flex flex-col overflow-hidden shadow-2xl backdrop-blur-2xl ${
        isJarvis
          ? 'bg-slate-950/95 border-cyan-800/40 text-slate-100 shadow-[0_10px_35px_rgba(0,0,0,0.8)]'
          : 'bg-slate-900/80 border-white/30 text-white shadow-[0_10px_35px_rgba(0,0,0,0.5)]'
      }`}
    >
      {/* Шапка истории */}
      <div
        className={`px-3 py-2.5 flex items-center justify-between border-b ${
          isJarvis ? 'border-cyan-900/30 bg-slate-900/50' : 'border-white/10 bg-white/5'
        }`}
      >
        <div className="flex items-center space-x-2">
          <Bot className="w-4 h-4 text-cyan-400" />
          <span className="text-xs font-mono font-semibold tracking-wider text-cyan-300">
            ЖУРНАЛ СОБЫТИЙ И ДИАЛОГА
          </span>
        </div>
        <div className="flex items-center space-x-1">
          {messages.length > 0 && (
            <button
              onClick={onClear}
              title="Очистить историю"
              className="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-rose-400 transition-colors"
            >
              <Trash2 className="w-3.5 h-3.5" />
            </button>
          )}
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-white/10 text-slate-400 hover:text-white transition-colors"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Список сообщений */}
      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto p-3 space-y-3 custom-scrollbar text-xs"
      >
        {messages.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center text-center p-4 text-slate-400">
            <Cpu className="w-8 h-8 text-cyan-400/50 mb-2 animate-pulse" />
            <p className="font-mono text-xs text-slate-300">Журнал пуст</p>
            <p className="text-[11px] text-slate-500 mt-1">
              Произнесите «Джарвис» или введите команду в поле ниже.
            </p>
          </div>
        ) : (
          messages.map((msg) => (
            <div
              key={msg.id}
              className={`flex flex-col space-y-1 ${
                msg.role === 'user' ? 'items-end' : 'items-start'
              }`}
            >
              <div className="flex items-center space-x-1 text-[10px] text-slate-400 px-1 font-mono">
                {msg.role === 'user' ? (
                  <>
                    <span>Вы</span>
                    <User className="w-3 h-3 text-cyan-400" />
                  </>
                ) : (
                  <>
                    <Bot className="w-3 h-3 text-cyan-400" />
                    <span>Джарвис</span>
                  </>
                )}
                <span>• {msg.timestamp}</span>
              </div>

              {/* Текст реплики */}
              <div
                className={`max-w-[85%] rounded-2xl px-3 py-2 leading-relaxed whitespace-pre-wrap break-words ${
                  msg.role === 'user'
                    ? isJarvis
                      ? 'bg-cyan-600/30 text-cyan-100 border border-cyan-500/40 rounded-br-xs'
                      : 'bg-white/20 text-white border border-white/30 rounded-br-xs'
                    : isJarvis
                    ? 'bg-slate-900/90 text-slate-200 border border-slate-800 rounded-bl-xs'
                    : 'bg-white/10 text-slate-100 border border-white/15 rounded-bl-xs'
                }`}
              >
                {msg.text}

                {/* Плашка вызова системного инструмента Windows (если есть) */}
                {msg.toolCall && (
                  <div className="mt-2 pt-2 border-t border-cyan-500/20 flex flex-col space-y-1 font-mono text-[10px]">
                    <div className="flex items-center space-x-1.5 text-cyan-300">
                      <Cpu className="w-3 h-3" />
                      <span className="font-semibold uppercase">{msg.toolCall.name}</span>
                      {msg.toolCall.status === 'success' ? (
                        <CheckCircle2 className="w-3 h-3 text-emerald-400" />
                      ) : (
                        <AlertCircle className="w-3 h-3 text-rose-400" />
                      )}
                    </div>
                    <p className="text-slate-300">{msg.toolCall.description}</p>
                    {msg.toolCall.result && (
                      <p className="text-cyan-400/90 bg-slate-950/60 p-1 rounded font-mono">
                        Результат: {msg.toolCall.result}
                      </p>
                    )}
                  </div>
                )}
              </div>
            </div>
          ))
        )}
      </div>
    </motion.div>
  );
};
