//! Decodes audio/video files to 16 kHz mono f32 PCM via symphonia.
//!
//! Video containers (mp4/mov/mkv) are supported by extracting the audio track.
//! Supported codecs: mp3, m4a/aac, wav, flac, ogg/vorbis, alac, and others.

use crate::audio;
use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decode a file to mono f32 16 kHz PCM. Checks `cancel` throughout;
/// returns an empty vec if cancelled.
pub fn decode_to_16k_mono(path: &Path, cancel: &AtomicBool) -> Result<Vec<f32>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("failed to open file: {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("file format not recognized or unsupported")?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow!("no audio track found in file"))?;
    let track_id = track.id;
    let src_rate = track.codec_params.sample_rate.unwrap_or(16_000);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("no decoder available for this file's audio codec")?;

    let mut mono: Vec<f32> = Vec::new();

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Vec::new());
        }
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(anyhow!("stream read error: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let ch = spec.channels.count().max(1);
                let mut sb = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                sb.copy_interleaved_ref(decoded);
                for frame in sb.samples().chunks(ch) {
                    mono.push(frame.iter().sum::<f32>() / ch as f32);
                }
            }
            // Corrupt packet — skip rather than failing the whole transcription.
            Err(SymError::DecodeError(_)) => continue,
            Err(SymError::IoError(_)) => break,
            Err(e) => return Err(anyhow!("decode error: {e}")),
        }
    }

    if mono.is_empty() {
        return Ok(Vec::new());
    }
    Ok(audio::resample_to_16k(&mono, src_rate))
}
