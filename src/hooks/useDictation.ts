import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { info as logInfo, attachConsole } from "@tauri-apps/plugin-log";
import { useI18n } from "../i18n";

let invokeSeq = 0;
const feLog = (msg: string) => {
  logInfo(`[fe] #${++invokeSeq} ${msg}`).catch(() => {});
};
import {
  EVENTS,
  type DownloadCancelled,
  type DownloadDone,
  type DownloadError,
  type DownloadProgress,
  type DownloadState,
  type FileError,
  type FileDuration,
  type FileStarted,
  type FileTask,
  type ModelId,
  type ModelInfo,
  type ModelStatus,
  type RecordingState,
} from "../types";

export interface DictationState {
  recordingState: RecordingState;
  transcript: string;
  interim: string;
  level: number;
  models: ModelInfo[];
  currentModel: ModelId;
  modelStatus: ModelStatus | null;
  downloads: Record<string, DownloadState>;
  fileTask: FileTask | null;
  error: string | null;
}

export interface DictationActions {
  start: () => void;
  stop: () => void;
  setLocked: (locked: boolean) => void;
  cancel: () => void;
  selectModel: (model: ModelId) => void;
  downloadModel: (model: ModelId) => void;
  cancelDownload: (model: ModelId) => void;
  deleteModel: (model: ModelId) => void;
  transcribeFile: (path: string) => void;
  cancelFile: () => void;
  copy: () => Promise<boolean>;
  clear: () => void;
  editTranscript: (text: string) => void;
  dismissError: () => void;
}

