//! whisper-rs wrapper (bindings to whisper.cpp).
//!
//! The model context and state (`WhisperState`) are created ONCE and reused
//! across runs. Creating a new state per call is not safe: on Metal it causes
//! backend re-initialization and the second `whisper_full_with_state` call fails
//! with "failed to encode". `WhisperState` holds an `Arc` to the internal context,
//! so keeping both fields is safe.
//!
//! macOS:         Metal GPU (`use_gpu(true)`). flash_attn is disabled — it breaks
//!                encode on Metal.
//! Windows/Linux: Vulkan GPU (`use_gpu(true)`) preferred; automatically falls back
//!                to CPU if no Vulkan-capable GPU or driver is present.

use crate::model_manager::ModelId;
use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::path::Path;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

pub struct Transcriber {
    state: Mutex<WhisperState>,
    #[allow(dead_code)]
    ctx: WhisperContext,
    #[allow(dead_code)]
    model: ModelId,
    /// true if the model is running on GPU (Metal/Vulkan), false on CPU fallback.
    pub gpu_enabled: bool,
}

impl Transcriber {
    /// Load a ggml model.
    ///
    /// macOS:         always Metal GPU.
    /// Windows/Linux: tries Vulkan GPU first; silently falls back to CPU on failure.
    pub fn load(model_path: &Path, model: ModelId) -> Result<Self> {
        #[cfg(target_os = "macos")]
        return Self::load_impl(model_path, model, true);

        #[cfg(not(target_os = "macos"))]
        Self::load_impl(model_path, model, true)
            .or_else(|_| Self::load_impl(model_path, model, false))
    }

    fn load_impl(model_path: &Path, model: ModelId, use_gpu: bool) -> Result<Self> {
        let mut params = WhisperContextParameters::default();
        if use_gpu {
            params.use_gpu(true);
            params.gpu_device(0);
        }

        let path_str = model_path
            .to_str()
            .context("model path contains non-UTF8 characters")?;

        let ctx = WhisperContext::new_with_params(path_str, params)
            .with_context(|| format!("failed to load model: {path_str}"))?;
        let state = ctx.create_state().context("failed to create whisper state")?;

        Ok(Self {
            state: Mutex::new(state),
            ctx,
            model,
            gpu_enabled: use_gpu,
        })
    }

    /// Transcribe mono f32 PCM at 16 kHz (language auto-detected).
    pub fn transcribe(&self, samples_16k: &[f32]) -> Result<String> {
        let mut state = self.state.lock();
        state
            .full(base_params(), samples_16k)
            .context("whisper run failed")?;
        collect_segments(&state)
    }

    /// Transcribes a long buffer (file) by splitting into 30 s windows.
    ///
    /// On macOS, `state.full()` on >30 s audio makes multiple Metal encode calls in a
    /// row; the second one fails due to contention with WKWebView. Chunking avoids
    /// this: each call gets ≤30 s → exactly one Metal encode → stable.
    /// On Windows/Linux, chunking also enables cancellation checks and smooth
    /// progress animation on the JS side.
    pub fn transcribe_file<A>(
        &self,
        samples_16k: &[f32],
        should_abort: A,
    ) -> Result<String>
    where
        A: Fn() -> bool,
    {
        const WINDOW: usize = 30 * 16_000; // 30 s — one Metal/Vulkan encode call
        let total = samples_16k.len();
        let mut parts: Vec<String> = Vec::new();
        let mut start = 0;

        while start < total {
            if should_abort() {
                return Err(anyhow::anyhow!("cancelled"));
            }
            let end = (start + WINDOW).min(total);
            let chunk = &samples_16k[start..end];
            if chunk.len() < 8_000 {
                break;
            }
            let text = self.transcribe(chunk)?;
            if !text.is_empty() {
                parts.push(text);
            }
            start += WINDOW;
        }
        Ok(parts.join(" "))
    }
}

fn base_params() -> FullParams<'static, 'static> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    let threads = std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4);
    params.set_n_threads(threads);
    params.set_language(Some("auto"));
    params.set_translate(false);
    params.set_no_context(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params
}

fn collect_segments(state: &WhisperState) -> Result<String> {
    let n = state.full_n_segments();
    let mut out = String::new();
    for i in 0..n {
        if let Some(seg) = state.get_segment(i) {
            if let Ok(text) = seg.to_str_lossy() {
                out.push_str(text.as_ref());
            }
        }
    }
    Ok(out.trim().to_string())
}
