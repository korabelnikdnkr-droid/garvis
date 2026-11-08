// src/services/voiceService.ts
/**
 * Сервис голосового ввода (Web Speech API) и синтеза речи (SpeechSynthesis).
 * Обеспечивает полноценную работу микрофона и голоса ассистента в браузере и WebView2.
 */

// Типизация интерфейса Web Speech API
interface IWindowSpeechRecognition extends EventTarget {
  continuous: boolean;
  interimResults: boolean;
  lang: string;
  start: () => void;
  stop: () => void;
  abort: () => void;
  onstart: ((this: IWindowSpeechRecognition, ev: Event) => void) | null;
  onresult: ((this: IWindowSpeechRecognition, ev: SpeechRecognitionEvent) => void) | null;
  onerror: ((this: IWindowSpeechRecognition, ev: SpeechRecognitionErrorEvent) => void) | null;
  onend: ((this: IWindowSpeechRecognition, ev: Event) => void) | null;
}

interface SpeechRecognitionEvent extends Event {
  results: SpeechRecognitionResultList;
}

interface SpeechRecognitionErrorEvent extends Event {
  error: string;
}

declare global {
  interface Window {
    SpeechRecognition?: new () => IWindowSpeechRecognition;
    webkitSpeechRecognition?: new () => IWindowSpeechRecognition;
  }
}

class VoiceService {
  private recognition: IWindowSpeechRecognition | null = null;
  private isListening = false;
  private audioContext: AudioContext | null = null;
  private mediaStream: MediaStream | null = null;
  private analyser: AnalyserNode | null = null;
  private animFrameId: number | null = null;

  constructor() {
    this.initRecognition();
  }

  private initRecognition() {
    const SpeechRecognitionClass = window.SpeechRecognition || window.webkitSpeechRecognition;
    if (SpeechRecognitionClass) {
      try {
        this.recognition = new SpeechRecognitionClass();
        this.recognition.continuous = false;
        this.recognition.interimResults = true;
        this.recognition.lang = 'ru-RU';
      } catch (err) {
        console.warn('Ошибка инициализации SpeechRecognition:', err);
      }
    }
  }

  /**
   * Запуск прослушивания микрофона и транскрипции русской речи
   */
  public startListening(
    onCommand: (text: string) => void,
    onPartial?: (text: string) => void,
    onError?: (error: string) => void
  ): boolean {
    if (!this.recognition) {
      this.initRecognition();
      if (!this.recognition) {
        if (onError) onError('Web Speech API не поддерживается в этом браузере.');
        return false;
      }
    }

    try {
      this.isListening = true;

      this.recognition.onresult = (event: SpeechRecognitionEvent) => {
        let interim = '';
        let final = '';

        for (let i = 0; i < event.results.length; i++) {
          const item = event.results[i];
          if (item.isFinal) {
            final += item[0].transcript;
          } else {
            interim += item[0].transcript;
          }
        }

        if (interim && onPartial) {
          onPartial(interim);
        }

        if (final && final.trim()) {
          this.isListening = false;
          onCommand(final.trim());
        }
      };

      this.recognition.onerror = (e: SpeechRecognitionErrorEvent) => {
        console.warn('Ошибка распознавания речи:', e.error);
        this.isListening = false;
        if (onError) onError(e.error);
      };

      this.recognition.onend = () => {
        this.isListening = false;
      };

      this.recognition.start();
      return true;
    } catch (err) {
      console.warn('Не удалось запустить распознавание:', err);
      this.isListening = false;
      if (onError) onError('Сбой запуска микрофона');
      return false;
    }
  }

  /**
   * Остановка прослушивания
   */
  public stopListening() {
    this.isListening = false;
    if (this.recognition) {
      try {
        this.recognition.stop();
      } catch {
        // Игнорируем
      }
    }
    this.stopAudioLevelTracking();
  }

  /**
   * Захват реального уровня громкости микрофона (RMS) для живой анимации голосовой сферы
   */
  public async startAudioLevelTracking(onLevel: (level: number) => void): Promise<boolean> {
    try {
      if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
        return false;
      }

      this.mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const AudioCtx = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
      this.audioContext = new AudioCtx();
      const source = this.audioContext.createMediaStreamSource(this.mediaStream);
      this.analyser = this.audioContext.createAnalyser();
      this.analyser.fftSize = 256;
      source.connect(this.analyser);

      const dataArray = new Uint8Array(this.analyser.frequencyBinCount);

      const checkVolume = () => {
        if (!this.analyser) return;
        this.analyser.getByteFrequencyData(dataArray);

        let sum = 0;
        for (let i = 0; i < dataArray.length; i++) {
          sum += dataArray[i];
        }
        const average = sum / dataArray.length;
        const normalized = Math.min(1, Math.max(0, average / 100));
        onLevel(normalized);

        this.animFrameId = requestAnimationFrame(checkVolume);
      };

      checkVolume();
      return true;
    } catch (err) {
      console.warn('Не удалось получить доступ к микрофону для замера уровня:', err);
      return false;
    }
  }

  public stopAudioLevelTracking() {
    if (this.animFrameId) {
      cancelAnimationFrame(this.animFrameId);
      this.animFrameId = null;
    }
    if (this.mediaStream) {
      this.mediaStream.getTracks().forEach((track) => track.stop());
      this.mediaStream = null;
    }
    if (this.audioContext && this.audioContext.state !== 'closed') {
      this.audioContext.close().catch(() => {});
      this.audioContext = null;
    }
    this.analyser = null;
  }

  /**
   * Синтез речи (TTS) через Web SpeechSynthesis с русским голосом
   */
  public speak(
    text: string,
    onStart?: () => void,
    onEnd?: () => void,
    onError?: (err: unknown) => void
  ) {
    if (!('speechSynthesis' in window)) {
      if (onEnd) onEnd();
      return;
    }

    try {
      // Прерываем предыдущую речь (Barge-in)
      window.speechSynthesis.cancel();

      // Очистка текста от технических спецсимволов перед озвучкой
      const cleanText = text
        .replace(/[*_#`~[\]()]/g, '')
        .replace(/https?:\/\/\S+/g, 'ссылка')
        .trim();

      if (!cleanText) {
        if (onEnd) onEnd();
        return;
      }

      const utterance = new SpeechSynthesisUtterance(cleanText);
      utterance.lang = 'ru-RU';
      utterance.rate = 1.05;
      utterance.pitch = 0.95;

      // Поиск наилучшего русского голоса
      const voices = window.speechSynthesis.getVoices();
      const ruVoice =
        voices.find((v) => v.lang.startsWith('ru') && (v.name.includes('Dmitry') || v.name.includes('Yuri') || v.name.includes('Google'))) ||
        voices.find((v) => v.lang.startsWith('ru'));

      if (ruVoice) {
        utterance.voice = ruVoice;
      }

      utterance.onstart = () => {
        if (onStart) onStart();
      };

      utterance.onend = () => {
        if (onEnd) onEnd();
      };

      utterance.onerror = (e) => {
        // В headless-браузерах без звукового оборудования это ожидаемо
        if (onEnd) onEnd();
        if (onError) onError(e);
      };

      window.speechSynthesis.speak(utterance);
    } catch (err) {
      console.warn('Сбой голосового синтеза:', err);
      if (onEnd) onEnd();
    }
  }

  /**
   * Принудительная остановка речи (Barge-in)
   */
  public stopSpeaking() {
    if ('speechSynthesis' in window) {
      window.speechSynthesis.cancel();
    }
  }
}

export const voiceService = new VoiceService();
