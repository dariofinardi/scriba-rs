mod gui;
pub mod i18n;
mod recorder;
mod models;
mod whisper;
mod config;

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Configuration for the Scriba transcription window.
#[derive(Debug, Clone)]
pub struct ScribaConfig {
    /// RGB accent color for the UI theme. Default: (232, 89, 12).
    pub accent_color: (u8, u8, u8),
    /// Dark mode. None = follow system preference. Default: None.
    pub dark_mode: Option<bool>,
    /// Default language code (e.g. "it", "en", "auto"). Default: "auto".
    pub default_language: String,
    /// Model ID to use ("lite", "medium", "turbo"). Default: "lite".
    pub model: String,
    /// Enable speaker diarization. Default: false.
    pub diarize: bool,
    /// Application name shown in the brand strip. Default: "Scriba".
    pub app_name: String,
    /// Custom data directory for models and config. None = platform default.
    pub data_dir: Option<PathBuf>,
    /// Keep window always on top. Default: true.
    pub always_on_top: bool,
    /// UI language code (e.g. "en", "it", "fr", "de", "es", "pt"). None = detect from system. Default: None.
    pub ui_language: Option<String>,
}

impl Default for ScribaConfig {
    fn default() -> Self {
        Self {
            accent_color: (232, 89, 12),
            dark_mode: None,
            default_language: "auto".to_string(),
            model: "lite".to_string(),
            diarize: false,
            app_name: "Scriba".to_string(),
            data_dir: None,
            always_on_top: true,
            ui_language: None,
        }
    }
}

/// Result delivered to the on_result callback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScribaResult {
    /// Full transcript text.
    pub text: String,
    /// Individual segments with timing.
    pub segments: Vec<ResultSegment>,
    /// Total audio duration in seconds.
    pub audio_duration_secs: f64,
    /// Model ID used (e.g. "turbo").
    pub model: String,
    /// Detected or selected language code.
    pub language: String,
    /// Whether diarization was applied.
    pub diarized: bool,
    /// Time spent on Whisper transcription, in seconds.
    pub transcription_time_secs: f64,
    /// Time spent on diarization, in seconds (0.0 if not applied).
    pub diarization_time_secs: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultSegment {
    /// Start time in seconds.
    pub start: f64,
    /// End time in seconds.
    pub end: f64,
    /// Transcribed text for this segment.
    pub text: String,
    /// Speaker label (e.g. "Speaker 1"). None if diarization was off.
    pub speaker: Option<String>,
}

/// Launch the Scriba transcription window.
///
/// Blocks until the window is closed. Must be called from the main thread (macOS requirement).
/// `on_result` is called when the user presses "Conferma". Not called if the user closes without confirming.
pub fn run<F>(config: ScribaConfig, on_result: F) -> anyhow::Result<()>
where
    F: FnOnce(ScribaResult) + 'static,
{
    run_with_cancel(config, on_result, None::<fn()>)
}

/// Launch with an optional on_cancel callback (called when user closes without confirming).
pub fn run_with_cancel<F, G>(
    config: ScribaConfig,
    on_result: F,
    on_cancel: Option<G>,
) -> anyhow::Result<()>
where
    F: FnOnce(ScribaResult) + 'static,
    G: FnOnce() + 'static,
{
    gui::launch(config, on_result, on_cancel)
}
