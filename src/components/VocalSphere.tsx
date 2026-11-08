// src/components/VocalSphere.tsx
/**
 * Центральный визуальный якорь — Интерактивная голосовая сфера Джарвиса.
 * Поддерживает 4 динамических состояния:
 * 1. Idling (Ожидание): медленный дыхательный импульс, вращающиеся орбитальные кольца.
 * 2. Listening (Слушаю): реактивное расширение сферы под громкость микрофона, расходящиеся звуковые волны.
 * 3. Thinking (Думаю): быстрый гироскопический спиннер, квантовые орбиты, импульсы обработки Grok.
 * 4. Speaking (Говорю): гармонические звуковые волны, синусоидальные пульсации в такт речи Edge-TTS.
 */

import React, { useMemo } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { AssistantState } from '../types';
import { useTheme } from '../context/ThemeContext';

interface VocalSphereProps {
  state: AssistantState;
  audioLevel?: number; // от 0.0 до 1.0 (громкость микрофона)
  onClick?: () => void;
}

export const VocalSphere: React.FC<VocalSphereProps> = ({
  state,
  audioLevel = 0.25,
  onClick,
}) => {
  const { theme } = useTheme();
  const isJarvis = theme === 'jarvis';

  // Динамические цветовые палитры для тем
  const colors = useMemo(() => {
    if (isJarvis) {
      return {
        coreGlow: 'rgba(0, 229, 255, 0.45)',
        coreCenter: 'from-cyan-400 via-blue-500 to-indigo-600',
        ringPrimary: 'border-cyan-400/60 shadow-[0_0_25px_rgba(6,182,212,0.6)]',
        ringSecondary: 'border-purple-500/50 shadow-[0_0_20px_rgba(168,85,247,0.5)]',
        particles: '#00e5ff',
        textBadge: 'text-cyan-300 bg-cyan-950/60 border-cyan-500/40',
      };
    } else {
      return {
        coreGlow: 'rgba(255, 255, 255, 0.55)',
        coreCenter: 'from-white via-cyan-200 to-teal-400',
        ringPrimary: 'border-white/70 shadow-[0_0_25px_rgba(255,255,255,0.7)]',
        ringSecondary: 'border-cyan-300/60 shadow-[0_0_20px_rgba(103,232,249,0.5)]',
        particles: '#ffffff',
        textBadge: 'text-white bg-white/10 border-white/30 backdrop-blur-md',
      };
    }
  }, [isJarvis]);

  // Подписи состояний на русском языке
  const stateLabels: Record<AssistantState, { title: string; subtitle: string }> = {
    idling: { title: 'ДЖАРВИС', subtitle: 'Ожидание фразы «Джарвис»' },
    listening: { title: 'СЛУШАЮ', subtitle: 'Прием голоса...' },
    thinking: { title: 'АНАЛИЗ', subtitle: 'Обработка запроса Grok...' },
    speaking: { title: 'ОТВЕТ', subtitle: 'Синтез речи...' },
  };

  // Вычисление масштаба при прослушивании на основе уровня звука
  const dynamicScale = state === 'listening' ? 1 + audioLevel * 0.45 : 1;

  return (
    <div
      onClick={onClick}
      className="relative flex flex-col items-center justify-center py-6 cursor-pointer select-none group"
      title="Нажмите для проверки или ручной смены состояния"
    >
      {/* Фоновое динамическое сияние */}
      <motion.div
        animate={{
          scale: state === 'listening' ? [1.1, 1.4 + audioLevel * 0.3, 1.1] : [1, 1.2, 1],
          opacity: state === 'idling' ? [0.35, 0.6, 0.35] : [0.6, 0.9, 0.6],
        }}
        transition={{
          duration: state === 'thinking' ? 1.2 : state === 'listening' ? 0.6 : 3,
          repeat: Infinity,
          ease: 'easeInOut',
        }}
        className={`absolute w-56 h-56 rounded-full blur-3xl pointer-events-none transition-colors duration-700 ${
          isJarvis
            ? state === 'thinking'
              ? 'bg-purple-600/30'
              : 'bg-cyan-500/30'
            : 'bg-white/25'
        }`}
      />

      {/* Контейнер интерактивной сферы */}
      <div className="relative w-44 h-44 flex items-center justify-center">
        {/* Внешнее орбитальное кольцо 1 */}
        <motion.div
          animate={{
            rotate: state === 'thinking' ? 720 : 360,
            scale: state === 'speaking' ? [1, 1.08, 0.98, 1.05] : dynamicScale,
          }}
          transition={{
            rotate: {
              duration: state === 'thinking' ? 4 : 20,
              repeat: Infinity,
              ease: 'linear',
            },
            scale: { duration: 0.8, repeat: Infinity, ease: 'easeInOut' },
          }}
          className={`absolute w-44 h-44 rounded-full border-2 border-dashed ${colors.ringSecondary} transition-all duration-500`}
        />

        {/* Промежуточное орбитальное кольцо 2 (вращение в противоположную сторону) */}
        <motion.div
          animate={{
            rotate: state === 'thinking' ? -720 : -360,
            scale: state === 'listening' ? [1, 1.15, 1] : 1,
          }}
          transition={{
            rotate: {
              duration: state === 'thinking' ? 3 : 15,
              repeat: Infinity,
              ease: 'linear',
            },
            scale: { duration: 0.4, repeat: Infinity, ease: 'easeInOut' },
          }}
          className={`absolute w-36 h-36 rounded-full border border-dotted ${colors.ringPrimary} transition-all duration-500`}
        />

        {/* Анимация расходящихся волн при состоянии Listening */}
        {state === 'listening' && (
          <motion.div
            initial={{ scale: 0.8, opacity: 0.9 }}
            animate={{ scale: 1.5, opacity: 0 }}
            transition={{ duration: 1.2, repeat: Infinity, ease: 'easeOut' }}
            className={`absolute w-32 h-32 rounded-full border border-cyan-400 pointer-events-none`}
          />
        )}

        {/* Анимация гармонических импульсов при состоянии Speaking */}
        {state === 'speaking' && (
          <motion.div
            animate={{
              scale: [1, 1.25, 1],
              opacity: [0.8, 0.2, 0.8],
            }}
            transition={{ duration: 0.8, repeat: Infinity, ease: 'easeInOut' }}
            className={`absolute w-36 h-36 rounded-full border-2 border-cyan-300/40 pointer-events-none`}
          />
        )}

        {/* Квантовые частицы в режиме Thinking */}
        {state === 'thinking' && (
          <motion.div
            animate={{ rotate: 360 }}
            transition={{ duration: 1.5, repeat: Infinity, ease: 'linear' }}
            className="absolute w-40 h-40 flex items-center justify-center pointer-events-none"
          >
            <span className="absolute top-0 w-2 h-2 rounded-full bg-cyan-300 shadow-[0_0_10px_#00e5ff]" />
            <span className="absolute bottom-0 w-2 h-2 rounded-full bg-purple-400 shadow-[0_0_10px_#c084fc]" />
            <span className="absolute left-0 w-2 h-2 rounded-full bg-white shadow-[0_0_10px_#ffffff]" />
            <span className="absolute right-0 w-2 h-2 rounded-full bg-cyan-400 shadow-[0_0_10px_#00e5ff]" />
          </motion.div>
        )}

        {/* Центральное плазменное ядро (Vocal Core) */}
        <motion.div
          animate={{
            scale:
              state === 'listening'
                ? [1, 1.15 + audioLevel * 0.35, 1.05]
                : state === 'thinking'
                ? [0.95, 1.08, 0.95]
                : state === 'speaking'
                ? [1, 1.12, 0.97, 1.08]
                : [0.96, 1.04, 0.96],
          }}
          transition={{
            duration: state === 'thinking' ? 0.6 : state === 'listening' ? 0.35 : 2.5,
            repeat: Infinity,
            ease: 'easeInOut',
          }}
          className={`relative w-28 h-28 rounded-full bg-gradient-to-tr ${colors.coreCenter} flex items-center justify-center shadow-2xl transition-transform duration-300 group-hover:scale-105`}
        >
          {/* Внутреннее свечение и стеклянный блик */}
          <div className="absolute inset-1 rounded-full bg-gradient-to-b from-white/40 via-transparent to-black/30 backdrop-blur-sm" />

          {/* Внутренний индикатор активности */}
          <div className="relative z-10 flex flex-col items-center justify-center">
            {state === 'thinking' ? (
              <div className="flex space-x-1">
                <motion.div
                  animate={{ y: [-4, 4, -4] }}
                  transition={{ duration: 0.6, repeat: Infinity, delay: 0 }}
                  className="w-1.5 h-4 bg-white rounded-full shadow-[0_0_8px_white]"
                />
                <motion.div
                  animate={{ y: [-4, 4, -4] }}
                  transition={{ duration: 0.6, repeat: Infinity, delay: 0.2 }}
                  className="w-1.5 h-6 bg-cyan-200 rounded-full shadow-[0_0_8px_white]"
                />
                <motion.div
                  animate={{ y: [-4, 4, -4] }}
                  transition={{ duration: 0.6, repeat: Infinity, delay: 0.4 }}
                  className="w-1.5 h-4 bg-white rounded-full shadow-[0_0_8px_white]"
                />
              </div>
            ) : state === 'listening' ? (
              <div className="flex items-center space-x-1">
                {[0.4, 0.8, 1, 0.7, 0.3].map((heightMul, i) => (
                  <motion.div
                    key={i}
                    animate={{
                      height: [
                        8,
                        Math.max(10, 28 * heightMul * (0.4 + audioLevel * 0.8)),
                        8,
                      ],
                    }}
                    transition={{
                      duration: 0.3 + i * 0.05,
                      repeat: Infinity,
                      ease: 'easeInOut',
                    }}
                    className="w-1 bg-white/90 rounded-full shadow-[0_0_6px_white]"
                  />
                ))}
              </div>
            ) : state === 'speaking' ? (
              <div className="flex items-center space-x-1">
                {[12, 22, 16, 26, 14].map((h, i) => (
                  <motion.div
                    key={i}
                    animate={{ height: [8, h, 8] }}
                    transition={{
                      duration: 0.45,
                      repeat: Infinity,
                      delay: i * 0.08,
                      ease: 'easeInOut',
                    }}
                    className="w-1 bg-white rounded-full shadow-[0_0_8px_white]"
                  />
                ))}
              </div>
            ) : (
              <div className="w-4 h-4 rounded-full bg-white/80 shadow-[0_0_12px_white] animate-pulse" />
            )}
          </div>
        </motion.div>
      </div>

      {/* Текстовый индикатор текущего состояния */}
      <div className="mt-4 flex flex-col items-center">
        <span
          className={`px-3 py-0.5 text-xs font-mono tracking-widest uppercase rounded-full border transition-all duration-300 ${colors.textBadge}`}
        >
          {stateLabels[state].title}
        </span>
        <span className="mt-1 text-xs text-white/60 font-sans tracking-wide">
          {stateLabels[state].subtitle}
        </span>
      </div>
    </div>
  );
};
