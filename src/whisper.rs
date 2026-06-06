use std::sync::{Arc, Mutex};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct TranscribeResult {
    pub text: String,
    pub segments: Vec<(f32, f32, String)>,
    pub detected_lang: String,
}

pub fn transcribe(
    model_path: &std::path::Path,
    lang: &str,
    audio: &[f32],
    progress_cb: Arc<dyn Fn(f32) + Send + Sync>,
) -> anyhow::Result<TranscribeResult> {
    let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())?;
    let mut state = ctx.create_state()?;

    let effective_lang = if lang == "auto" {
        let mut detect_params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        detect_params.set_detect_language(true);
        detect_params.set_print_special(false);
        detect_params.set_print_progress(false);
        detect_params.set_print_realtime(false);
        detect_params.set_print_timestamps(false);
        state.full(detect_params, audio)?;

        let lang_id = state.full_lang_id_from_state();
        let detected = whisper_rs::get_lang_str(lang_id).unwrap_or("en");
        eprintln!("[scriba] detected language: {}", detected);
        detected.to_string()
    } else {
        lang.to_string()
    };

    let mut state = ctx.create_state()?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some(&effective_lang));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    let pcb = progress_cb.clone();
    let max_progress = Arc::new(Mutex::new(0i32));
    params.set_progress_callback_safe(move |progress: i32| {
        let mut max = max_progress.lock().unwrap();
        if progress > *max {
            *max = progress;
            pcb(progress as f32 / 100.0);
        }
    });

    state.full(params, audio)?;

    let mut text = String::new();
    let mut segments = Vec::new();
    for seg in state.as_iter() {
        let t0 = seg.start_timestamp() as f32 / 100.0;
        let t1 = seg.end_timestamp() as f32 / 100.0;
        let seg_text = seg.to_str_lossy().unwrap_or_default().to_string();
        text.push_str(&seg_text);
        segments.push((t0, t1, seg_text));
    }
    Ok(TranscribeResult {
        text: text.trim().to_string(),
        segments,
        detected_lang: effective_lang,
    })
}
