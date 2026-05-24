export type Lang = "en" | "ru";

export interface Translations {
  status: {
    idle: string;
    recording: string;
    locked: string;
    finalizing: string;
  };
  fileDialog: { mediaLabel: string };
  file: {
    decoding: string;
    recognizing: (pct: number) => string;
    cancel: string;
    fallbackName: string;
  };
  stage: {
    hintReady: string;
    hintNoModel: string;
    cancel: string;
    finish: string;
    fileBtn: string;
  };
  dropzone: string;
  update: {
    label: (version: string) => string;
    install: string;
    installing: string;
    dismiss: string;
  };
  mic: {
    sealTitle: string;
    ariaActive: string;
    ariaIdle: string;
  };
  transcript: {
    placeholder: string;
    clear: string;
    copy: string;
    copied: string;
  };
  model: {
    recommended: string;
    deleteConfirm: string;
    deleteTitle: string;
    downloading: string;
    cancelDownload: string;
    retry: string;
    download: string;
    hint: string;
  };
  settings: {
    theme: string;
    language: string;
  };
}

export const en: Translations = {
  status: {
    idle: "silence",
    recording: "recording",
    locked: "locked",
    finalizing: "transcribing",
  },
  fileDialog: { mediaLabel: "Audio & Video" },
  file: {
    decoding: "decoding…",
    recognizing: (pct) => `transcribing ${pct}%`,
    cancel: "cancel",
    fallbackName: "file",
  },
  stage: {
    hintReady: "hold · drag up to lock",
    hintNoModel: "download a model to start",
    cancel: "cancel",
    finish: "finish",
    fileBtn: "or transcribe a file",
  },
  dropzone: "drop file to transcribe",
  update: {
    label: (v) => `Update ${v}`,
    install: "install",
    installing: "installing…",
    dismiss: "Close",
  },
  mic: {
    sealTitle: "locked",
    ariaActive: "Recording",
    ariaIdle: "Hold to speak",
  },
  transcript: {
    placeholder: "hold the dot or Space and speak · text can be edited",
    clear: "clear",
    copy: "copy ⌘C",
    copied: "copied ✓",
  },
  model: {
    recommended: "recommended",
    deleteConfirm: "delete?",
    deleteTitle: "Delete model",
    downloading: "downloading…",
    cancelDownload: "Cancel download",
    retry: "Retry",
    download: "Download",
    hint: "models stored locally, work offline",
  },
  settings: {
    theme: "Theme",
    language: "Language",
  },
};

export const ru: Translations = {
  status: {
    idle: "тишина",
    recording: "запись",
    locked: "зафиксировано",
    finalizing: "распознаю",
  },
  fileDialog: { mediaLabel: "Аудио и видео" },
  file: {
    decoding: "декодирование…",
    recognizing: (pct) => `распознавание ${pct}%`,
    cancel: "отмена",
    fallbackName: "файл",
  },
  stage: {
    hintReady: "удерживайте · тяните вверх для фиксации",
    hintNoModel: "скачайте модель, чтобы начать",
    cancel: "отмена",
    finish: "завершить",
    fileBtn: "или расшифровать файл",
  },
  dropzone: "отпустите файл для расшифровки",
  update: {
    label: (v) => `Обновление ${v}`,
    install: "установить",
    installing: "установка…",
    dismiss: "Закрыть",
  },
  mic: {
    sealTitle: "зафиксировано",
    ariaActive: "Идёт запись",
    ariaIdle: "Удерживайте, чтобы говорить",
  },
  transcript: {
    placeholder: "удерживайте точку или Пробел и говорите · текст можно править руками",
    clear: "очистить",
    copy: "копировать ⌘C",
    copied: "скопировано ✓",
  },
  model: {
    recommended: "рекомендуется",
    deleteConfirm: "удалить?",
    deleteTitle: "Удалить модель",
    downloading: "загрузка…",
    cancelDownload: "Отменить загрузку",
    retry: "Повторить",
    download: "Скачать",
    hint: "модели хранятся локально и работают офлайн",
  },
  settings: {
    theme: "Тема",
    language: "Язык",
  },
};

export const locales: Record<Lang, Translations> = { en, ru };
