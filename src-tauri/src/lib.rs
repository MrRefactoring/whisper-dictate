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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // On macOS, request microphone permission immediately on startup so the
            // system dialog never interrupts the user mid-gesture on the mic button.
            #[cfg(target_os = "macos")]
            std::thread::spawn(crate::audio::request_microphone_permission);

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
