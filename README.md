# scriba-rs

Embeddable speech-to-text widget for Rust desktop applications. Drop a borderless, always-on-top transcription window into any app with a single function call — powered by [Whisper.cpp](https://github.com/ggerganov/whisper.cpp) and [Slint UI](https://slint.dev).

![License](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue)

## Overview

`scriba-rs` provides a complete local transcription UI: model selection, audio file import, live microphone recording, progress feedback, clipboard copy, and optional speaker diarization. It uses [scriba-core](https://github.com/dariofinardi/scriba-core-rs) for the headless audio pipeline and adds a polished Slint-based interface on top.

The window is **borderless** and **always-on-top** by default, with a custom title bar (drag, minimize, close with confirmation), a system tray icon, and full dark/light theme support.

## Quick start

Add to your `Cargo.toml`:

```toml
[dependencies]
scriba-rs = { git = "https://github.com/dariofinardi/scriba-rs.git", tag = "v20260606" }
```

Then launch the transcription window:

```rust
fn main() {
    scriba_rs::run(
        scriba_rs::ScribaConfig {
            accent_color: (0, 120, 212),
            model: "turbo".into(),
            ..Default::default()
        },
        |result| {
            println!("Transcription: {}", result.text);
            println!("Language: {}", result.language);
            println!("Duration: {:.1}s", result.audio_duration_secs);
        },
    )
    .unwrap();
}
```

`run()` blocks until the user closes the window. The callback fires when the user confirms the transcription result.

## API

### `ScribaConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `accent_color` | `(u8, u8, u8)` | `(232, 89, 12)` | RGB accent color for the UI theme |
| `dark_mode` | `Option<bool>` | `None` | `None` follows the OS preference |
| `default_language` | `String` | `"auto"` | Language code or `"auto"` for detection |
| `model` | `String` | `"lite"` | Whisper model: `"lite"`, `"medium"`, `"turbo"` |
| `diarize` | `bool` | `false` | Enable speaker diarization |
| `app_name` | `String` | `"Scriba"` | Name shown in the title bar |
| `data_dir` | `Option<PathBuf>` | `None` | Custom directory for models and config |
| `always_on_top` | `bool` | `true` | Keep window above all others |

### `ScribaResult`

Returned to the `on_result` callback, serializable to JSON via serde.

| Field | Type | Description |
|-------|------|-------------|
| `text` | `String` | Full transcript |
| `segments` | `Vec<ResultSegment>` | Timed segments with optional speaker labels |
| `audio_duration_secs` | `f64` | Source audio duration |
| `model` | `String` | Model used for transcription |
| `language` | `String` | Detected or selected language |
| `diarized` | `bool` | Whether diarization was applied |
| `transcription_time_secs` | `f64` | Whisper inference time |
| `diarization_time_secs` | `f64` | Diarization time (0.0 if disabled) |

### Entry points

```rust
// Basic — fires on_result when the user confirms
pub fn run<F>(config: ScribaConfig, on_result: F) -> anyhow::Result<()>

// With optional cancellation callback
pub fn run_with_cancel<F, G>(
    config: ScribaConfig,
    on_result: F,
    on_cancel: Option<G>,
) -> anyhow::Result<()>
```

Both functions block until the window is closed. Must be called from the main thread (macOS requirement).

## UI features

- **Borderless window** with custom drag-to-move title bar
- **Always-on-top** (configurable)
- **Minimize to taskbar** + system tray icon with Show/About menu
- **Close confirmation** dialog to prevent accidental exit
- **Model management** — download, select, and switch Whisper models from the built-in setup panel
- **Audio file import** — drag-and-drop or file picker (WAV, MP3, OGG)
- **Live microphone recording** — select input device, real-time capture
- **Transcription progress** with ETA
- **Clipboard copy** with one click
- **Speaker diarization** — identify who is speaking (optional feature)
- **Dark / light theme** — follows system or set explicitly
- **Customizable accent color** — match your app's branding

## Feature flags

| Flag | Description |
|------|-------------|
| `diarize` | Speaker diarization via sherpa-rs |
| `recorder` | BLE microphone support via mic-rs |
| `cuda` | GPU acceleration (CUDA) |
| `metal` | GPU acceleration (Metal, macOS) |
| `vulkan` | GPU acceleration (Vulkan) |

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

**Important:** when the `diarize` feature is enabled, always build in release mode (`--release`). The ONNX Runtime used by sherpa-rs crashes in debug mode on ARM64.

## Running the example

```sh
cargo run --release --example basic
```

## Architecture

```
scriba-rs
├── src/
│   ├── lib.rs        # Public API (ScribaConfig, ScribaResult, run)
│   ├── gui.rs        # Slint window orchestration, callbacks, tray icon
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

`scriba-rs` depends on [scriba-core](https://github.com/dariofinardi/scriba-core-rs) for audio decoding, resampling, and Whisper inference. The UI layer adds Slint rendering, microphone management, model downloads, and the callback-based API.

## Supported platforms

- Windows x86_64
- Windows ARM64 (Qualcomm Snapdragon X Elite)
- macOS (Apple Silicon / Intel)
- Linux x86_64

## License

AGPL-3.0-or-later — see [LICENSE](LICENSE) for details.

Copyright © 2026 Dario Finardi