export function useDictation(): DictationState & DictationActions {
  const { t } = useI18n();
  const [recordingState, setRecordingState] = useState<RecordingState>("idle");
  const [transcript, setTranscript] = useState("");
  const [interim, setInterim] = useState("");
  const [level, setLevel] = useState(0);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [currentModel, setCurrentModel] = useState<ModelId>("large-v3-turbo");
  const [modelStatus, setModelStatus] = useState<ModelStatus | null>(null);
  const [downloads, setDownloads] = useState<Record<string, DownloadState>>({});
  const [fileTask, setFileTask] = useState<FileTask | null>(null);
  const [error, setError] = useState<string | null>(null);

  const activeRef = useRef(false);
  const fileAnimRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const fileAnimStartRef = useRef<number>(0);
  const fileAnimTauRef = useRef<number>(10_000);
  const fileDoneTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    let unlisteners: UnlistenFn[] = [];
    let detachConsole: UnlistenFn | undefined;
    let cancelled = false;

    attachConsole()
      .then((d) => {
        if (cancelled) d();
        else detachConsole = d;
      })
      .catch(() => {});

    (async () => {
      const subs = await Promise.all([
        listen<RecordingState>(EVENTS.recordingState, (e) => {
          setRecordingState(e.payload);
          activeRef.current = e.payload === "recording" || e.payload === "locked";
          if (e.payload === "idle") setInterim("");
        }),
        listen<string>(EVENTS.interim, (e) => setInterim(e.payload)),
        listen<string>(EVENTS.final, (e) => {
          setInterim("");
          const text = e.payload.trim();
          if (text) {
            setTranscript((prev) => (prev ? `${prev} ${text}` : text));
          }
        }),
        listen<ModelStatus>(EVENTS.modelStatus, (e) => {
          setModelStatus(e.payload);
          if (e.payload.loaded) refreshModels();
        }),
        listen<number>(EVENTS.audioLevel, (e) => setLevel(e.payload)),
        listen<string>(EVENTS.error, (e) => setError(e.payload)),
        listen<DownloadProgress>(EVENTS.downloadProgress, (e) => {
          setDownloads((prev) => ({
            ...prev,
            [e.payload.model]: { status: "downloading", pct: e.payload.pct },
          }));
        }),
        listen<DownloadDone>(EVENTS.downloadDone, (e) => {
          const model = e.payload.model;
          setDownloads((prev) => ({ ...prev, [model]: { status: "done", pct: 1 } }));
          refreshModels();
          selectModel(model);
        }),
        listen<DownloadError>(EVENTS.downloadError, (e) => {
          setDownloads((prev) => ({
            ...prev,
            [e.payload.model]: { status: "error", pct: null, error: e.payload.error },
          }));
          setError(e.payload.error);
        }),
        listen<DownloadCancelled>(EVENTS.downloadCancelled, (e) => {
          setDownloads((prev) => {
            const next = { ...prev };
            delete next[e.payload.model];
            return next;
          });
        }),
        listen<FileStarted>(EVENTS.fileStarted, (e) =>
          setFileTask({ name: e.payload.name, pct: 0 }),
        ),
        listen<FileDuration>(EVENTS.fileDuration, (e) => {
          const estimated = Math.max(10, e.payload.duration_secs / 3);
          const tau = (estimated / Math.log(19)) * 1000;
          fileAnimStartRef.current = Date.now();
          fileAnimTauRef.current = tau;
          if (fileAnimRef.current !== null) clearInterval(fileAnimRef.current);
          fileAnimRef.current = setInterval(() => {
            const elapsed = Date.now() - fileAnimStartRef.current;
            const pct = Math.min(99, Math.round(95 * (1 - Math.exp(-elapsed / fileAnimTauRef.current))));
            setFileTask((prev) => (prev ? { ...prev, pct } : prev));
          }, 150);
        }),
        listen(EVENTS.fileDone, () => {
          if (fileAnimRef.current !== null) { clearInterval(fileAnimRef.current); fileAnimRef.current = null; }
          setFileTask((prev) => (prev ? { ...prev, pct: 100 } : null));
          fileDoneTimerRef.current = setTimeout(() => setFileTask(null), 600);
        }),
        listen(EVENTS.fileCancelled, () => {
          if (fileAnimRef.current !== null) { clearInterval(fileAnimRef.current); fileAnimRef.current = null; }
          setFileTask(null);
        }),
        listen<FileError>(EVENTS.fileError, (e) => {
          if (fileAnimRef.current !== null) { clearInterval(fileAnimRef.current); fileAnimRef.current = null; }
          setFileTask(null);
          setError(e.payload.error);
        }),
      ]);
      if (cancelled) {
        subs.forEach((u) => u());
      } else {
        unlisteners = subs;
      }
    })();

    refreshModels();
    invoke<ModelStatus>("get_model_status")
      .then((s) => setModelStatus(s))
      .catch(() => {});

    return () => {
      cancelled = true;
      unlisteners.forEach((u) => u());
      detachConsole?.();
      if (fileAnimRef.current !== null) clearInterval(fileAnimRef.current);
      if (fileDoneTimerRef.current !== null) clearTimeout(fileDoneTimerRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const refreshModels = useCallback(async () => {
    try {
      const list = await invoke<ModelInfo[]>("list_models");
      setModels(list);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const start = useCallback(() => {
    if (activeRef.current) {
      feLog("start_recording SUPPRESSED (already active)");
      return;
    }
    activeRef.current = true;
    feLog("start_recording → invoke");
    invoke("start_recording").catch((e) => setError(String(e)));
  }, []);

  const stop = useCallback(() => {
    activeRef.current = false;
    feLog("stop_recording → invoke");
    invoke("stop_recording").catch((e) => setError(String(e)));
  }, []);

  const setLocked = useCallback((locked: boolean) => {
    feLog(`set_locked(${locked}) → invoke`);
    invoke("set_locked", { locked }).catch((e) => setError(String(e)));
  }, []);

  const cancel = useCallback(() => {
    activeRef.current = false;
    feLog("cancel_recording → invoke");
    invoke("cancel_recording").catch((e) => setError(String(e)));
  }, []);

  const selectModel = useCallback((model: ModelId) => {
    setCurrentModel(model);
    invoke("set_model", { model }).catch((e) => setError(String(e)));
  }, []);

  const downloadModel = useCallback((model: ModelId) => {
    setDownloads((prev) => ({ ...prev, [model]: { status: "downloading", pct: 0 } }));
    invoke("download_model", { model }).catch((e) => setError(String(e)));
  }, []);

  const cancelDownload = useCallback((model: ModelId) => {
    invoke("cancel_download", { model }).catch((e) => setError(String(e)));
  }, []);

  const deleteModel = useCallback(
    (model: ModelId) => {
      invoke("delete_model", { model })
        .then(() => refreshModels())
        .catch((e) => setError(String(e)));
    },
    [refreshModels],
  );

  const transcribeFile = useCallback((path: string) => {
    const name = path.split("/").pop()?.split("\\").pop() || t.file.fallbackName;
    setFileTask({ name, pct: 0 });
    invoke("transcribe_file", { path }).catch((e) => {
      setFileTask(null);
      setError(String(e));
    });
  }, [t]);

  const cancelFile = useCallback(() => {
    invoke("cancel_file_transcription").catch((e) => setError(String(e)));
  }, []);

  const copy = useCallback(async () => {
    if (!transcript) return false;
    try {
      await writeText(transcript);
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    }
  }, [transcript]);

  const clear = useCallback(() => {
    setTranscript("");
    setInterim("");
  }, []);

  const editTranscript = useCallback((text: string) => setTranscript(text), []);

  const dismissError = useCallback(() => setError(null), []);

  return {
    recordingState,
    transcript,
    interim,
    level,
    models,
    currentModel,
    modelStatus,
    downloads,
    fileTask,
    error,
    start,
    stop,
    setLocked,
    cancel,
    selectModel,
    downloadModel,
    cancelDownload,
    deleteModel,
    transcribeFile,
    cancelFile,
    copy,
    clear,
    editTranscript,
    dismissError,
  };
}
