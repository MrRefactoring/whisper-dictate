//! Microphone capture via cpal, converted to whisper's expected format.
//!
//! Whisper needs 16 kHz mono f32 PCM. Most microphones output 44.1/48 kHz
//! multi-channel, so we downmix to mono on the fly; resampling to 16 kHz
//! happens in [`resample_to_16k`] just before transcription.

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SizedSample, Stream};
use parking_lot::Mutex;
use std::sync::Arc;

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

pub type SharedBuffer = Arc<Mutex<Vec<f32>>>;

/// Raw `AVCaptureDevice.authorizationStatusForMediaType:` for audio.
/// Returns the AVAuthorizationStatus: 0=NotDetermined, 1=Restricted,
/// 2=Denied, 3=Authorized. Returns 3 (assume ok) if AVFoundation is missing.
#[cfg(target_os = "macos")]
pub(crate) fn mic_tcc_status() -> isize {
    use objc2::msg_send;
    use objc2::runtime::AnyClass;
    use objc2_foundation::ns_string;

    unsafe {
        let Some(cls) = AnyClass::get(c"AVCaptureDevice") else {
            log::warn!(target: "audio", "AVCaptureDevice not found, assuming authorized");
            return 3;
        };
        let audio_type = ns_string!("soun");
        msg_send![cls, authorizationStatusForMediaType: audio_type]
    }
}

/// Synchronously requests microphone access and blocks until the user answers
/// the system dialog. Returns whether access was granted.
///
/// Only called when the status is `NotDetermined`. We wait for the completion
/// handler (delivered on an AVFoundation queue) via a channel so that, by the
/// time cpal opens the stream, the status is already resolved — CoreAudio then
/// does *not* show its own dialog, so the user sees exactly one prompt. The
/// earlier "asked 6 times" bug came from firing `requestAccess` *without*
/// awaiting it, racing cpal's implicit dialog.
///
/// Blocking here is safe: `start_capture` runs on the engine worker thread, not
/// the main thread, so the main runloop stays free to present the modal dialog.
#[cfg(target_os = "macos")]
fn request_mic_access_blocking() -> bool {
    use block2::RcBlock;
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, Bool};
    use objc2_foundation::ns_string;

    let Some(cls) = AnyClass::get(c"AVCaptureDevice") else {
        log::warn!(target: "audio", "AVCaptureDevice not found, assuming authorized");
        return true;
    };

    let (tx, rx) = crossbeam_channel::bounded::<bool>(1);
    // The handler runs on an arbitrary AVFoundation queue, so it must be
    // `Send + 'static`; it captures only the channel sender.
    let handler = RcBlock::new(move |granted: Bool| {
        let _ = tx.send(granted.as_bool());
    });

    let audio_type = ns_string!("soun");
    unsafe {
        let _: () = msg_send![
            cls,
            requestAccessForMediaType: audio_type,
            completionHandler: &*handler,
        ];
    }

    // Hold `handler` alive until the answer arrives, then return it.
    match rx.recv() {
        Ok(granted) => granted,
        Err(_) => false,
    }
}

/// Ensures the app may use the microphone, requesting access on first use.
///
/// `Authorized` proceeds immediately; `Denied`/`Restricted` errors out;
/// `NotDetermined` triggers a single blocking system prompt (see
/// [`request_mic_access_blocking`]).
#[cfg(target_os = "macos")]
fn ensure_mic_authorized() -> Result<()> {
    let status = mic_tcc_status();
    log::info!(target: "audio", "TCC mic status = {status} (0=notdetermined,1=restricted,2=denied,3=authorized)");
    let denied = || {
        anyhow!("microphone access denied — open System Settings → Privacy & Security → Microphone")
    };
    match status {
        3 => Ok(()),
        1 | 2 => Err(denied()),
        0 => {
            log::info!(target: "audio", "requesting microphone access (blocking)");
            if request_mic_access_blocking() {
                log::info!(target: "audio", "microphone access granted");
                Ok(())
            } else {
                log::warn!(target: "audio", "microphone access denied by user");
                Err(denied())
            }
        }
        other => {
            log::warn!(target: "audio", "unexpected TCC status {other}, proceeding");
            Ok(())
        }
    }
}

/// Enumerates available input devices with their default config, for diagnostics.
pub fn enumerate_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    let mut out = Vec::new();
    match host.input_devices() {
        Ok(devices) => {
            for d in devices {
                let name = d.name().unwrap_or_else(|_| "<unknown>".into());
                let cfg = d
                    .default_input_config()
                    .map(|c| format!("{c:?}"))
                    .unwrap_or_else(|e| format!("config error: {e}"));
                let marker = if name == default_name { " [default]" } else { "" };
                out.push(format!("{name}{marker} — {cfg}"));
            }
        }
        Err(e) => out.push(format!("input_devices() error: {e}")),
    }
    if out.is_empty() {
        out.push("<no input devices>".into());
    }
    out
}

/// Active capture stream. Dropping stops the microphone.
pub struct AudioCapture {
    _stream: Stream,
    /// Device sample rate, needed for downstream resampling.
    pub sample_rate: u32,
}

pub fn start_capture(buffer: SharedBuffer) -> Result<AudioCapture> {
    log::info!(target: "audio", "start_capture called");

    #[cfg(target_os = "macos")]
    ensure_mic_authorized()?;

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no default input device found"))?;

    log::info!(target: "audio", "input device: {}", device.name().unwrap_or_default());

    let supported = device.default_input_config()?;
    log::info!(target: "audio", "config: {supported:?}");
    let sample_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let config: cpal::StreamConfig = supported.config();

    let err_fn = |e| log::error!(target: "audio", "stream error: {e}");

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, channels, buffer, err_fn)?,
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, channels, buffer, err_fn)?,
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, channels, buffer, err_fn)?,
        other => return Err(anyhow!("unsupported sample format: {other:?}")),
    };

    log::info!(target: "audio", "stream built, calling play()");
    stream.play()?;
    log::info!(target: "audio", "stream playing");
    Ok(AudioCapture {
        _stream: stream,
        sample_rate,
    })
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    buffer: SharedBuffer,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream>
where
    T: Sample + SizedSample,
    f32: FromSample<T>,
{
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let mut buf = buffer.lock();
            buf.reserve(data.len() / channels);
            for frame in data.chunks(channels) {
                let mut sum = 0.0f32;
                for &sample in frame {
                    sum += f32::from_sample(sample);
                }
                buf.push(sum / channels as f32);
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

/// Linear resampling to 16 kHz. Quality is sufficient for speech; can be
/// replaced with a polyphase filter (e.g. `rubato`) if needed.
pub fn resample_to_16k(input: &[f32], src_rate: u32) -> Vec<f32> {
    if input.is_empty() || src_rate == TARGET_SAMPLE_RATE {
        return input.to_vec();
    }
    let ratio = TARGET_SAMPLE_RATE as f64 / src_rate as f64;
    let out_len = (input.len() as f64 * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    let last = input.len() - 1;
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = input[idx.min(last)];
        let b = input[(idx + 1).min(last)];
        out.push(a + (b - a) * frac);
    }
    out
}
