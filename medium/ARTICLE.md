# Scriba: Local Speech-to-Text in Rust — Architecture and Technical Deep Dive

Speech-to-text is a solved problem — if you're willing to send your audio to someone else's server. For everything else, there's a gap: local inference exists (whisper.cpp is excellent), but the tooling around it — audio decoding, resampling, model management, microphone capture, speaker identification, and a usable interface — is still scattered across dozens of libraries with no unified integration point.

Scriba fills that gap. It's a Rust crate that wraps the entire local transcription pipeline into a single function call: give it a config, get back a structured result. No network, no API keys, no recurring costs. Audio never leaves the device.

This article covers what Scriba does, how it's built, and what it deliberately doesn't do.

---

## What Scriba is

Scriba is a **desktop transcription widget** — a borderless, always-on-top window that any Rust application can summon with:

```rust
scriba_rs::run(config, |result| {
    // result: ScribaResult with text, segments, timings, speaker labels
});
```

The call blocks until the user closes the window. When they press "Confirm", the callback fires with a `ScribaResult` — full transcript text, timed segments with optional speaker labels, processing times, detected language. The struct is `Serialize`/`Deserialize` via serde, so piping it to JSON is one line.

The window handles everything: file import (WAV, MP3, OGG, FLAC via native file picker), microphone recording with device selection, model download and management, language selection, and result display with clipboard copy.

## Three crates, one pipeline

The project is a Cargo workspace with three crates:

```
Whisper-RS/
├── core/          → scriba-core   (headless engine, no UI)
├── scriba-rs/     → scriba-rs     (embeddable Slint widget)
└── whisper-cli/   → whisper-cli   (CLI transcription tool)
```

### scriba-core — the headless engine

