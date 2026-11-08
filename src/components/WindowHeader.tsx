// src/components/WindowHeader.tsx
/**
 * Верхняя панель безрамочного окна Джарвис.
 * Содержит область перетаскивания окна (drag region), переключение Always on Top,
 * смену тем оформления, вызов настроек и сворачивание в системный трей.
 */

import React from 'react';
import {
  Pin,
  PinOff,
  Minimize2,
  Settings,
  Sparkles,
  Layers,
  X,
  Volume2,
} from 'lucide-react';
import { useTheme } from '../context/ThemeContext';
import { startWindowDragging, minimizeToTray, toggleAlwaysOnTop } from '../services/tauriBridge';

interface WindowHeaderProps {
  alwaysOnTop: boolean;
  onToggleAlwaysOnTop: (value: boolean) => void;
  onOpenSettings: () => void;
  hasGrokKey: boolean;
}

export const WindowHeader: React.FC<WindowHeaderProps> = ({
  alwaysOnTop,
  onToggleAlwaysOnTop,
  onOpenSettings,
  hasGrokKey,
}) => {
  const { theme, toggleTheme } = useTheme();
  const isJarvis = theme === 'jarvis';

  const handleDrag = () => {
    startWindowDragging();
  };

  const handleAlwaysOnTop = async () => {
    const updated = await toggleAlwaysOnTop(!alwaysOnTop);
    onToggleAlwaysOnTop(updated);
  };

  const handleMinimize = async () => {
    await minimizeToTray();
  };

  return (
    <header
      data-tauri-drag-region
      onMouseDown={handleDrag}
      className={`h-12 w-full flex items-center justify-between px-3 border-b select-none transition-colors duration-500 cursor-grab active:cursor-grabbing ${
        isJarvis
          ? 'bg-slate-950/80 border-cyan-900/30 text-slate-200'
          : 'bg-white/15 border-white/20 text-slate-100 backdrop-blur-md'
      }`}
    >
      {/* Левая секция: Логотип и статус */}
      <div className="flex items-center space-x-2 pointer-events-none">
        <div className="relative flex items-center justify-center">
          <div
            className={`w-2.5 h-2.5 rounded-full ${
              hasGrokKey ? 'bg-cyan-400' : 'bg-amber-400'
            } animate-pulse`}
          />
          <div
            className={`absolute w-4 h-4 rounded-full ${
              hasGrokKey ? 'bg-cyan-400/30' : 'bg-amber-400/30'
            } animate-ping`}
          />
        </div>
        <span className="font-mono font-bold tracking-widest text-xs uppercase text-cyan-300">
          JARVIS v2.0
        </span>
        <span className="text-[10px] px-1.5 py-0.5 rounded bg-white/5 border border-white/10 font-mono text-slate-400">
          WIN11
        </span>
      </div>

      {/* Правая секция: Кнопки управления окном */}
      <div
        className="flex items-center space-x-1"
        onMouseDown={(e) => e.stopPropagation()} // Исключаем кнопки из drag-области
      >
        {/* Переключатель темы (Jarvis / Fluid Glass) */}
        <button
          onClick={toggleTheme}
          title={isJarvis ? 'Переключить на тему Fluid Glass' : 'Переключить на тему Jarvis'}
          className="p-1.5 rounded-md hover:bg-white/10 text-slate-300 hover:text-white transition-all duration-200 hover:scale-110 active:scale-95"
        >
          {isJarvis ? (
            <Layers className="w-4 h-4 text-cyan-400" />
          ) : (
            <Sparkles className="w-4 h-4 text-white" />
          )}
        </button>

        {/* Закрепление поверх всех окон */}
        <button
          onClick={handleAlwaysOnTop}
          title={alwaysOnTop ? 'Окно закреплено поверх всех окон' : 'Закрепить поверх всех окон'}
          className={`p-1.5 rounded-md transition-all duration-200 hover:scale-110 active:scale-95 ${
            alwaysOnTop
              ? isJarvis
                ? 'bg-cyan-950/80 text-cyan-300 border border-cyan-500/40 shadow-[0_0_8px_rgba(6,182,212,0.4)]'
                : 'bg-white/25 text-white border border-white/40'
              : 'hover:bg-white/10 text-slate-400 hover:text-slate-200'
          }`}
        >
          {alwaysOnTop ? <Pin className="w-4 h-4" /> : <PinOff className="w-4 h-4" />}
        </button>

        {/* Настройки (Ключ Grok API) */}
        <button
          onClick={onOpenSettings}
          title="Настройки ассистента и API ключи"
          className="relative p-1.5 rounded-md hover:bg-white/10 text-slate-400 hover:text-white transition-all duration-200 hover:scale-110 active:scale-95"
        >
          <Settings className="w-4 h-4" />
          {!hasGrokKey && (
            <span className="absolute top-1 right-1 w-2 h-2 rounded-full bg-amber-500" />
          )}
        </button>

        {/* Свернуть в системный трей */}
        <button
          onClick={handleMinimize}
          title="Свернуть Джарвис в системный трей Windows"
          className="p-1.5 rounded-md hover:bg-white/10 text-slate-400 hover:text-white transition-all duration-200 hover:scale-110 active:scale-95"
        >
          <Minimize2 className="w-4 h-4" />
        </button>

        {/* Закрыть / скрыть окно */}
        <button
          onClick={handleMinimize}
          title="Скрыть окно"
          className="p-1.5 rounded-md hover:bg-rose-500/20 text-slate-400 hover:text-rose-300 transition-all duration-200 hover:scale-110 active:scale-95"
        >
          <X className="w-4 h-4" />
        </button>
      </div>
    </header>
  );
};
