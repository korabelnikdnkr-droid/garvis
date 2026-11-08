// src/components/SettingsModal.tsx
/**
 * Модальное окно настроек Джарвис.
 * Позволяет ввести GROK_API_KEY, выполнить легковесный Ping-тест к xAI API
 * и сохранить ключ в защищенное системное хранилище Windows Credential Manager.
 */

import React, { useState } from 'react';
import { motion } from 'motion/react';
import { X, Key, ShieldCheck, RefreshCw, CheckCircle, AlertTriangle, Eye, EyeOff } from 'lucide-react';
import { pingGrokKey, saveGrokKey } from '../services/tauriBridge';
import { useTheme } from '../context/ThemeContext';

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  hasGrokKey: boolean;
  onKeySaved: () => void;
}

export const SettingsModal: React.FC<SettingsModalProps> = ({
  isOpen,
  onClose,
  hasGrokKey,
  onKeySaved,
}) => {
  const { theme } = useTheme();
  const isJarvis = theme === 'jarvis';

  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [isPinging, setIsPinging] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [statusMessage, setStatusMessage] = useState<{
    type: 'success' | 'error' | 'info';
    text: string;
  } | null>(null);

  if (!isOpen) return null;

  // Обработка кнопки "Проверить" (Ping Test)
  const handlePingTest = async () => {
    if (!apiKey.trim()) {
      setStatusMessage({
        type: 'error',
        text: 'Пожалуйста, введите ключ xAI Grok для проверки',
      });
      return;
    }

    setIsPinging(true);
    setStatusMessage({ type: 'info', text: 'Отправка легковесного запроса к xAI Grok...' });

    const result = await pingGrokKey(apiKey.trim());
    setIsPinging(false);

    if (result.success) {
      setStatusMessage({
        type: 'success',
        text: 'Успешно: API-ключ валиден, соединение с xAI установлено',
      });
    } else {
      setStatusMessage({
        type: 'error',
        text: `Ошибка валидации: ${result.message}`,
      });
    }
  };

  // Обработка кнопки "Сохранить"
  const handleSave = async () => {
    if (!apiKey.trim()) {
      setStatusMessage({
        type: 'error',
        text: 'Ключ не может быть пустым',
      });
      return;
    }

    setIsSaving(true);
    const result = await saveGrokKey(apiKey.trim());
    setIsSaving(false);

    if (result.success) {
      setStatusMessage({
        type: 'success',
        text: 'Ключ надежно сохранен в Windows Credential Manager',
      });
      onKeySaved();
      setTimeout(() => {
        onClose();
      }, 1200);
    } else {
      setStatusMessage({
        type: 'error',
        text: result.message,
      });
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-md animate-fade-in">
      <motion.div
        initial={{ scale: 0.9, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        exit={{ scale: 0.9, opacity: 0 }}
        className={`w-full max-w-sm rounded-2xl border p-4 shadow-2xl relative ${
          isJarvis
            ? 'bg-slate-950 border-cyan-700/50 text-slate-100 shadow-[0_0_40px_rgba(6,182,212,0.25)]'
            : 'bg-slate-900/90 border-white/30 text-white shadow-[0_0_40px_rgba(255,255,255,0.2)]'
        }`}
      >
        {/* Шапка модального окна */}
        <div className="flex items-center justify-between pb-3 border-b border-white/10">
          <div className="flex items-center space-x-2">
            <Key className="w-4 h-4 text-cyan-400" />
            <h3 className="text-xs font-mono font-bold uppercase tracking-wider text-cyan-300">
              Настройки Grok API
            </h3>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded-lg hover:bg-white/10 text-slate-400 hover:text-white transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Текущий статус ключа */}
        <div className="my-3 flex items-center justify-between p-2.5 rounded-xl bg-white/5 border border-white/10 text-xs">
          <div className="flex items-center space-x-2">
            <ShieldCheck
              className={`w-4 h-4 ${hasGrokKey ? 'text-emerald-400' : 'text-amber-400'}`}
            />
            <span className="text-slate-300">Состояние хранилища:</span>
          </div>
          <span
            className={`font-mono text-[11px] font-semibold px-2 py-0.5 rounded ${
              hasGrokKey
                ? 'bg-emerald-950/80 text-emerald-300 border border-emerald-500/40'
                : 'bg-amber-950/80 text-amber-300 border border-amber-500/40'
            }`}
          >
            {hasGrokKey ? 'Ключ сохранен' : 'Ключ не задан'}
          </span>
        </div>

        {/* Поле ввода ключа */}
        <div className="space-y-1.5 mb-3">
          <label className="text-[11px] text-slate-400 font-mono flex items-center justify-between">
            <span>xAI GROK API KEY</span>
            <span className="text-[10px] text-slate-500">api.x.ai</span>
          </label>
          <div className="relative">
            <input
              type={showKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="xai-xxxxxxxxxxxxxxxxxxxx"
              className={`w-full py-2 pl-3 pr-9 text-xs rounded-xl border font-mono transition-colors focus:outline-none ${
                isJarvis
                  ? 'bg-slate-900 border-slate-700 text-slate-100 placeholder-slate-600 focus:border-cyan-500'
                  : 'bg-white/10 border-white/20 text-white placeholder-white/40 focus:border-white/50'
              }`}
            />
            <button
              type="button"
              onClick={() => setShowKey(!showKey)}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-400 hover:text-white"
            >
              {showKey ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
            </button>
          </div>
        </div>

        {/* Статусное сообщение */}
        {statusMessage && (
          <div
            className={`p-2 rounded-xl mb-3 text-[11px] flex items-start space-x-2 border leading-snug ${
              statusMessage.type === 'success'
                ? 'bg-emerald-950/70 border-emerald-500/40 text-emerald-300'
                : statusMessage.type === 'error'
                ? 'bg-rose-950/70 border-rose-500/40 text-rose-300'
                : 'bg-cyan-950/70 border-cyan-500/40 text-cyan-300'
            }`}
          >
            {statusMessage.type === 'success' ? (
              <CheckCircle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
            ) : statusMessage.type === 'error' ? (
              <AlertTriangle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
            ) : (
              <RefreshCw className="w-3.5 h-3.5 mt-0.5 shrink-0 animate-spin" />
            )}
            <span>{statusMessage.text}</span>
          </div>
        )}

        {/* Кнопки действий */}
        <div className="grid grid-cols-2 gap-2 pt-1">
          {/* Кнопка "Проверить" */}
          <button
            onClick={handlePingTest}
            disabled={isPinging || isSaving}
            className={`py-2 px-3 rounded-xl border text-xs font-semibold flex items-center justify-center space-x-1.5 transition-all duration-200 hover:scale-[1.02] active:scale-[0.98] ${
              isJarvis
                ? 'bg-slate-900 border-cyan-800/60 text-cyan-300 hover:bg-cyan-950'
                : 'bg-white/10 border-white/20 text-white hover:bg-white/20'
            }`}
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isPinging ? 'animate-spin' : ''}`} />
            <span>{isPinging ? 'Проверка...' : 'Проверить'}</span>
          </button>

          {/* Кнопка "Сохранить" */}
          <button
            onClick={handleSave}
            disabled={isSaving || isPinging}
            className="py-2 px-3 rounded-xl bg-cyan-500 hover:bg-cyan-400 text-slate-950 text-xs font-bold flex items-center justify-center space-x-1.5 shadow-[0_0_15px_rgba(6,182,212,0.4)] transition-all duration-200 hover:scale-[1.02] active:scale-[0.98]"
          >
            <ShieldCheck className="w-3.5 h-3.5" />
            <span>{isSaving ? 'Запись...' : 'Сохранить'}</span>
          </button>
        </div>
      </motion.div>
    </div>
  );
};
