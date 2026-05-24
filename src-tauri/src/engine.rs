//! Dictation engine: a dedicated worker thread owning audio capture and the
//! whisper context, processing commands from the UI.
//!
//! Why a separate thread: cpal `Stream` is `!Send`, so it must be created and
//! dropped on the same thread. The worker owns it; the UI communicates via a
//! crossbeam channel (whose `Sender` is `Send + Sync`, suitable for Tauri state).
//!
//! Recording state machine: Idle → Recording (hold) → [Locked] → Finalizing → Idle.
//! The hold-vs-lock distinction lives in the frontend; the backend only reflects state.
//!
//! Transcription mode — hybrid: during recording, periodically run whisper on
//! the accumulated buffer and emit `transcription-interim`; on stop, run a final
//! pass and emit `transcription-final`.

use crate::audio::{self, AudioCapture, SharedBuffer};
use crate::decode;
use crate::loop_detect;
use crate::model_manager::{self, ModelId};
use crate::transcription::Transcriber;
use crate::vad;
use crossbeam_channel::{bounded, Receiver, RecvTimeoutError, Sender};
use parking_lot::Mutex;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// How often to run interim transcription during recording.
const INTERIM_INTERVAL: Duration = Duration::from_millis(900);
/// Minimum buffer length (in 16 kHz samples) to run whisper.
const MIN_SAMPLES_16K: usize = 8_000; // 0.5 s

#[derive(Debug)]
pub enum EngineCommand {
    Start,
    SetLocked(bool),
    Stop,
    Cancel,
    LoadModel(ModelId),
    /// Transcribe an audio/video file. The flag is checked in the whisper abort callback.
    TranscribeFile(PathBuf, Arc<AtomicBool>),
}

/// Handle for sending commands to the worker thread. Cloneable; stored in Tauri state.
#[derive(Clone)]
pub struct EngineHandle {
    tx: Sender<EngineCommand>,
}

