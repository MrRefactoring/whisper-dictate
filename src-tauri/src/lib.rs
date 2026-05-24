//! Entry point for the Whisper Dictate Tauri app.
//!
//! Data flow: audio (cpal) → engine worker (whisper-rs) → React UI events.
//! Default model: large-v3-turbo. Output: transcript + clipboard copy.
//!
//! The global-shortcut plugin is included but shortcuts are NOT registered
//! automatically, to avoid requesting permissions beyond microphone access.

pub mod audio;
mod commands;
pub mod decode;
mod engine;
pub mod loop_detect;
pub mod model_manager;
pub mod transcription;
mod vad;

use model_manager::ModelId;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Workaround for a crash in ggml-metal on Apple Silicon (whisper-rs 0.16.0).
    // MTLResidencySet objects have a 180 s keep-alive; if the process exits before
    // that window the Metal device destructor asserts [rsets->data count] == 0.
    // Setting GGML_METAL_NO_RESIDENCY=1 tells ggml to skip residency sets entirely
    // (GPU memory becomes evictable after ~1 s of inactivity — negligible for STT).
    // Must be set before ggml initialises the Metal device (i.e. before model load).
    // See: https://github.com/ggml-org/llama.cpp/pull/11427
    #[cfg(target_os = "macos")]
    // SAFETY: single-threaded at this point; no other threads have started yet.
    unsafe { std::env::set_var("GGML_METAL_NO_RESIDENCY", "1"); }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let status = engine::ModelStatusShared::default();
            app.manage(status.clone());
            let engine = engine::spawn(handle, ModelId::LargeV3Turbo, status);
            app.manage(engine);
            app.manage(commands::DownloadFlags::default());
            app.manage(commands::FileCancel::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::set_locked,
            commands::cancel_recording,
            commands::set_model,
            commands::list_models,
            commands::get_model_status,
            commands::download_model,
            commands::cancel_download,
            commands::delete_model,
            commands::transcribe_file,
            commands::cancel_file_transcription,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
