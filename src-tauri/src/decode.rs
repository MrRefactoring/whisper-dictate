//! Decodes audio/video files to 16 kHz mono f32 PCM via symphonia.
//!
//! Video containers (mp4/mov/mkv) are supported by extracting the audio track.
//! Supported codecs: mp3, m4a/aac, wav, flac, ogg/vorbis, alac, and others.
//!
//! Opus (ogg/opus, .opus, webm/opus) is demuxed by symphonia but decoded via
//! libopus (the `opus` crate), since symphonia ships no Opus decoder. Opus is
//! always 48 kHz; its output is resampled to 16 kHz like every other codec, and
//! the encoder pre-skip reported by symphonia is trimmed from the start.

use crate::audio;
use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL, CODEC_TYPE_OPUS};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader};
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
    // Copy out everything we need so the `track` borrow on `format` ends here and
    // both decode paths can borrow `format` mutably.
    let codec_params = track.codec_params.clone();
    let track_id = track.id;
    let codec = codec_params.codec;
    let src_rate = codec_params.sample_rate.unwrap_or(16_000);
    let channel_count = codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(1)
        .max(1);

    // Symphonia demuxes ogg/opus but has no Opus decoder, so decode those packets
    // with libopus. Every other codec uses symphonia's built-in decoders.
    let mono = if codec == CODEC_TYPE_OPUS {
        let pre_skip = codec_params.delay.unwrap_or(0) as usize;
        decode_opus_track(&mut format, track_id, channel_count, pre_skip, cancel)?
    } else {
        let mut decoder = symphonia::default::get_codecs()
            .make(&codec_params, &DecoderOptions::default())
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
                Err(SymError::DecodeError(_)) => continue,
                Err(SymError::IoError(_)) => break,
                Err(e) => return Err(anyhow!("decode error: {e}")),
            }
        }
        mono
    };

    if mono.is_empty() {
        return Ok(Vec::new());
    }
    Ok(audio::resample_to_16k(&mono, src_rate))
}

/// Decode the Opus packets that symphonia demuxes from an ogg/webm container.
/// Returns mono f32 PCM at 48 kHz (the caller resamples to 16 kHz), or an empty
/// vec if cancelled.
fn decode_opus_track(
    format: &mut Box<dyn FormatReader>,
    track_id: u32,
    channel_count: usize,
    pre_skip: usize,
    cancel: &AtomicBool,
) -> Result<Vec<f32>> {
    // libopus only decodes mono or stereo; downmix anything wider to mono.
    let ch = if channel_count >= 2 { 2 } else { 1 };
    let channels = if ch == 2 {
        opus::Channels::Stereo
    } else {
        opus::Channels::Mono
    };
    let mut decoder =
        opus::Decoder::new(48_000, channels).context("failed to create Opus decoder")?;

    let mut mono: Vec<f32> = Vec::new();
    // Largest Opus frame is 120 ms @ 48 kHz = 5760 samples per channel.
    let mut out = vec![0f32; 5760 * ch];
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
        match decoder.decode_float(&packet.data, &mut out, false) {
            Ok(n) => {
                for frame in out[..n * ch].chunks(ch) {
                    mono.push(frame.iter().sum::<f32>() / ch as f32);
                }
            }
            Err(_) => continue,
        }
    }

    // Trim the encoder delay (pre-skip) reported in the OpusHead.
    if pre_skip > 0 {
        let drop = pre_skip.min(mono.len());
        mono.drain(0..drop);
    }
    Ok(mono)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Render `seconds` of a 440 Hz tone to ogg/opus with ffmpeg.
    /// Returns `None` (so the test self-skips) if ffmpeg or libopus is missing.
    fn make_opus_fixture(seconds: u32) -> Option<std::path::PathBuf> {
        let path = std::env::temp_dir().join(format!("whisper_dictate_opus_{seconds}s.ogg"));
        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency=440:duration={seconds}"),
                "-c:a",
                "libopus",
                "-ac",
                "1",
            ])
            .arg(&path)
            .status();
        match status {
            Ok(s) if s.success() && path.exists() => Some(path),
            _ => None,
        }
    }

    #[test]
    fn decodes_ogg_opus_to_16k() {
        let Some(path) = make_opus_fixture(1) else {
            eprintln!("skipping: ffmpeg/libopus unavailable");
            return;
        };
        let pcm = decode_to_16k_mono(&path, &AtomicBool::new(false))
            .expect("ogg/opus should decode");
        // ~1 s at 16 kHz; allow slack for pre-skip trim and resampling.
        assert!(
            (14_000..=18_000).contains(&pcm.len()),
            "unexpected sample count: {}",
            pcm.len()
        );
        assert!(pcm.iter().any(|&s| s.abs() > 0.01), "decoded audio is silent");
    }

    #[test]
    fn cancelled_decode_returns_empty() {
        let Some(path) = make_opus_fixture(1) else {
            eprintln!("skipping: ffmpeg/libopus unavailable");
            return;
        };
        let cancel = AtomicBool::new(true);
        let pcm = decode_to_16k_mono(&path, &cancel).expect("decode should not error");
        assert!(pcm.is_empty(), "cancelled decode must yield no samples");
    }
}
