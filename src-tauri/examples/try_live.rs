//! Headless-репро пути ЖИВОЙ диктовки: повторные transcribe() на растущем
//! буфере (как interim каждые 900 мс), затем финальный прогон.
//! Запуск: cargo run --example try_live -- /path/to/file.mp4

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use whisper_dictate_lib::decode;
use whisper_dictate_lib::model_manager::ModelId;
use whisper_dictate_lib::transcription::Transcriber;

fn main() {
    let path = std::env::args().nth(1).expect("usage: try_live <file>");
    let model = format!(
        "{}/Library/Application Support/com.vladislav.whisperdictate/models/ggml-large-v3-turbo-q5_0.bin",
        std::env::var("HOME").unwrap()
    );

    let handle = std::thread::spawn(move || {
        let cancel = AtomicBool::new(false);
        let pcm = decode::decode_to_16k_mono(&PathBuf::from(&path), &cancel).expect("decode failed");
        eprintln!("[worker] декод ок: {:.1} c", pcm.len() as f32 / 16000.0);

        let t = Transcriber::load(std::path::Path::new(&model), ModelId::LargeV3Turbo)
            .expect("load failed");
        eprintln!("[worker] модель загружена");

        // Имитируем interim: растущие окна по ~1 c, как при удержании кнопки.
        let step = 16_000; // 1 c при 16 kHz
        let cap = pcm.len().min(15 * 16_000); // как реальное удержание ~15 c
        let mut end = step;
        let mut call = 0;
        while end < cap {
            call += 1;
            let chunk = &pcm[..end];
            match t.transcribe(chunk) {
                Ok(txt) => eprintln!("[worker] interim #{call} ({} c) ок: {} симв", end / 16_000, txt.chars().count()),
                Err(e) => {
                    eprintln!("[worker] interim #{call} ({} c) ОШИБКА: {e:#}", end / 16_000);
                    panic!("interim failed on call {call}");
                }
            }
            end += step;
        }

        // Финальный прогон по всему буферу.
        match t.transcribe(&pcm) {
            Ok(txt) => eprintln!("[worker] FINAL ок: {} симв", txt.chars().count()),
            Err(e) => {
                eprintln!("[worker] FINAL ОШИБКА: {e:#}");
                panic!("final failed");
            }
        }
    });

    handle.join().expect("worker panicked");
    println!("\n===== LIVE OK =====");
}
