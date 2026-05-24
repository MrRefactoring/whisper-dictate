# Whisper Dictate

> [Русская версия](README.ru.md)

Offline dictation and file transcription app: speak → see transcript → copy to clipboard.
All speech recognition runs locally via [Whisper](https://github.com/openai/whisper) — no audio ever leaves your device.

- **Platforms:** macOS 11+ (Metal GPU) · Windows (Vulkan GPU, CPU fallback)
- **Stack:** Tauri 2 (Rust) + React 19 + TypeScript + Vite
- **STT:** [whisper-rs](https://github.com/tazz4843/whisper-rs) (bindings for whisper.cpp)
- **GPU:** Metal on macOS · Vulkan on Windows (automatic CPU fallback if no GPU)
- **Default model:** `large-v3-turbo` q5_0 (~570 MB), optionally `large-v3`
- **Design:** "ma" (間) aesthetic — washi paper and ink, cinnabar accent
- **UI language:** auto-detected from OS locale (EN / RU), switchable in the app

## Download

Pre-built binaries are on the [Releases](https://github.com/MrRefactoring/whisper-dictate/releases) page:
- **macOS:** `.dmg` (universal — Intel + Apple Silicon)
- **Windows:** `.msi` / `.exe`

The app will notify you about updates on startup.

## Screenshots

| Light | Dark |
|---|---|
| ![Light theme](docs/screenshot-light.png) | ![Dark theme](docs/screenshot-dark.png) |

> Screenshots coming soon. To contribute, run the app and drop images in `docs/`.

## Build from source

### macOS

```bash
# Requirements: Node + pnpm, Rust, Xcode CLI Tools, CMake
brew install cmake

pnpm install
pnpm app          # dev run (first Rust+whisper.cpp build takes a while)
pnpm app:build    # production build → src-tauri/target/release/bundle/
```

On first launch macOS will prompt for microphone access.

### Windows

```bash
# Requirements: Node + pnpm, Rust (MSVC), Visual Studio C++ tools, CMake
pnpm install
pnpm app
```

## Commands

| Command | Description |
|---|---|
| `pnpm app` | Dev run |
| `pnpm app:build` | Production build (`.app` + `.dmg` / `.msi`) |
| `pnpm app:dmg` | `.dmg` only (macOS) |
| `pnpm model` | Download `large-v3-turbo` manually |
| `pnpm model:large` | Download `large-v3` (best quality) |

## Usage

**Dictation (push-to-talk):** hold the dot or Space → recording; drag up → lock (hands-free) until manually stopped.

**File transcription:** drag an audio/video file into the window or click "transcribe a file".

Supported formats: mp3, m4a, aac, wav, flac, ogg, opus, mp4, mov, mkv, webm, and more.

**Language:** the app auto-detects your OS locale and switches between English and Russian. You can override it with the `en · ru` toggle in the top-right corner.

## Architecture

```
src-tauri/src/
  audio.rs         — microphone capture (cpal), downmix → mono, resample to 16 kHz
  vad.rs           — energy-based VAD (RMS) for level and silence detection
  transcription.rs — whisper-rs; Metal GPU on macOS, CPU on Windows
  decode.rs        — audio/video file decoding (symphonia)
  loop_detect.rs   — looped audio detection for video files
  model_manager.rs — model selection / search / download
  engine.rs        — worker thread: recording state machine + hybrid mode
  commands.rs      — Tauri commands (start/stop/lock/transcribe_file…)
  lib.rs           — app assembly, plugins, command registration

src/
  i18n/             — translations (EN/RU), language context and hook
  hooks/
    useDictation.ts — all dictation events/state
    useUpdater.ts   — update check on startup
  components/       — MicButton, InkWave, TranscriptPanel, ModelPicker, LangSwitch, Toast
```

## License

MIT © 2026 Vladislav Tupikin
