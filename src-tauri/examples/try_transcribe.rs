//! Headless check of the file transcription pipeline (decode + whisper) without a GUI.
//! Run: cargo run --example try_transcribe -- /path/to/file.mp4
//! All work runs on a SEPARATE thread — same as in the app's engine worker.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use whisper_dictate_lib::decode;
use whisper_dictate_lib::model_manager::ModelId;
use whisper_dictate_lib::transcription::Transcriber;

fn main() {
    let path = std::env::args().nth(1).expect("usage: try_transcribe <file>");
    let model = format!(
        "{}/Library/Application Support/com.vladislav.whisperdictate/models/ggml-large-v3-turbo-q5_0.bin",
        std::env::var("HOME").unwrap()
    );

    // Simulate the engine: load model and transcribe on a spawned thread.
    let handle = std::thread::spawn(move || {
        eprintln!("[worker] decoding {path}…");
        let cancel = AtomicBool::new(false);
        let pcm = decode::decode_to_16k_mono(&PathBuf::from(&path), &cancel).expect("decode failed");
        eprintln!("[worker] decode ok: {:.1} s", pcm.len() as f32 / 16000.0);

        eprintln!("[worker] loading model…");
        let t = Transcriber::load(std::path::Path::new(&model), ModelId::LargeV3Turbo)
            .expect("load failed");

        eprintln!("[worker] transcribing…");
        t.transcribe_chunked(&pcm, || false)
            .expect("transcribe failed")
    });

    let text = handle.join().expect("worker panicked");
    println!("\n===== RESULT =====\n{text}\n==================");
}
