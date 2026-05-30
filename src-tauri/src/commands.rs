//! Tauri commands — bridge between the React frontend and the dictation engine.
//!
//! Recording uses Telegram-style PTT with lock: the frontend calls start/stop on
//! key press/release. Final and interim text arrives via events (see engine.rs).

use crate::engine::{EngineCommand, EngineHandle, ModelStatusShared, ModelStatusValue};
use crate::model_manager::{self, DownloadOutcome, ModelId, ModelInfo};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

/// Active download cancel flags keyed by model id. Stored in Tauri managed state.
#[derive(Default, Clone)]
pub struct DownloadFlags(pub Arc<Mutex<HashMap<ModelId, Arc<AtomicBool>>>>);

/// Cancellation flag for the current file transcription. Read in the whisper abort callback.
#[derive(Default, Clone)]
pub struct FileCancel(pub Arc<AtomicBool>);

#[tauri::command]
pub fn start_recording(engine: State<EngineHandle>) {
    engine.send(EngineCommand::Start);
}

#[tauri::command]
pub fn stop_recording(engine: State<EngineHandle>) {
    engine.send(EngineCommand::Stop);
}

#[tauri::command]
pub fn set_locked(engine: State<EngineHandle>, locked: bool) {
    engine.send(EngineCommand::SetLocked(locked));
}

#[tauri::command]
pub fn cancel_recording(engine: State<EngineHandle>) {
    engine.send(EngineCommand::Cancel);
}

#[tauri::command]
pub fn set_model(engine: State<EngineHandle>, model: ModelId) {
    engine.send(EngineCommand::LoadModel(model));
}

#[tauri::command]
pub fn list_models(app: AppHandle) -> Vec<ModelInfo> {
    model_manager::list_models(&app)
}

/// Returns current model status. The frontend calls this on startup to avoid
/// missing the initial `model-status` event due to subscription timing.
#[tauri::command]
pub fn get_model_status(status: State<ModelStatusShared>) -> ModelStatusValue {
    status.0.lock().clone()
}

/// Download a model in the background. Progress → `model-download-progress`,
/// done → `model-download-done`, error → `model-download-error`.
/// The UI triggers model loading via `set_model` after download, keeping
/// UI and engine model state in sync.
#[tauri::command]
pub fn download_model(app: AppHandle, flags: State<DownloadFlags>, model: ModelId) {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        // Guard against a double-click spawning two writers to the same `.part`
        // file (File::create truncates → races/corruption). If a download for
        // this model is already in flight, ignore the request.
        let mut map = flags.0.lock().unwrap();
        if map.contains_key(&model) {
            return;
        }
        map.insert(model, cancel.clone());
    }
    let map = flags.0.clone();
    std::thread::spawn(move || {
        let result = model_manager::download_model(&app, model, &cancel, |received, total| {
            let pct = total
                .filter(|t| *t > 0)
                .map(|t| received as f64 / t as f64);
            let _ = app.emit(
                "model-download-progress",
                serde_json::json!({
                    "model": model,
                    "received": received,
                    "total": total,
                    "pct": pct,
                }),
            );
        });
        map.lock().unwrap().remove(&model);
        match result {
            Ok(DownloadOutcome::Completed) => {
                let _ = app.emit("model-download-done", serde_json::json!({ "model": model }));
            }
            Ok(DownloadOutcome::Cancelled) => {
                let _ = app.emit("model-download-cancelled", serde_json::json!({ "model": model }));
            }
            Err(e) => {
                let _ = app.emit(
                    "model-download-error",
                    serde_json::json!({ "model": model, "error": e.to_string() }),
                );
            }
        }
    });
}

#[tauri::command]
pub fn cancel_download(flags: State<DownloadFlags>, model: ModelId) {
    if let Some(flag) = flags.0.lock().unwrap().get(&model) {
        flag.store(true, Ordering::Relaxed);
    }
}

/// Delete a downloaded model from app data. An in-memory model continues
/// working until the app restarts or the model is switched.
#[tauri::command]
pub fn delete_model(app: AppHandle, model: ModelId) -> Result<(), String> {
    model_manager::delete_model(&app, model).map_err(|e| e.to_string())
}

/// Transcribe an audio/video file. Events: `transcription-final`,
/// `file-done`, `file-cancelled`, `file-error`.
#[tauri::command]
pub fn transcribe_file(engine: State<EngineHandle>, file_cancel: State<FileCancel>, path: String) {
    file_cancel.0.store(false, Ordering::Relaxed);
    engine.send(EngineCommand::TranscribeFile(
        PathBuf::from(path),
        file_cancel.0.clone(),
    ));
}

#[tauri::command]
pub fn cancel_file_transcription(file_cancel: State<FileCancel>) {
    file_cancel.0.store(true, Ordering::Relaxed);
}
