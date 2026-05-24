//! Simple energy-based VAD (voice activity detection).
//!
//! RMS energy is sufficient for the microphone level indicator and rough
//! silence detection. For accurate speech segmentation, whisper-rs has a
//! built-in VAD, or silero/webrtc-vad can be integrated.

/// RMS amplitude (0.0 = silence, ~1.0 = clipping). Used for the mic level indicator.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}
