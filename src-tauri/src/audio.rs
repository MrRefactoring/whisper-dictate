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

/// Requests macOS microphone permission via AVCaptureDevice.requestAccess.
/// Blocks the calling thread until the user responds (or returns immediately
/// if permission was already granted or denied). This must be called before
/// any cpal audio capture so the system dialog never interrupts a recording
/// gesture and eats the pointer-up event.
#[cfg(target_os = "macos")]
pub fn request_microphone_permission() {
    use block2::RcBlock;
    use objc2::msg_send;
    use objc2::runtime::AnyClass;
    use objc2_foundation::ns_string;
    use std::sync::mpsc;

    unsafe {
        let Some(cls) = AnyClass::get(c"AVCaptureDevice") else { return };
        // "soun" is the raw value of AVMediaTypeAudio
        let audio_type = ns_string!("soun");

        // 0 = AVAuthorizationStatusNotDetermined; any other value means already decided
        let status: isize = msg_send![cls, authorizationStatusForMediaType: audio_type];
        if status != 0 {
            return;
        }

        let (tx, rx) = mpsc::channel::<()>();
        // Block argument must be `runtime::Bool`, not `bool` — objc2 Encode requires it
        let block: RcBlock<dyn Fn(objc2::runtime::Bool)> =
            RcBlock::new(move |_granted: objc2::runtime::Bool| {
                let _ = tx.send(());
            });
        let _: () =
            msg_send![cls, requestAccessForMediaType: audio_type, completionHandler: &*block];
        let _ = rx.recv(); // block until user responds
    }
}

/// Active capture stream. Dropping stops the microphone.
pub struct AudioCapture {
    _stream: Stream,
    /// Device sample rate, needed for downstream resampling.
    pub sample_rate: u32,
}

pub fn start_capture(buffer: SharedBuffer) -> Result<AudioCapture> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no default input device found"))?;

    let supported = device.default_input_config()?;
    // In cpal 0.17, `SampleRate` is a type alias for `u32` (no `.0` needed).
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

    stream.play()?;
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
