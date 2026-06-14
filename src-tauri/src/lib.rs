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
mod diag;
mod engine;
pub mod loop_detect;
pub mod model_manager;
pub mod transcription;
mod vad;

use model_manager::ModelId;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    diag::redirect_native_stderr();

    #[cfg(target_os = "macos")]
    unsafe { std::env::set_var("GGML_METAL_NO_RESIDENCY", "1"); }

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets(diag::log_targets())
                .level(log::LevelFilter::Debug)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            diag::startup_diagnostics(app.handle());
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
