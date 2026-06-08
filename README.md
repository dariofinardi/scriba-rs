# Scriba

**Local, private, multilingual speech-to-text for desktop.**

![License](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue)
![Platform](https://img.shields.io/badge/platform-Windows-blue)
![Rust](https://img.shields.io/badge/language-Rust-orange)

## Why Scriba

Cloud transcription services raise concerns around privacy, network dependency, and recurring costs. Scriba offers a fully local alternative: audio never leaves the device — transcription runs entirely on-device powered by OpenAI's [Whisper](https://github.com/openai/whisper), executed natively via [whisper.cpp](https://github.com/ggerganov/whisper.cpp).

The goal is to provide a simple, embeddable tool: a transcription widget that any Rust application can integrate with a single function call. Scriba handles the UI, model downloads, microphone recording, and returns a structured JSON result to the caller.

## Features

- **Audio file transcription** — import WAV, MP3, OGG, FLAC via native file picker
- **Microphone recording** — select input device, real-time capture with timer and progress bar
- **Automatic language detection** — or manual selection among Italian, English, Spanish, French, German, Portuguese
- **Speaker identification** — optional diarization: who is speaking in each segment (via [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx))
- **Built-in model management** — download, select, and switch models directly from the setup panel
- **Clipboard copy** — one click to copy the transcript
- **Structured result** — `ScribaResult` with full text, timed segments, language, speaker labels, processing times — JSON-serializable
- **BLE recorder support** — automatic download and transcription from Bluetooth devices (optional `recorder` feature, requires [mic-rs](https://github.com/dariofinardi/mic-rs))

### User interface

**Borderless** and **always-on-top** window, designed to stay above the working application without getting in the way. Custom title bar with drag, minimize, and close with confirmation dialog. System tray icon with "Show Scriba" and "About..." menu. Light/dark theme with customizable accent color.

The application starts minimized to the taskbar. Closing always requires confirmation to prevent accidental loss.

## Whisper models

Scriba uses quantized GGML models, downloaded automatically on first use from [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp/tree/main):

| ID | Model | File | Size | Speed | Quality | Recommended use |
|----|-------|------|------|-------|---------|-----------------|
| `lite` | Small Q5 | `ggml-small-q5_1.bin` | 190 MB | ★★★★ | ★★ | Quick notes, drafts, informal meetings |
| `medium` | Medium Q5 | `ggml-medium-q5_0.bin` | 515 MB | ★★★ | ★★★ | Good speed/accuracy balance |
| `turbo` | Large V3 Turbo Q5 | `ggml-large-v3-turbo-q5_0.bin` | 574 MB | ★★★ | ★★★★ | Maximum accuracy, professional transcriptions |

All models support 90+ languages. Q5 quantization reduces memory usage while maintaining quality close to the original float16 models.

Models are stored in:
- `models/` in the working directory (if present)
- Otherwise in `%APPDATA%/scriba/models` (Windows) / `~/Library/Application Support/scriba/models` (macOS) / `~/.local/share/scriba/models` (Linux)

## Quick start

Add to your `Cargo.toml`:

```toml
[dependencies]
scriba-rs = { git = "https://github.com/dariofinardi/scriba-rs.git", tag = "v20260606" }
```

Launch the transcription window:

```rust
fn main() {
    scriba_rs::run(
        scriba_rs::ScribaConfig {
            accent_color: (0, 120, 212),
            model: "turbo".into(),
            diarize: true,
            ..Default::default()
        },
        |result| {
            let json = serde_json::to_string_pretty(&result).unwrap();
            println!("{json}");
        },
    )
    .unwrap();
}
```

`run()` blocks until the window is closed. The callback is invoked when the user presses "Confirm" on the result.

## API

### `ScribaConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `accent_color` | `(u8, u8, u8)` | `(232, 89, 12)` | RGB accent color for the UI theme |
| `dark_mode` | `Option<bool>` | `None` | `None` = follow OS preference |
| `default_language` | `String` | `"auto"` | Language code or `"auto"` for detection |
| `model` | `String` | `"lite"` | Model ID: `"lite"`, `"medium"`, `"turbo"` |
| `diarize` | `bool` | `false` | Enable speaker identification |
| `app_name` | `String` | `"Scriba"` | Name shown in the title bar |
| `data_dir` | `Option<PathBuf>` | `None` | Custom directory for models and config |
| `always_on_top` | `bool` | `true` | Keep window always on top |

### `ScribaResult`

Returned to the `on_result` callback, JSON-serializable via serde.

| Field | Type | Description |
|-------|------|-------------|
| `text` | `String` | Full transcript text |
| `segments` | `Vec<ResultSegment>` | Timed segments with optional speaker labels |
| `audio_duration_secs` | `f64` | Source audio duration in seconds |
| `model` | `String` | Model used for transcription |
| `language` | `String` | Detected or selected language |
| `diarized` | `bool` | Whether diarization was applied |
| `transcription_time_secs` | `f64` | Whisper inference time |
| `diarization_time_secs` | `f64` | Diarization time (0.0 if disabled) |

### Entry points

```rust
pub fn run<F>(config: ScribaConfig, on_result: F) -> anyhow::Result<()>

pub fn run_with_cancel<F, G>(
    config: ScribaConfig,
    on_result: F,
    on_cancel: Option<G>,
) -> anyhow::Result<()>
```

Both block until the window is closed. Must be called from the main thread (macOS requirement).

## Feature flags

| Flag | Description |
|------|-------------|
| `diarize` | Speaker identification via sherpa-rs + ONNX Runtime |
| `recorder` | BLE recorder support via mic-rs |
| `cuda` | NVIDIA GPU acceleration (CUDA) |
| `metal` | Apple GPU acceleration (Metal) |
| `vulkan` | Cross-platform GPU acceleration (Vulkan) |

## Building

### Standard build

```sh
cargo build --release
```

### With all optional features

```sh
cargo build --release --features "recorder,diarize"
```

### Windows ARM64 (Qualcomm Snapdragon)

Requires Ninja and clang-cl. See [scriba-core build instructions](https://github.com/dariofinardi/scriba-core-rs#windows-arm64-qualcomm-snapdragon) for environment setup.

**Note:** when the `diarize` feature is enabled, always build in release mode (`--release`). ONNX Runtime crashes in debug mode on ARM64.

### Running the example

```sh
cargo run --release --example basic
```

## Architecture

```
scriba-rs
├── src/
│   ├── lib.rs        # Public API (ScribaConfig, ScribaResult, run)
│   ├── gui.rs        # Slint window orchestration, callbacks, system tray
│   ├── recorder.rs   # Microphone capture (cpal)
│   ├── models.rs     # Model registry, download, path management
│   ├── whisper.rs    # Whisper transcription with progress
│   └── config.rs     # JSON config persistence
├── slint-ui/
│   ├── app.slint     # Main window layout
│   ├── theme.slint   # Colors, icons, enums
│   ├── widgets.slint # Reusable UI components
│   └── setup.slint   # Model setup panel
└── examples/
    └── basic.rs      # Minimal integration example
```

### Key dependencies

| Crate | Role |
|-------|------|
| [scriba-core](https://github.com/dariofinardi/scriba-core-rs) | Audio decoding, resampling, Whisper inference, diarization |
| [whisper-rs](https://github.com/dariofinardi/whisper-rs) | Rust bindings for whisper.cpp (fork with Windows fixes) |
| [slint](https://slint.dev) | Native UI framework, winit backend |
| [cpal](https://crates.io/crates/cpal) | Cross-platform audio capture |
| [tray-icon](https://crates.io/crates/tray-icon) | System tray icon |
| [rfd](https://crates.io/crates/rfd) | Native dialogs (file picker, confirmations) |
| [sherpa-rs](https://crates.io/crates/sherpa-rs) | Speaker diarization (optional) |
| [mic-rs](https://github.com/dariofinardi/mic-rs) | BLE Soundcore recorder (optional) |

## Platforms

| Platform | Status |
|----------|--------|
| Windows x86_64 | Tested |
| Windows ARM64 (Qualcomm Snapdragon X Elite) | Tested |
| macOS (Apple Silicon / Intel) | Not yet tested |
| Linux x86_64 | Not yet tested |

The codebase is cross-platform by design (Slint, cpal, and whisper.cpp all compile on every platform), but only Windows has been verified so far. Contributions for macOS and Linux testing are welcome.

## License

AGPL-3.0-or-later — see [LICENSE](LICENSE).

Copyright © 2026 Dario Finardi