[scriba-core](https://github.com/dariofinardi/scriba-core-rs) contains the transcription pipeline with zero UI dependencies. It's designed to be embedded in any Rust application — CLI tools, servers, batch processors — without pulling in a windowing framework.

The pipeline has four stages:

1. **Decode** — Symphonia handles WAV, MP3, and OGG. Multi-channel audio is mixed down to mono automatically.
2. **Resample** — Rubato performs sinc resampling to 16 kHz (the sample rate Whisper expects), using a 256-tap BlackmanHarris2 windowed filter. If the source is already 16 kHz, this is a passthrough.
3. **Transcribe** — whisper.cpp inference via the `TranscriberBackend` trait, returning segments with start/end timestamps. Supports automatic language detection, explicit language selection, and translation to English.
4. **Diarize** (optional) — sherpa-onnx speaker segmentation and embedding, identifying who is speaking in each segment.

Each stage is independently usable. You can decode without transcribing, or transcribe pre-decoded PCM, or diarize segments from any source.

### scriba-rs — the UI widget

[scriba-rs](https://github.com/dariofinardi/scriba-rs) depends on scriba-core and adds everything UI:

- **Slint** for the native interface (winit backend)
- **cpal** for real-time microphone capture
- **tray-icon** for system tray integration
- **rfd** for native file picker and confirmation dialogs
- Model registry with automatic download from Hugging Face
- JSON config persistence

The separation means the CLI tool doesn't link against Slint, and headless consumers of scriba-core pay no UI tax.

### whisper-cli — the command-line tool

`whisper-cli` is a ready-to-use CLI for file transcription and live microphone input. It depends only on scriba-core — no UI overhead.

```sh
# Transcribe a file with auto-detected language
whisper-cli --model ggml-large-v3-turbo-q5_0.bin file recording.wav

# Live microphone transcription
whisper-cli --model ggml-small-q5_1.bin listen

# With speaker diarization (requires --features diarize)
whisper-cli --model ggml-medium-q5_0.bin --diarize file meeting.wav
```

When diarization is enabled, the output includes speaker labels and a merged transcript:

```
[Speaker 1]  Buongiorno Marco, come stai oggi?
[Speaker 2]  Ciao Isabella, tutto bene e grazie. E tu come stai?
```

## Whisper models

Scriba uses quantized GGML models from the whisper.cpp ecosystem. Three are built into the model registry:

| ID | Model | Size | Trade-off |
|----|-------|------|-----------|
| `lite` | Small Q5 | 190 MB | Fastest inference, lower accuracy. Good for drafts and quick notes. |
| `medium` | Medium Q5 | 515 MB | Balanced. The default choice for most use cases. |
| `turbo` | Large V3 Turbo Q5 | 574 MB | Best accuracy. Noticeably slower, but worth it for professional transcription. |

All three support 90+ languages. Q5 quantization reduces memory usage and model size substantially — the full float16 Large V3 Turbo is 3.1 GB; the Q5 version is 574 MB — while keeping quality within a few percent of the original.

Models are downloaded automatically on first use. Storage follows platform conventions: `%APPDATA%/scriba/models` on Windows, `~/Library/Application Support/scriba/models` on macOS, `~/.local/share/scriba/models` on Linux. A `models/` directory in the working directory takes precedence if it exists.

## Speaker diarization

When the `diarize` feature flag is enabled, Scriba identifies speakers in the transcript. Each segment gets a `speaker` label ("Speaker 1", "Speaker 2", etc.) based on voice embedding similarity.

The diarization pipeline uses sherpa-onnx models:

- **Segmentation** — pyannote-based model that detects speech boundaries and speaker turns
- **Embedding** — speaker embedding model that computes voice signatures for clustering

Both models are downloaded automatically on first use (~80 MB total). Diarization runs as a post-processing step after Whisper transcription — it takes the audio and the segment boundaries, computes embeddings per segment, and clusters them.

One important constraint: on Windows ARM64, ONNX Runtime crashes in debug mode. Diarization requires `--release` builds on that platform.

## The UI

Scriba's window is **borderless** (`no-frame: true` in Slint) and **always-on-top**, designed to float above the user's working application without getting in the way. A custom 42px title bar provides drag-to-move, minimize, and close (with confirmation dialog to prevent accidental transcript loss).

The application starts minimized to the system tray. The tray icon provides "Show Scriba" and "About..." menu items.

Theme support includes light and dark modes (follows OS preference by default, or can be forced via config) with a customizable accent color passed as an RGB tuple in `ScribaConfig`.

The UI state machine is straightforward:

```
idle → recording → transcribing → [diarizing →] result
idle → (file import) → transcribing → [diarizing →] result
```

During transcription, a progress indicator shows the percentage. Results display in a scrollable area with timed segments, optional speaker labels, and a "Copy" button. If diarization ran, each segment is prefixed with its speaker label.

## Configuration and API surface

### ScribaConfig

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `accent_color` | `(u8, u8, u8)` | `(232, 89, 12)` | RGB accent for the UI theme |
| `dark_mode` | `Option<bool>` | `None` | `None` follows the OS |
| `default_language` | `String` | `"auto"` | Language code or `"auto"` |
| `model` | `String` | `"lite"` | Model ID |
| `diarize` | `bool` | `false` | Enable speaker identification |
| `app_name` | `String` | `"Scriba"` | Shown in the title bar |
| `data_dir` | `Option<PathBuf>` | `None` | Custom storage directory |
| `always_on_top` | `bool` | `true` | Keep window above others |

### ScribaResult

The result struct is intentionally flat and complete:

```rust
pub struct ScribaResult {
    pub text: String,                    // full transcript
    pub segments: Vec<ResultSegment>,    // timed segments
    pub audio_duration_secs: f64,        // source audio length
    pub model: String,                   // which model was used
    pub language: String,                // detected or selected
    pub diarized: bool,                  // whether diarization ran
    pub transcription_time_secs: f64,    // Whisper inference time
    pub diarization_time_secs: f64,      // diarization time (0 if disabled)
}
```

No enums, no nested option types, no builder patterns. A consumer can serialize it to JSON and be done.

### Entry points

```rust
pub fn run<F>(config: ScribaConfig, on_result: F) -> anyhow::Result<()>

pub fn run_with_cancel<F, G>(
    config: ScribaConfig,
    on_result: F,
    on_cancel: Option<G>,
) -> anyhow::Result<()>
```

Both block until the window is closed. Must be called from the main thread (macOS requirement). `run_with_cancel` adds an optional callback for when the user closes without confirming.

## Feature flags

| Flag | What it adds |
|------|--------------|
| `diarize` | Speaker identification (sherpa-rs + ONNX Runtime) |
| `recorder` | BLE recorder support (mic-rs, for Soundcore-compatible Bluetooth devices) |
| `cuda` | NVIDIA GPU acceleration |
| `metal` | Apple GPU acceleration |
| `vulkan` | Cross-platform GPU acceleration |

Features are additive — the base crate has no optional dependencies enabled. GPU acceleration flags are passed through to whisper-rs and affect Whisper inference only.

## BLE recorder support

The optional `recorder` feature integrates [mic-rs](https://github.com/dariofinardi/mic-rs), a crate for communicating with Bluetooth Low Energy audio recorders. The current implementation targets Anker Soundcore devices — small, portable BLE recorders that capture audio locally and transfer it over Bluetooth when connected.

When enabled, Scriba can detect a paired BLE recorder, trigger recording, download the captured audio over Bluetooth, and transcribe it — all from the UI. The workflow is: record on the go with the Anker in your pocket, then connect to the PC and Scriba pulls the audio and transcribes it automatically. This is a niche feature for specific hardware, hence the feature flag.

## Lessons from the trenches

Building a native Whisper integration in Rust surfaces issues that higher-level wrappers hide. Two examples:

### The auto-detect trap

whisper.cpp exposes a `detect_language` flag in its inference parameters. The whisper-rs Rust bindings faithfully wrap it as `set_detect_language(true)`. The documentation says it's equivalent to setting the language to `None` (auto). In practice, with recent whisper.cpp versions, enabling this flag runs only the language detection forward pass — it identifies the language correctly but returns zero transcription segments.

The fix is to use `set_language(None)` instead, which triggers auto-detection *and* runs the full inference. A one-line change, but one that turns a working feature into silent failure — the API returns `Ok(vec![])`, no error, no warning.

### Building on Windows ARM64

The primary development machine is a Qualcomm Snapdragon X Elite. This is not a well-trodden path for native C++ compilation. whisper.cpp's CMakeLists.txt explicitly rejects MSVC for ARM targets, requiring clang-cl and the Ninja build generator instead. The Rust `cmake` crate auto-detects "Visual Studio 18 2026" as the generator, which doesn't exist — Ninja must be forced via environment variable.

ONNX Runtime (used by sherpa-rs for diarization) introduces another constraint: it crashes in debug mode on ARM64, likely due to incompatible debug assertions between clang-cl-built code and the MSVC debug runtime. All builds with diarization must use `--release`.

sherpa-onnx's CMake scripts also assume Visual Studio's `CMAKE_VS_PLATFORM_NAME` variable to select the correct ONNX Runtime architecture. Under Ninja, this variable is unset, causing it to fall back to x64 binaries on an ARM64 host. The workaround is a custom CMake toolchain file that sets `CMAKE_VS_PLATFORM_NAME=ARM64` explicitly.

Windows 11 removed `wmic.exe`, which sherpa-onnx's build scripts call to detect the OS version. The build crashes with an empty output. A minimal C shim that prints a static version string resolves it.

None of these are bugs in the individual projects — they're edge cases at the intersection of a new platform, multiple build systems, and native code compilation through Rust's `cc`/`cmake` crates. Documenting them here because finding each one cost hours.

## What Scriba doesn't do

Some explicit non-goals and current limitations:

- **No streaming transcription from files.** Whisper processes audio in fixed windows. Scriba transcribes the full audio after recording or import, not in real time during playback.
- **No cloud fallback.** If local inference is too slow for your hardware, there's no option to offload to a server. This is by design.
- **No text editing.** The result is read-only. Scriba is a transcription tool, not an editor. Post-processing (correction, formatting, summarization) belongs in the consuming application.
- **No training or fine-tuning.** Scriba uses pre-trained Whisper models as-is. Model quality is upstream's responsibility.
- **No real-time streaming from microphone to text.** The live microphone feature in whisper-cli uses overlapping windows and processes each chunk, but it's not true streaming — there's latency proportional to the chunk size.
- **macOS and Linux are untested.** The codebase is cross-platform by design (whisper.cpp, Slint, cpal, and Symphonia all support these platforms), but only Windows x86_64 and Windows ARM64 (Qualcomm Snapdragon X Elite) have been verified. Contributions for other platforms are welcome.

## Building

### Individual crates

```sh
# Headless engine only
cargo build --release -p scriba-core

# CLI tool
cargo build --release -p whisper-cli

# CLI with diarization
cargo build --release -p whisper-cli --features diarize
```

### Full UI with all features

```sh
cargo build --release -p scriba-rs --features "recorder,diarize"
```

The `recorder` feature links mic-rs for BLE support. The `diarize` feature links sherpa-rs + ONNX Runtime for speaker identification. Both are optional — the base UI works without them.

### Windows ARM64

Building on Qualcomm Snapdragon requires Ninja, clang-cl, and a custom toolchain file:

```powershell
$env:PATH = "cmake;" + $env:PATH
$env:CMAKE_TOOLCHAIN_FILE = "cmake/arm64-toolchain.cmake"
$env:CMAKE_GENERATOR = "Ninja"
$env:CMAKE_C_COMPILER = "clang-cl"
$env:CMAKE_CXX_COMPILER = "clang-cl"
$env:CMAKE_ASM_COMPILER = "clang-cl"
$env:GGML_NATIVE = "OFF"

cargo build --release -p scriba-rs --features "recorder,diarize"
```

The `cmake/` directory in the repository contains the ARM64 toolchain file and a wmic.exe shim for Windows 11 compatibility. `GGML_NATIVE=OFF` disables CPU-specific optimizations that cause cross-compile test failures on ARM64.

## Key dependencies

| Crate | Role |
|-------|------|
| [whisper-rs](https://github.com/dariofinardi/whisper-rs) | Rust bindings for whisper.cpp (fork with Windows ARM64 fixes) |
| [slint](https://slint.dev) | Native UI framework |
| [symphonia](https://crates.io/crates/symphonia) | Multi-format audio decoding |
| [rubato](https://crates.io/crates/rubato) | High-quality sinc resampling |
| [cpal](https://crates.io/crates/cpal) | Cross-platform audio capture |
| [tray-icon](https://crates.io/crates/tray-icon) | System tray integration |
| [rfd](https://crates.io/crates/rfd) | Native file picker and dialogs |
| [sherpa-rs](https://crates.io/crates/sherpa-rs) | Speaker diarization (optional) |
| [mic-rs](https://github.com/dariofinardi/mic-rs) | BLE recorder support (optional) |

## Platform status

| Platform | Status |
|----------|--------|
| Windows x86_64 | Tested |
| Windows ARM64 (Qualcomm Snapdragon X Elite) | Tested |
| macOS (Apple Silicon / Intel) | Not yet tested |
| Linux x86_64 | Not yet tested |

---

## License

Scriba is released under AGPL-3.0-or-later.

Copyright © 2026 Dario Finardi

- [scriba-rs on GitHub](https://github.com/dariofinardi/scriba-rs)
- [scriba-core-rs on GitHub](https://github.com/dariofinardi/scriba-core-rs)
