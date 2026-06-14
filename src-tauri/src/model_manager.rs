//! Whisper ggml model management: available models, file locations, downloads.
//!
//! Model lookup order: user app data folder first, then the app bundle.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager};

pub enum DownloadOutcome {
    Completed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelId {
    LargeV3Turbo,
    LargeV3,
}

impl ModelId {
    pub fn filename(self) -> &'static str {
        match self {
            ModelId::LargeV3Turbo => "ggml-large-v3-turbo-q5_0.bin",
            ModelId::LargeV3 => "ggml-large-v3.bin",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ModelId::LargeV3Turbo => "Large v3 Turbo (q5_0)",
            ModelId::LargeV3 => "Large v3",
        }
    }

    pub fn download_url(self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.filename()
        )
    }

    pub fn is_recommended(self) -> bool {
        matches!(self, ModelId::LargeV3Turbo)
    }

    pub fn approx_size(self) -> &'static str {
        match self {
            ModelId::LargeV3Turbo => "≈ 547 MB",
            ModelId::LargeV3 => "≈ 3.1 GB",
        }
    }

    pub fn all() -> [ModelId; 2] {
        [ModelId::LargeV3Turbo, ModelId::LargeV3]
    }
}

/// Returns (creating if necessary) the directory for user-downloaded models.
pub fn models_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_data_dir()?.join("models");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn bundled_path(app: &AppHandle, id: ModelId) -> Option<PathBuf> {
    app.path()
        .resource_dir()
        .ok()
        .map(|r| r.join("models").join(id.filename()))
        .filter(|p| p.exists())
}

pub fn resolve_model_path(app: &AppHandle, id: ModelId) -> Result<PathBuf> {
    let user_path = models_dir(app)?.join(id.filename());
    if user_path.exists() {
        return Ok(user_path);
    }
    if let Some(bundled) = bundled_path(app, id) {
        return Ok(bundled);
    }
    Err(anyhow!(
        "model file '{}' not found (checked app data and bundle). \
         Run scripts/fetch-model.sh or download it from the app settings.",
        id.filename()
    ))
}

#[derive(Serialize)]
pub struct ModelInfo {
    pub id: ModelId,
    pub label: String,
    pub recommended: bool,
    pub size: String,
    /// Whether the model file is currently available locally.
    pub available: bool,
}

pub fn list_models(app: &AppHandle) -> Vec<ModelInfo> {
    ModelId::all()
        .iter()
        .map(|&id| ModelInfo {
            id,
            label: id.label().to_string(),
            recommended: id.is_recommended(),
            size: id.approx_size().to_string(),
            available: resolve_model_path(app, id).is_ok(),
        })
        .collect()
}

/// Delete a downloaded model from app data. No-op if the file doesn't exist.
pub fn delete_model(app: &AppHandle, id: ModelId) -> Result<()> {
    let path = models_dir(app)?.join(id.filename());
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to delete model: {}", path.display()))?;
    }
    Ok(())
}

/// Download ggml weights to the app data folder. Progress is reported via
/// `on_progress(received_bytes, total_bytes)` (total is None if no Content-Length).
/// Downloads to a `.part` file first, then atomically renames on completion.
pub fn download_model<F>(
    app: &AppHandle,
    id: ModelId,
    cancel: &AtomicBool,
    mut on_progress: F,
) -> Result<DownloadOutcome>
where
    F: FnMut(u64, Option<u64>),
{
    let dir = models_dir(app)?;
    let dest = dir.join(id.filename());
    if dest.exists() {
        return Ok(DownloadOutcome::Completed);
    }
    let part = dir.join(format!("{}.part", id.filename()));
    let url = id.download_url();

    let client = reqwest::blocking::Client::builder()
        .build()
        .context("failed to create HTTP client")?;
    let mut resp = client
        .get(&url)
        .send()
        .with_context(|| format!("failed to start download: {url}"))?
        .error_for_status()
        .context("server returned an error while downloading model")?;

    let total = resp.content_length();
    let mut file = std::fs::File::create(&part)
        .with_context(|| format!("failed to create file: {}", part.display()))?;

    let mut buf = [0u8; 1 << 16];
    let mut received: u64 = 0;
    let mut last_report: u64 = 0;
    on_progress(0, total);
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&part);
            return Ok(DownloadOutcome::Cancelled);
        }
        let n = resp.read(&mut buf).context("error reading download stream")?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).context("error writing model file")?;
        received += n as u64;
        if received - last_report >= 2_000_000 {
            on_progress(received, total);
            last_report = received;
        }
    }
    file.flush().ok();
    drop(file);

    if let Some(expected) = total {
        if received != expected {
            let _ = std::fs::remove_file(&part);
            return Err(anyhow!(
                "incomplete download: received {received} of {expected} bytes \
                 (connection interrupted) — please retry"
            ));
        }
    }

    std::fs::rename(&part, &dest).context("failed to rename .part to final file")?;
    on_progress(received, total.or(Some(received)));
    Ok(DownloadOutcome::Completed)
}
