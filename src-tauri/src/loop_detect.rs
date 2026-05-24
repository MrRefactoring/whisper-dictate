//! Loop detection in 16 kHz mono PCM audio.
//!
//! Algorithm: split audio into 500-sample sub-windows, compute RMS of each as a
//! fingerprint. Use the first 5 s as a reference; slide a window with a 0.25 s
//! step and look for repetition. Cosine similarity ≥ THRESHOLD → loop found.
//!
//! Reliable for exact duplicates (looped tracks). Few false positives on normal
//! speech due to the high threshold.

const SAMPLE_RATE: usize = 16_000;
const MIN_LOOP_SECS: usize = 10; // search for repetition no earlier than 10 s
const WINDOW_SECS: usize = 5;    // comparison window 5 s
const SUB_WIN: usize = 500;      // fingerprint sub-window, 500 samples (~31 ms)
const STEP: usize = 4_000;       // search step, 0.25 s
const THRESHOLD: f32 = 0.92;     // 0.97 missed reels with compression artifacts

/// Find the start of a looped segment (in 16 kHz samples).
/// Returns `None` if no loop is detected or the audio is too short.
pub fn find_loop_point(samples: &[f32]) -> Option<usize> {
    let window = WINDOW_SECS * SAMPLE_RATE;
    let min_loop = MIN_LOOP_SECS * SAMPLE_RATE;

    if samples.len() < min_loop + window {
        return None;
    }

    let ref_fp = fingerprint(&samples[..window]);

    let mut max_sim = 0.0f32;
    let mut pos = min_loop;
    while pos + window <= samples.len() {
        let fp = fingerprint(&samples[pos..pos + window]);
        let sim = cosine_sim(&ref_fp, &fp);
        if sim > max_sim {
            max_sim = sim;
        }
        if sim >= THRESHOLD {
            log::info!(target: "loop_detect", "loop at {:.1} s (sim={:.4})", pos as f32 / 16_000.0, sim);
            return Some(pos);
        }
        pos += STEP;
    }
    log::info!(target: "loop_detect", "no loop found, max_sim={max_sim:.4}");
    None
}

fn fingerprint(samples: &[f32]) -> Vec<f32> {
    samples
        .chunks(SUB_WIN)
        .map(|c| {
            let mean_sq = c.iter().map(|&x| x * x).sum::<f32>() / c.len() as f32;
            mean_sq.sqrt()
        })
        .collect()
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }
    let dot: f32 = a[..len].iter().zip(&b[..len]).map(|(x, y)| x * y).sum();
    let na: f32 = a[..len].iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b[..len].iter().map(|x| x * x).sum::<f32>().sqrt();
    if na < 1e-10 || nb < 1e-10 {
        return 0.0;
    }
    (dot / (na * nb)).clamp(-1.0, 1.0)
}
