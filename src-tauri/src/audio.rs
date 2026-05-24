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

/// Returns an error if the user has explicitly denied microphone access via TCC.
///
/// For `NotDetermined` and `Authorized` we return `Ok(())` — cpal / CoreAudio
/// will trigger the system dialog automatically when it opens the audio stream.
/// That one-shot CoreAudio dialog is the correct, reliable path on macOS (it is
/// what v0.1.0 used and it works).  Calling `AVCaptureDevice.requestAccess`
/// ourselves *before* cpal opens the stream causes two separate TCC requests
/// racing each other, which is why permission was being asked 6 times.
#[cfg(target_os = "macos")]
fn check_mic_not_denied() -> Result<()> {
    use objc2::msg_send;
    use objc2::runtime::AnyClass;
    use objc2_foundation::ns_string;

    unsafe {
        let Some(cls) = AnyClass::get(c"AVCaptureDevice") else {
            eprintln!("[audio] AVCaptureDevice not found, skipping TCC pre-check");
            return Ok(());
        };
        let audio_type = ns_string!("soun"); // AVMediaTypeAudio
        // AVAuthorizationStatus: 0=NotDetermined, 1=Restricted, 2=Denied, 3=Authorized
        let status: isize = msg_send![cls, authorizationStatusForMediaType: audio_type];
        eprintln!("[audio] TCC mic status = {status} (0=unknown,1=restricted,2=denied,3=ok)");
        match status {
            1 | 2 => Err(anyhow!(
                "microphone access denied — open System Settings → Privacy & Security → Microphone"
            )),
            _ => Ok(()),
        }
    }
}

/// Active capture stream. Dropping stops the microphone.
pub struct AudioCapture {
    _stream: Stream,
    /// Device sample rate, needed for downstream resampling.
    pub sample_rate: u32,
}

pub fn start_capture(buffer: SharedBuffer) -> Result<AudioCapture> {
    eprintln!("[audio] start_capture called");

    // On macOS: bail early if permission was explicitly denied so the user gets
    // a human-readable error instead of a cryptic CoreAudio code.
    // For NotDetermined/Authorized we fall through and let cpal open the stream —
    // CoreAudio shows the one-time TCC dialog automatically when needed.
    #[cfg(target_os = "macos")]
    check_mic_not_denied()?;

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no default input device found"))?;

    eprintln!("[audio] input device: {}", device.name().unwrap_or_default());

    let supported = device.default_input_config()?;
    eprintln!("[audio] config: {:?}", supported);
    let sample_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let config: cpal::StreamConfig = supported.config();

    let err_fn = |e| eprintln!("[audio] stream error: {e}");

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, channels, buffer, err_fn)?,
        cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, channels, buffer, err_fn)?,
        cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, channels, buffer, err_fn)?,
        other => return Err(anyhow!("unsupported sample format: {other:?}")),
    };

    eprintln!("[audio] stream built, calling play()");
    stream.play()?;
    eprintln!("[audio] stream playing");
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
