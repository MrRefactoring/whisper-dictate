//! Headless-проверка пайплайна расшифровки файла (декод + whisper) без GUI.
//! Запуск: cargo run --example try_transcribe -- /path/to/file.mp4
//! Вся работа идёт на ОТДЕЛЬНОМ потоке — как в движке приложения (engine worker).

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

    // Имитируем движок: загрузка модели и транскрипция на спавненном потоке.
    let handle = std::thread::spawn(move || {
        eprintln!("[worker] декодирую {path}…");
        let cancel = AtomicBool::new(false);
        let pcm = decode::decode_to_16k_mono(&PathBuf::from(&path), &cancel).expect("decode failed");
        eprintln!("[worker] декод ок: {:.1} c", pcm.len() as f32 / 16000.0);

        eprintln!("[worker] загружаю модель…");
        let t = Transcriber::load(std::path::Path::new(&model), ModelId::LargeV3Turbo)
            .expect("load failed");

        eprintln!("[worker] транскрибирую…");
        t.transcribe_with(&pcm, |p| eprintln!("[worker] прогресс {p}%"), || false)
            .expect("transcribe failed")
    });

    let text = handle.join().expect("worker panicked");
    println!("\n===== РЕЗУЛЬТАТ =====\n{text}\n=====================");
}