impl EngineHandle {
    pub fn send(&self, cmd: EngineCommand) {
        let _ = self.tx.send(cmd);
    }
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum RecordingState {
    Idle,
    Recording,
    Locked,
    Finalizing,
}

/// Current model status, shared so the frontend can query it on startup
/// without missing the initial event due to subscription timing.
#[derive(Serialize, Clone, Default)]
pub struct ModelStatusValue {
    pub model: Option<ModelId>,
    pub loaded: bool,
    pub error: Option<String>,
}

#[derive(Clone, Default)]
pub struct ModelStatusShared(pub std::sync::Arc<Mutex<ModelStatusValue>>);

pub fn spawn(app: AppHandle, initial_model: ModelId, status: ModelStatusShared) -> EngineHandle {
    let (tx, rx) = bounded::<EngineCommand>(32);
    thread::spawn(move || worker(app, rx, initial_model, status));
    EngineHandle { tx }
}

fn worker(
    app: AppHandle,
    rx: Receiver<EngineCommand>,
    initial_model: ModelId,
    status: ModelStatusShared,
) {
    let buffer: SharedBuffer = Arc::new(Mutex::new(Vec::new()));
    let mut capture: Option<AudioCapture> = None;
    let mut transcriber: Option<Transcriber> = None;
    let mut current_model = initial_model;
    let mut recording = false;
    let mut last_interim = Instant::now();

    match load_model(&app, current_model) {
        Ok(t) => {
            let backend = if t.gpu_enabled {
                if cfg!(target_os = "macos") { "Metal GPU" } else { "Vulkan GPU" }
            } else {
                "CPU"
            };
            eprintln!("[engine] model loaded: {:?} ({})", current_model, backend);
            transcriber = Some(t);
            set_status(&app, &status, current_model, true, None);
        }
        Err(e) => {
            eprintln!("[engine] failed to load model: {e}");
            set_status(&app, &status, current_model, false, Some(e.to_string()));
        }
    }

    loop {
        // Poll quickly during recording (interim + level), sleep until command otherwise.
        let timeout = if recording {
            Duration::from_millis(200)
        } else {
            Duration::from_secs(3600)
        };

        match rx.recv_timeout(timeout) {
            Ok(EngineCommand::Start) => {
                buffer.lock().clear();
                match audio::start_capture(buffer.clone()) {
                    Ok(c) => {
                        capture = Some(c);
                        recording = true;
                        last_interim = Instant::now();
                        emit_state(&app, RecordingState::Recording);
                    }
                    Err(e) => emit_error(&app, format!("microphone: {e}")),
                }
            }
            Ok(EngineCommand::SetLocked(locked)) => {
                if recording {
                    emit_state(
                        &app,
                        if locked {
                            RecordingState::Locked
                        } else {
                            RecordingState::Recording
                        },
                    );
                }
            }
            Ok(EngineCommand::Stop) => {
                let rate = capture.as_ref().map(|c| c.sample_rate).unwrap_or(16_000);
                capture = None; // drop stops the stream
                recording = false;
                emit_state(&app, RecordingState::Finalizing);
                finalize(&app, transcriber.as_ref(), &buffer, rate);
                buffer.lock().clear();
                emit_state(&app, RecordingState::Idle);
            }
            Ok(EngineCommand::Cancel) => {
                capture = None;
                recording = false;
                buffer.lock().clear();
                emit_state(&app, RecordingState::Idle);
            }
            Ok(EngineCommand::LoadModel(id)) => {
                current_model = id;
                set_status(&app, &status, current_model, false, Some("loading".into()));
                match load_model(&app, current_model) {
                    Ok(t) => {
                        let backend = if t.gpu_enabled {
                            if cfg!(target_os = "macos") { "Metal GPU" } else { "Vulkan GPU" }
                        } else {
                            "CPU"
                        };
                        eprintln!("[engine] model switched: {:?} ({})", current_model, backend);
                        transcriber = Some(t);
                        set_status(&app, &status, current_model, true, None);
                    }
                    Err(e) => {
                        transcriber = None;
                        set_status(&app, &status, current_model, false, Some(e.to_string()));
                    }
                }
            }
            Ok(EngineCommand::TranscribeFile(path, cancel)) => {
                transcribe_file(&app, transcriber.as_ref(), &path, &cancel);
            }
            Err(RecvTimeoutError::Timeout) => {
                if recording {
                    let rate = capture.as_ref().map(|c| c.sample_rate).unwrap_or(16_000);
                    tick(&app, transcriber.as_ref(), &buffer, rate, &mut last_interim);
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Periodic tick during recording: emit microphone level + run interim transcription.
fn tick(
    app: &AppHandle,
    transcriber: Option<&Transcriber>,
    buffer: &SharedBuffer,
    rate: u32,
    last_interim: &mut Instant,
) {
    let snapshot = buffer.lock().clone();

    // Mic level from the last ~100 ms window, for the visual indicator.
    let window = (rate as usize / 10).max(1);
    let start = snapshot.len().saturating_sub(window);
    let _ = app.emit("audio-level", vad::rms(&snapshot[start..]));

    if last_interim.elapsed() >= INTERIM_INTERVAL {
        if let Some(t) = transcriber {
            let pcm = audio::resample_to_16k(&snapshot, rate);
            if pcm.len() >= MIN_SAMPLES_16K {
                if let Ok(text) = t.transcribe(&pcm) {
                    if !text.is_empty() {
                        let _ = app.emit("transcription-interim", text);
                    }
                }
            }
        }
        *last_interim = Instant::now();
    }
}

/// Final transcription pass over the full recorded buffer.
fn finalize(app: &AppHandle, transcriber: Option<&Transcriber>, buffer: &SharedBuffer, rate: u32) {
    let snapshot = buffer.lock().clone();
    let Some(t) = transcriber else {
        emit_error(app, "model not loaded".into());
        return;
    };
    let pcm = audio::resample_to_16k(&snapshot, rate);
    if pcm.len() < MIN_SAMPLES_16K {
        let _ = app.emit("transcription-final", String::new());
        return;
    }
    match t.transcribe(&pcm) {
        Ok(text) => {
            let _ = app.emit("transcription-final", text);
        }
        Err(e) => emit_error(app, e.to_string()),
    }
}

/// Decode a file and transcribe it with progress events and cancellation support.
/// Text is appended to the transcript via `transcription-final`.
fn transcribe_file(
    app: &AppHandle,
    transcriber: Option<&Transcriber>,
    path: &std::path::Path,
    cancel: &Arc<AtomicBool>,
) {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let _ = app.emit("file-started", serde_json::json!({ "name": name }));

    let Some(t) = transcriber else {
        let _ = app.emit("file-error", serde_json::json!({ "error": "model not loaded" }));
        return;
    };

    let mut pcm = match decode::decode_to_16k_mono(path, cancel) {
        Ok(p) => p,
        Err(e) => {
            let _ = app.emit("file-error", serde_json::json!({ "error": e.to_string() }));
            return;
        }
    };

    if cancel.load(Ordering::Relaxed) {
        let _ = app.emit("file-cancelled", serde_json::json!({}));
        return;
    }
    if pcm.is_empty() {
        let _ = app.emit(
            "file-error",
            serde_json::json!({ "error": "failed to extract audio from file" }),
        );
        return;
    }

    // Trim at the loop point if one was detected.
    if let Some(loop_pt) = loop_detect::find_loop_point(&pcm) {
        eprintln!("[engine] loop at {:.1} s, trimming", loop_pt as f32 / 16_000.0);
        pcm.truncate(loop_pt);
    }

    let duration_secs = pcm.len() as f64 / 16_000.0;
    let _ = app.emit("file-duration", serde_json::json!({ "duration_secs": duration_secs }));

    let cancel_ref = cancel.clone();
    let result = t.transcribe_file(
        &pcm,
        move || cancel_ref.load(Ordering::Relaxed),
    );

    if cancel.load(Ordering::Relaxed) {
        let _ = app.emit("file-cancelled", serde_json::json!({}));
        return;
    }
    match result {
        Ok(text) => {
            if !text.is_empty() {
                let _ = app.emit("transcription-final", text);
            }
            let _ = app.emit("file-done", serde_json::json!({}));
        }
        Err(e) => {
            let _ = app.emit("file-error", serde_json::json!({ "error": e.to_string() }));
        }
    }
}

fn load_model(app: &AppHandle, id: ModelId) -> anyhow::Result<Transcriber> {
    let path = model_manager::resolve_model_path(app, id)?;
    Transcriber::load(&path, id)
}

fn emit_state(app: &AppHandle, state: RecordingState) {
    let _ = app.emit("recording-state", state);
}

/// Update the shared model status and emit a `model-status` event.
fn set_status(
    app: &AppHandle,
    status: &ModelStatusShared,
    id: ModelId,
    loaded: bool,
    error: Option<String>,
) {
    *status.0.lock() = ModelStatusValue {
        model: Some(id),
        loaded,
        error: error.clone(),
    };
    let _ = app.emit(
        "model-status",
        serde_json::json!({ "model": id, "loaded": loaded, "error": error }),
    );
}

fn emit_error(app: &AppHandle, message: String) {
    let _ = app.emit("engine-error", message);
}
