export type ModelId = "large-v3-turbo" | "large-v3";

export interface ModelInfo {
  id: ModelId;
  label: string;
  recommended: boolean;
  size: string;
  available: boolean;
}

export interface DownloadProgress {
  model: ModelId;
  received: number;
  total: number | null;
  pct: number | null;
}

export interface DownloadDone {
  model: ModelId;
}

export interface DownloadError {
  model: ModelId;
  error: string;
}

export interface DownloadCancelled {
  model: ModelId;
}

export interface DownloadState {
  status: "downloading" | "done" | "error";
  pct: number | null;
  error?: string;
}

export type RecordingState = "idle" | "recording" | "locked" | "finalizing";

export interface ModelStatus {
  model: string | null;
  loaded: boolean;
  error: string | null;
}

export const EVENTS = {
  recordingState: "recording-state",
  interim: "transcription-interim",
  final: "transcription-final",
  modelStatus: "model-status",
  audioLevel: "audio-level",
  error: "engine-error",
  downloadProgress: "model-download-progress",
  downloadDone: "model-download-done",
  downloadError: "model-download-error",
  downloadCancelled: "model-download-cancelled",
  fileStarted: "file-started",
  fileDuration: "file-duration",
  fileDone: "file-done",
  fileError: "file-error",
  fileCancelled: "file-cancelled",
} as const;

export interface FileStarted {
  name: string;
}

export interface FileDuration {
  duration_secs: number;
}

export interface FileError {
  error: string;
}

export interface FileTask {
  name: string;
  /** Transcription progress 0..100 (0 = decoding in progress). */
  pct: number;
}
