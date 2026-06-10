use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slint::{Model, ModelRc, SharedString, Timer, TimerMode, VecModel};

use crate::{ScribaConfig, ScribaResult, ResultSegment};
use crate::config;
use crate::i18n::{self, Locale, Translations};
use crate::models;
use crate::recorder::{self, Recorder};
use crate::whisper;

slint::include_modules!();

fn eta_string(start: Instant, progress: f32) -> String {
    if progress < 0.05 { return "00:00".to_string(); }
    let elapsed = start.elapsed().as_secs_f32();
    let total_est = elapsed / progress;
    let remaining = (total_est - elapsed).max(0.0) as u64;
    format!("{:02}:{:02}", remaining / 60, remaining % 60)
}

fn build_model_items(selected: &str, data_dir: Option<&std::path::Path>, t: &'static Translations) -> Vec<ModelItem> {
    models::registry().iter().map(|m| {
        let note = match m.id {
            "lite" => t.model_lite_note,
            "medium" => t.model_medium_note,
            "turbo" => t.model_turbo_note,
            _ => m.note,
        };
        ModelItem {
        id: m.id.into(),
        name: m.name.into(),
        size: m.size.into(),
        note: note.into(),
        speed: m.speed,
        quality: m.quality,
        installed: models::is_installed(m.id, data_dir),
        selected: m.id == selected,
        downloading: false,
        progress: 0.0,
        progress_label: SharedString::new(),
    }}).collect()
}

fn apply_translations(ui: &AppWindow, t: &Translations) {
    let tr = ui.global::<Tr>();
    tr.set_subtitle(t.subtitle.into());
    tr.set_mic_label(t.mic_label.into());
    tr.set_lang_label(t.lang_label.into());
    tr.set_btn_setup(t.btn_setup.into());
    tr.set_btn_copy(t.btn_copy.into());
    tr.set_btn_confirm(t.btn_confirm.into());
    tr.set_btn_delete(t.btn_delete.into());
    tr.set_btn_keep(t.btn_keep.into());
    tr.set_status_transcribing(t.status_transcribing.into());
    tr.set_status_diarizing(t.status_diarizing.into());
    tr.set_status_recording(t.status_recording.into());
    tr.set_idle_title(t.idle_title.into());
    tr.set_idle_hint(t.idle_hint.into());
    tr.set_state_recording(t.state_recording.into());
    tr.set_state_download(t.state_download.into());
    tr.set_state_transcribing(t.state_transcribing.into());
    tr.set_state_diarizing(t.state_diarizing.into());
    tr.set_state_done(t.state_done.into());
    tr.set_state_ready(t.state_ready.into());
    tr.set_setup_title(t.setup_title.into());
    tr.set_setup_subtitle(t.setup_subtitle.into());
    tr.set_setup_installed(t.setup_installed.into());
    tr.set_setup_speed(t.setup_speed.into());
    tr.set_setup_quality(t.setup_quality.into());
    tr.set_setup_in_use(t.setup_in_use.into());
    tr.set_setup_download(t.setup_download.into());
    tr.set_setup_cancel(t.setup_cancel.into());
    tr.set_setup_done(t.setup_done.into());
    tr.set_diarize_title(t.diarize_title.into());
    tr.set_diarize_subtitle(t.diarize_subtitle.into());
    tr.set_diarize_not_installed(t.diarize_not_installed.into());
    tr.set_diarize_installed(t.diarize_installed.into());
    tr.set_setup_ui_lang(t.setup_ui_lang.into());
    tr.set_privacy_note(t.privacy_note.into());
}

fn current_model_id(ui: &AppWindow) -> String {
    let m = ui.get_models();
    for i in 0..m.row_count() {
        if let Some(it) = m.row_data(i) {
            if it.selected { return it.id.to_string(); }
        }
    }
    "lite".to_string()
}

pub fn launch<F, G>(
    cfg: ScribaConfig,
    on_result: F,
    on_cancel: Option<G>,
) -> anyhow::Result<()>
where
    F: FnOnce(ScribaResult) + 'static,
    G: FnOnce() + 'static,
{
    let data_dir = cfg.data_dir.clone();
    let dd = data_dir.as_deref();

    let ui = AppWindow::new()?;

    // ---- theme ----
    let dark = cfg.dark_mode.unwrap_or(false);
    ui.global::<Theme>().set_dark(dark);
    let (r, g, b) = cfg.accent_color;
    ui.global::<Theme>().set_accent(slint::Color::from_rgb_u8(r, g, b));

    // ---- always on top ----
    ui.set_on_top(cfg.always_on_top);

    // ---- app name ----
    ui.set_app_name(cfg.app_name.clone().into());

    // ---- i18n ----
    let saved_cfg = config::load_config(dd);
    let locale = saved_cfg.ui_language.as_deref()
        .or(cfg.ui_language.as_deref())
        .map(Locale::from_code)
        .unwrap_or_else(Locale::detect_system);
    let t = i18n::translations(locale);
    apply_translations(&ui, t);
    ui.set_ui_lang(locale.code().into());

    // ---- microphones ----
    let mics = recorder::input_device_names();
    let default_mic = recorder::default_input_name()
        .or_else(|| mics.first().cloned())
        .unwrap_or_else(|| "\u{2014}".to_string());
    let mic_model: Rc<VecModel<SharedString>> =
        Rc::new(VecModel::from(mics.iter().map(|s| s.as_str().into()).collect::<Vec<SharedString>>()));
    ui.set_mics(ModelRc::from(mic_model));
    ui.set_mic(default_mic.as_str().into());

    // ---- languages ----
    let auto_label = t.auto_detect.to_string();
    let langs: Vec<(String, &str)> = vec![
        (auto_label.clone(), "auto"),
        ("Italiano".into(), "it"), ("English".into(), "en"), ("Espa\u{00f1}ol".into(), "es"),
        ("Fran\u{00e7}ais".into(), "fr"), ("Deutsch".into(), "de"), ("Portugu\u{00ea}s".into(), "pt"),
    ];
    let lang_codes: HashMap<String, String> =
        langs.iter().map(|(n, c)| (n.clone(), c.to_string())).collect();
    let lang_model: Rc<VecModel<SharedString>> =
        Rc::new(VecModel::from(langs.iter().map(|(n, _)| SharedString::from(n.as_str())).collect::<Vec<SharedString>>()));
    ui.set_languages(ModelRc::from(lang_model.clone()));

    let default_lang_label = langs.iter()
        .find(|(_, c)| *c == cfg.default_language)
        .map(|(n, _)| n.as_str())
        .unwrap_or(&auto_label);
    ui.set_language(default_lang_label.into());

    // ---- models + saved config ----
    let initial_model = if models::is_installed(&cfg.model, dd) {
        cfg.model.clone()
    } else if models::is_installed(&saved_cfg.model, dd) {
        saved_cfg.model.clone()
    } else {
        models::registry().iter()
            .find(|m| models::is_installed(m.id, dd))
            .map(|m| m.id.to_string())
            .unwrap_or_else(|| cfg.model.clone())
    };
    let model_items = Rc::new(VecModel::from(build_model_items(&initial_model, dd, t)));
    ui.set_models(ModelRc::from(model_items));
    ui.set_diarize_enabled(saved_cfg.diarize || cfg.diarize);
    ui.set_diarize_active(saved_cfg.diarize || cfg.diarize);

    // ---- shared state ----
    let rec: Rc<RefCell<Option<Recorder>>> = Rc::new(RefCell::new(None));
    let current_mic = Rc::new(RefCell::new(default_mic));
    let default_lang_code = lang_codes.get(default_lang_label)
        .cloned().unwrap_or_else(|| "auto".to_string());
    let current_lang = Rc::new(RefCell::new(default_lang_code));
    let current_ui_lang: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(
        saved_cfg.ui_language.or_else(|| cfg.ui_language.clone())
    ));
    let rec_start = Rc::new(RefCell::new(Instant::now()));

    // Arc<Mutex<>> so it can cross into upgrade_in_event_loop (Send required)
    let pending_result: Arc<Mutex<Option<PendingResult>>> = Arc::new(Mutex::new(None));
    let ble_file_ids: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
    // These stay Rc — only accessed from main-thread Slint callbacks
    let on_result_cb: Rc<RefCell<Option<Box<dyn FnOnce(ScribaResult)>>>> =
        Rc::new(RefCell::new(Some(Box::new(on_result))));
    let on_cancel_cb: Rc<RefCell<Option<Box<dyn FnOnce()>>>> =
        Rc::new(RefCell::new(on_cancel.map(|f| Box::new(f) as Box<dyn FnOnce()>)));

    // ---- timer ----
    let timer = Timer::default();
    {
        let weak = ui.as_weak();
        let rs = rec_start.clone();
        timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
            if let Some(ui) = weak.upgrade() {
                if ui.get_state() == AppState::Recording {
                    let s = rs.borrow().elapsed().as_secs();
                    ui.set_timer_text(format!("{:02}:{:02}", s / 60, s % 60).into());
                }
            }
        });
    }

    // ---- mic selection ----
    {
        let weak = ui.as_weak();
        let cm = current_mic.clone();
        ui.on_select_mic(move |v| {
            *cm.borrow_mut() = v.to_string();
            if let Some(ui) = weak.upgrade() { ui.set_mic(v); }
        });
    }

    // ---- language selection ----
    {
        let weak = ui.as_weak();
        let cl = current_lang.clone();
        let lc = lang_codes.clone();
        ui.on_select_language(move |v| {
            if let Some(code) = lc.get(v.as_str()) { *cl.borrow_mut() = code.clone(); }
            if let Some(ui) = weak.upgrade() { ui.set_language(v); }
        });
    }

    // ---- record / stop ----
    {
        let weak = ui.as_weak();
        let recorder = rec.clone();
        let cm = current_mic.clone();
        let cl = current_lang.clone();
        let rs = rec_start.clone();
        let dd = data_dir.clone();
        let pr = pending_result.clone();
        ui.on_toggle_record(move || {
            let ui = match weak.upgrade() { Some(u) => u, None => return };
            match ui.get_state() {
                AppState::Recording => {
                    if let Some(rec) = recorder.borrow_mut().take() {
                        let samples = rec.finish();
                        let dur_secs = samples.len() as f64 / 16000.0;
                        let dh = dur_secs as u64 / 3600;
                        let dm = (dur_secs as u64 % 3600) / 60;
                        let ds = dur_secs as u64 % 60;
                        ui.set_state(AppState::Transcribing);
                        ui.set_source_name(format!("{} ({dh:02}:{dm:02}:{ds:02})", t.mic_recording).into());
                        let lang = cl.borrow().clone();
                        let mid = current_model_id(&ui);
                        let do_diarize = ui.get_diarize_active();
                        let weak2 = ui.as_weak();
                        let weak3 = weak2.clone();
                        let dd2 = dd.clone();
                        let pr2 = pr.clone();
                        std::thread::spawn(move || {
                            let t_start = Instant::now();
                            let pcb: Arc<dyn Fn(f32) + Send + Sync> = {
                                let w = weak3.clone();
                                Arc::new(move |p: f32| {
                                    let eta = eta_string(t_start, p);
                                    let w2 = w.clone();
                                    let _ = w2.upgrade_in_event_loop(move |ui| {
                                        ui.set_transcribe_progress(p);
                                        ui.set_timer_text(eta.into());
                                    });
                                })
                            };
                            let result = run_transcription(
                                &mid, &lang, &samples, dur_secs, do_diarize,
                                dd2.as_deref(), pcb, weak3, t,
                            );
                            let _ = weak2.upgrade_in_event_loop(move |ui| {
                                match result {
                                    Ok(pr_val) => {
                                        ui.set_transcript(pr_val.text.clone().into());
                                        *pr2.lock().unwrap() = Some(pr_val);
                                    }
                                    Err(e) => {
                                        ui.set_transcript(format!("\u{26a0}\u{fe0e} {e}").into());
                                    }
                                }
                                ui.set_transcribe_progress(0.0);
                                ui.set_timer_text("00:00".into());
                                ui.set_state(AppState::Result);
                            });
                        });
                    } else {
                        ui.set_state(AppState::Idle);
                    }
                }
                _ => {
                    let mic = cm.borrow().clone();
                    match Recorder::start(&mic) {
                        Ok(r) => {
                            *recorder.borrow_mut() = Some(r);
                            *rs.borrow_mut() = Instant::now();
                            ui.set_timer_text("00:00".into());
                            ui.set_transcript("".into());
                            ui.set_state(AppState::Recording);
                        }
                        Err(e) => {
                            ui.set_transcript(format!("\u{26a0}\u{fe0e} {}: {e}", t.mic_unavailable).into());
                            ui.set_state(AppState::Result);
                        }
                    }
                }
            }
        });
    }

    // ---- open file ----
    {
        let weak = ui.as_weak();
        let cl = current_lang.clone();
        let dd = data_dir.clone();
        let pr = pending_result.clone();
        ui.on_open_file(move || {
            let ui = match weak.upgrade() { Some(u) => u, None => return };
            ui.set_on_top(false);
            use i_slint_backend_winit::WinitWindowAccessor;
            let path = ui.window().with_winit_window(|w: &winit::window::Window| {
                rfd::FileDialog::new()
                    .add_filter("Audio", &["wav", "mp3", "ogg", "flac"])
                    .set_parent(w)
                    .pick_file()
            }).flatten();
            ui.set_on_top(true);
            if let Some(path) = path {
                ui.set_state(AppState::Transcribing);
                ui.set_transcript("".into());
                let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                ui.set_source_name(fname.clone().into());
                let lang = cl.borrow().clone();
                let mid = current_model_id(&ui);
                let do_diarize = ui.get_diarize_active();
                let weak2 = ui.as_weak();
                let weak3 = weak2.clone();
                let dd2 = dd.clone();
                let pr2 = pr.clone();
                std::thread::spawn(move || {
                    let t_start = Instant::now();
                    let pcb: Arc<dyn Fn(f32) + Send + Sync> = {
                        let w = weak3.clone();
                        Arc::new(move |p: f32| {
                            let eta = eta_string(t_start, p);
                            let w2 = w.clone();
                            let _ = w2.upgrade_in_event_loop(move |ui| {
                                ui.set_transcribe_progress(p);
                                ui.set_timer_text(eta.into());
                            });
                        })
                    };
                    let result = (|| -> anyhow::Result<PendingResult> {
                        let decoded = scriba_core::audio::decode_audio_file(&path)?;
                        let dur_secs = decoded.samples.len() as f64 / decoded.sample_rate as f64;
                        let dh = dur_secs as u64 / 3600;
                        let dm = (dur_secs as u64 % 3600) / 60;
                        let ds = dur_secs as u64 % 60;
                        let label = format!("{fname} ({dh:02}:{dm:02}:{ds:02})");
                        let w = weak3.clone();
                        let _ = w.upgrade_in_event_loop(move |ui| {
                            ui.set_source_name(label.into());
                        });
                        let pcm = scriba_core::audio::resample_to_16khz(&decoded.samples, decoded.sample_rate)?;
                        run_transcription(
                            &mid, &lang, &pcm, dur_secs, do_diarize,
                            dd2.as_deref(), pcb, weak3, t,
                        )
                    })();
                    let _ = weak2.upgrade_in_event_loop(move |ui| {
                        match result {
                            Ok(pr_val) => {
                                ui.set_transcript(pr_val.text.clone().into());
                                *pr2.lock().unwrap() = Some(pr_val);
                            }
                            Err(e) => {
                                ui.set_transcript(format!("\u{26a0}\u{fe0e} {e}").into());
                            }
                        }
                        ui.set_transcribe_progress(0.0);
                        ui.set_timer_text("00:00".into());
                        ui.set_state(AppState::Result);
                    });
                });
            }
        });
    }

    // ---- choose model ----
    {
        let weak = ui.as_weak();
        let dd = data_dir.clone();
        let cul = current_ui_lang.clone();
        ui.on_choose_model(move |id| {
            if let Some(ui) = weak.upgrade() {
                let m = ui.get_models();
                for i in 0..m.row_count() {
                    if let Some(mut it) = m.row_data(i) {
                        it.selected = it.id == id;
                        m.set_row_data(i, it);
                    }
                }
                config::save_config(&id, ui.get_diarize_enabled(), cul.borrow().as_deref(), dd.as_deref());
            }
        });
    }

    // ---- download model ----
    {
        let weak = ui.as_weak();
        let dd = data_dir.clone();
        ui.on_download_model(move |id| {
            let ui = match weak.upgrade() { Some(u) => u, None => return };
            let m = ui.get_models();
            for i in 0..m.row_count() {
                if let Some(mut it) = m.row_data(i) {
                    if it.id == id {
                        it.downloading = true;
                        it.progress = 0.0;
                        it.progress_label = t.download_starting.into();
                        m.set_row_data(i, it);
                    }
                }
            }

            let id_s = id.to_string();
            let weak2 = ui.as_weak();
            let dd2 = dd.clone();
            std::thread::spawn(move || {
                let mut last_posted: u64 = 0;
                let id_p = id_s.clone();
                let weak3 = weak2.clone();
                let res = models::download_model(&id_s, dd2.as_deref(), |dl, total| {
                    if dl.saturating_sub(last_posted) < 4_000_000 && total.map(|t| dl < t).unwrap_or(true) {
                        return;
                    }
                    last_posted = dl;
                    let frac = total.map(|t| dl as f32 / t as f32).unwrap_or(0.0);
                    let label = match total {
                        Some(t) => format!("{} / {}", models::human_bytes(dl), models::human_bytes(t)),
                        None => models::human_bytes(dl),
                    };
                    let idp = id_p.clone();
                    let _ = weak3.upgrade_in_event_loop(move |ui| {
                        let m = ui.get_models();
                        for i in 0..m.row_count() {
                            if let Some(mut it) = m.row_data(i) {
                                if it.id.as_str() == idp {
                                    it.progress = frac;
                                    it.progress_label = label.clone().into();
                                    m.set_row_data(i, it);
                                }
                            }
                        }
                    });
                });

                let ok = res.is_ok();
                let err_txt = res.err().map(|e| e.to_string());
                let ids = id_p.clone();
                let _ = weak2.upgrade_in_event_loop(move |ui| {
                    let m = ui.get_models();
                    for i in 0..m.row_count() {
                        if let Some(mut it) = m.row_data(i) {
                            if it.id.as_str() == ids {
                                it.downloading = false;
                                it.progress = 0.0;
                                it.progress_label = SharedString::new();
                                if ok { it.installed = true; }
                            }
                            if ok { it.selected = it.id.as_str() == ids; }
                            m.set_row_data(i, it);
                        }
                    }
                    if let Some(e) = &err_txt {
                        ui.set_transcript(format!("\u{26a0}\u{fe0e} {}: {e}", t.download_failed).into());
                    }
                });
            });
        });
    }

    // ---- cancel transcribe ----
    {
        let weak = ui.as_weak();
        ui.on_cancel_transcribe(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_state(AppState::Idle);
                ui.set_transcript("".into());
                ui.set_transcribe_progress(0.0);
            }
        });
    }

    // ---- copy transcript ----
    {
        let weak = ui.as_weak();
        ui.on_copy_text(move || {
            if let Some(ui) = weak.upgrade() {
                let text = ui.get_transcript().to_string();
                if let Ok(mut clip) = arboard::Clipboard::new() {
                    let _ = clip.set_text(text);
                }
            }
        });
    }

    // ---- confirm result ----
    {
        let weak = ui.as_weak();
        let pr = pending_result.clone();
        let cb = on_result_cb.clone();
        ui.on_confirm_result(move || {
            if let Some(pr_val) = pr.lock().unwrap().take() {
                if let Some(f) = cb.borrow_mut().take() {
                    f(pr_val.into_scriba_result());
                }
            }
            if let Some(ui) = weak.upgrade() {
                let _ = ui.hide();
            }
        });
    }

    // ---- minimize window ----
    {
        let weak = ui.as_weak();
        ui.on_minimize_window(move || {
            if let Some(ui) = weak.upgrade() {
                use i_slint_backend_winit::WinitWindowAccessor;
                ui.window().with_winit_window(|w: &winit::window::Window| {
                    w.set_minimized(true);
                });
            }
        });
    }

    // ---- drag window ----
    {
        let weak = ui.as_weak();
        ui.on_start_drag(move || {
            if let Some(ui) = weak.upgrade() {
                use i_slint_backend_winit::WinitWindowAccessor;
                ui.window().with_winit_window(|w: &winit::window::Window| {
                    let _ = w.drag_window();
                });
            }
        });
    }

    // ---- close window (with confirmation) ----
    {
        let weak = ui.as_weak();
        let cc = on_cancel_cb.clone();
        ui.on_close_window(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_on_top(false);
                let confirmed = rfd::MessageDialog::new()
                    .set_title(t.close_title)
                    .set_description(t.close_message)
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show();
                if confirmed == rfd::MessageDialogResult::Yes {
                    if let Some(f) = cc.borrow_mut().take() {
                        f();
                    }
                    let _ = ui.hide();
                } else {
                    ui.set_on_top(true);
                }
            }
        });
    }

    // ---- system tray icon ----
    let _tray_state = setup_tray_icon(&ui, &cfg, t);

    // ---- delete / keep device files ----
    {
        let weak = ui.as_weak();
        let ids = ble_file_ids.clone();
        ui.on_delete_device_files(move || {
            let file_ids: Vec<u32> = ids.lock().unwrap().drain(..).collect();
            if file_ids.is_empty() { return; }
            if let Some(ui) = weak.upgrade() {
                ui.set_show_delete_prompt(false);
            }
            #[cfg(feature = "recorder")]
            {
                let weak2 = weak.clone();
                std::thread::spawn(move || {
                    ble_delete_from_device(file_ids, weak2, t);
                });
            }
        });
    }
    {
        let weak = ui.as_weak();
        let ids = ble_file_ids.clone();
        ui.on_keep_device_files(move || {
            ids.lock().unwrap().clear();
            if let Some(ui) = weak.upgrade() {
                ui.set_show_delete_prompt(false);
            }
        });
    }

    // ---- recorder available ----
    #[cfg(feature = "recorder")]
    ui.set_recorder_available(true);

    // ---- open recorder (BLE) ----
    {
        let weak = ui.as_weak();
        let _cl = current_lang.clone();
        let _dd = data_dir.clone();
        let _pr = pending_result.clone();
        let _ids = ble_file_ids.clone();
        ui.on_open_recorder(move || {
            let ui = match weak.upgrade() { Some(u) => u, None => return };
            let st = ui.get_state();
            if st != AppState::Idle && st != AppState::Result { return; }

            ui.set_state(AppState::Downloading);
            ui.set_download_label(t.ble_scanning.into());
            ui.set_timer_text("".into());
            ui.set_transcript("".into());
            ui.set_transcribe_progress(0.0);
            ui.set_show_delete_prompt(false);

            let weak2 = ui.as_weak();

            #[cfg(feature = "recorder")]
            {
                let lang = _cl.borrow().clone();
                let mid = current_model_id(&ui);
                let do_diarize = ui.get_diarize_active();
                let dd2 = _dd.clone();
                let pr2 = _pr.clone();
                let ids2 = _ids.clone();
                std::thread::spawn(move || {
                    ble_recorder_flow(weak2, dd2, lang, mid, do_diarize, pr2, ids2, t);
                });
            }
            #[cfg(not(feature = "recorder"))]
            {
                let _ = weak2.upgrade_in_event_loop(move |ui| {
                    ui.set_transcript(format!("\u{26a0}\u{fe0e} {}", t.ble_feature_missing).into());
                    ui.set_state(AppState::Result);
                });
            }
        });
    }

    // ---- diarization state ----
    #[cfg(feature = "diarize")]
    ui.set_diarize_models_installed(scriba_core::diarize::models_installed());
    #[cfg(not(feature = "diarize"))]
    ui.set_diarize_models_installed(false);

    // ---- toggle diarize (from Setup — persists) ----
    {
        let weak = ui.as_weak();
        let dd = data_dir.clone();
        let cul = current_ui_lang.clone();
        ui.on_toggle_diarize(move |enabled| {
            if let Some(ui) = weak.upgrade() {
                ui.set_diarize_enabled(enabled);
                ui.set_diarize_active(enabled);
                let mid = current_model_id(&ui);
                config::save_config(&mid, enabled, cul.borrow().as_deref(), dd.as_deref());
            }
        });
    }

    // ---- download diarize models ----
    {
        let _weak = ui.as_weak();
        ui.on_download_diarize_models(move || {
            #[cfg(feature = "diarize")]
            {
                let weak2 = _weak.clone();
                let weak3 = _weak.clone();
                let _ = _weak.upgrade_in_event_loop(move |ui| {
                    ui.set_diarize_status(t.diarize_download.into());
                });
                std::thread::spawn(move || {
                    let w = weak3.clone();
                    let res = scriba_core::diarize::download_models(|dl, total| {
                        let pct = total.map(|t| (dl as f32 / t as f32 * 100.0) as u32).unwrap_or(0);
                        let w2 = w.clone();
                        let _ = w2.upgrade_in_event_loop(move |ui| {
                            ui.set_diarize_status(format!("{pct}%").into());
                        });
                    });
                    let _ = weak2.upgrade_in_event_loop(move |ui| {
                        match res {
                            Ok(()) => {
                                ui.set_diarize_models_installed(true);
                                ui.set_diarize_enabled(true);
                                ui.set_diarize_active(true);
                                ui.set_diarize_status("".into());
                            }
                            Err(e) => {
                                ui.set_diarize_status(format!("{}: {e}", t.runtime_error).into());
                            }
                        }
                    });
                });
            }
            #[cfg(not(feature = "diarize"))]
            {
                let _ = _weak.upgrade_in_event_loop(move |ui| {
                    ui.set_diarize_status(t.diarize_unavailable.into());
                });
            }
        });
    }

    // ---- change UI language ----
    {
        let weak = ui.as_weak();
        let dd = data_dir.clone();
        let cul = current_ui_lang.clone();
        let lm = lang_model.clone();
        let tray_show = _tray_state.as_ref().map(|ts| ts.show_item.clone());
        let tray_about = _tray_state.as_ref().map(|ts| ts.about_item.clone());
        ui.on_change_ui_lang(move |code| {
            let locale = Locale::from_code(code.as_str());
            let new_t = i18n::translations(locale);
            *cul.borrow_mut() = Some(code.to_string());
            if let Some(ui) = weak.upgrade() {
                apply_translations(&ui, new_t);
                ui.set_ui_lang(code.clone());
                // update auto-detect label in language combo
                lm.set_row_data(0, new_t.auto_detect.into());
                // update model notes
                let m = ui.get_models();
                for i in 0..m.row_count() {
                    if let Some(mut it) = m.row_data(i) {
                        it.note = match it.id.as_str() {
                            "lite" => new_t.model_lite_note,
                            "medium" => new_t.model_medium_note,
                            "turbo" => new_t.model_turbo_note,
                            _ => return,
                        }.into();
                        m.set_row_data(i, it);
                    }
                }
                // update tray menu items
                if let Some(ref item) = tray_show { item.set_text(new_t.tray_show); }
                if let Some(ref item) = tray_about { item.set_text(new_t.tray_about); }
                let mid = current_model_id(&ui);
                config::save_config(&mid, ui.get_diarize_enabled(), Some(code.as_str()), dd.as_deref());
            }
        });
    }

    // ---- start minimized ----
    let _startup_timer = {
        let t = Timer::default();
        let weak = ui.as_weak();
        t.start(TimerMode::SingleShot, Duration::from_millis(0), move || {
            if let Some(ui) = weak.upgrade() {
                use i_slint_backend_winit::WinitWindowAccessor;
                ui.window().with_winit_window(|w: &winit::window::Window| {
                    w.set_minimized(true);
                });
            }
        });
        t
    };

    ui.run()?;
    Ok(())
}

// ── Internal helpers ──────────────────────────────────

struct PendingResult {
    text: String,
    segments: Vec<ResultSegment>,
    audio_duration_secs: f64,
    model: String,
    language: String,
    diarized: bool,
    transcription_time_secs: f64,
    diarization_time_secs: f64,
}

impl PendingResult {
    fn into_scriba_result(self) -> ScribaResult {
        ScribaResult {
            text: self.text,
            segments: self.segments,
            audio_duration_secs: self.audio_duration_secs,
            model: self.model,
            language: self.language,
            diarized: self.diarized,
            transcription_time_secs: self.transcription_time_secs,
            diarization_time_secs: self.diarization_time_secs,
        }
    }
}

fn run_transcription(
    mid: &str,
    lang: &str,
    samples: &[f32],
    dur_secs: f64,
    do_diarize: bool,
    data_dir: Option<&std::path::Path>,
    pcb: Arc<dyn Fn(f32) + Send + Sync>,
    weak: slint::Weak<AppWindow>,
    t: &'static Translations,
) -> anyhow::Result<PendingResult> {
    let mp = models::model_path(mid, data_dir)
        .ok_or_else(|| anyhow::anyhow!("{}", t.unknown_model))?;

    if !mp.exists() {
        let w = weak.clone();
        let _ = w.upgrade_in_event_loop(|ui| {
            ui.set_state(AppState::Downloading);
            ui.set_timer_text("".into());
            ui.set_transcribe_progress(0.0);
        });
        let w2 = weak.clone();
        let mid_s = mid.to_string();
        models::download_model(&mid_s, data_dir, |dl, total| {
            let frac = total.map(|t| dl as f32 / t as f32).unwrap_or(0.0);
            let label = match total {
                Some(t) => format!("{} / {}", models::human_bytes(dl), models::human_bytes(t)),
                None => models::human_bytes(dl),
            };
            let w3 = w2.clone();
            let _ = w3.upgrade_in_event_loop(move |ui| {
                ui.set_transcribe_progress(frac);
                ui.set_timer_text(label.into());
            });
        })?;
        let w4 = weak.clone();
        let _ = w4.upgrade_in_event_loop(|ui| {
            ui.set_state(AppState::Transcribing);
            ui.set_transcribe_progress(0.0);
            ui.set_timer_text("".into());
        });
    }

    let t_start = Instant::now();
    let tr = whisper::transcribe(&mp, lang, samples, pcb)?;
    let transcription_time = t_start.elapsed().as_secs_f64();

    let mut diarization_time = 0.0;
    let (final_text, segments_with_speakers) = maybe_diarize(
        do_diarize, &tr, samples, &weak, &mut diarization_time, t,
    )?;

    Ok(PendingResult {
        text: final_text,
        segments: segments_with_speakers,
        audio_duration_secs: dur_secs,
        model: mid.to_string(),
        language: tr.detected_lang.clone(),
        diarized: do_diarize && diarization_time > 0.0,
        transcription_time_secs: transcription_time,
        diarization_time_secs: diarization_time,
    })
}

fn maybe_diarize(
    do_diarize: bool,
    tr: &whisper::TranscribeResult,
    _samples: &[f32],
    _weak: &slint::Weak<AppWindow>,
    _diarization_time: &mut f64,
    t: &'static Translations,
) -> anyhow::Result<(String, Vec<ResultSegment>)> {
    #[cfg(feature = "diarize")]
    if do_diarize && scriba_core::diarize::models_installed() {
        let w = _weak.clone();
        let _ = w.upgrade_in_event_loop(|ui| {
            ui.set_state(AppState::Diarizing);
            ui.set_transcribe_progress(0.0);
            ui.set_timer_text("".into());
        });
        let d_start = Instant::now();
        let w2 = _weak.clone();
        let dcb: Box<dyn Fn(i32, i32) -> i32 + Send + 'static> = Box::new(move |done, total| {
            let p = if total > 0 { done as f32 / total as f32 } else { 0.0 };
            let eta = eta_string(d_start, p);
            let w3 = w2.clone();
            let _ = w3.upgrade_in_event_loop(move |ui| {
                ui.set_transcribe_progress(p);
                ui.set_timer_text(eta.into());
            });
            0
        });

        let speaker_segs = scriba_core::diarize::diarize(_samples, Some(dcb))?;
        *_diarization_time = d_start.elapsed().as_secs_f64();

        let merged = scriba_core::diarize::merge_transcript_with_speakers(&tr.segments, &speaker_segs);

        let segments: Vec<ResultSegment> = tr.segments.iter().map(|(s, e, txt)| {
            let mid_t = (s + e) / 2.0;
            let speaker = speaker_segs.iter()
                .find(|ss| ss.start <= mid_t && mid_t <= ss.end)
                .map(|ss| format!("{} {}", t.speaker_label, ss.speaker + 1));
            ResultSegment {
                start: *s as f64,
                end: *e as f64,
                text: txt.trim().to_string(),
                speaker,
            }
        }).collect();

        return Ok((merged, segments));
    }

    let _ = do_diarize;
    let segments: Vec<ResultSegment> = tr.segments.iter().map(|(s, e, txt)| {
        ResultSegment {
            start: *s as f64,
            end: *e as f64,
            text: txt.trim().to_string(),
            speaker: None,
        }
    }).collect();
    Ok((tr.text.clone(), segments))
}

// ── BLE recorder flow ───────────────────────────────────

#[cfg(feature = "recorder")]
fn ble_recorder_flow(
    weak: slint::Weak<AppWindow>,
    data_dir: Option<std::path::PathBuf>,
    lang: String,
    model_id: String,
    do_diarize: bool,
    pending_result: Arc<Mutex<Option<PendingResult>>>,
    ble_file_ids: Arc<Mutex<Vec<u32>>>,
    t: &'static Translations,
) {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.set_transcript(format!("\u{26a0}\u{fe0e} {}: {e}", t.runtime_error).into());
                ui.set_state(AppState::Result);
            });
            return;
        }
    };

    let weak2 = weak.clone();
    let scan_result = rt.block_on(ble_download_files(weak.clone(), t));

    match scan_result {
        Ok((file_ids, opus_paths)) if !opus_paths.is_empty() => {
            // Store file IDs for potential deletion later (after transcription)
            *ble_file_ids.lock().unwrap() = file_ids;

            // Process each downloaded file through the transcription pipeline
            let w = weak.clone();
            let _ = w.upgrade_in_event_loop(|ui| {
                ui.set_state(AppState::Transcribing);
                ui.set_transcribe_progress(0.0);
                ui.set_timer_text("".into());
            });

            let dd = data_dir.as_deref();
            let mut all_text = String::new();
            let mut all_segments = Vec::new();
            let mut total_duration = 0.0;
            let mut total_transcription_time = 0.0;
            let mut total_diarization_time = 0.0;

            for (i, path) in opus_paths.iter().enumerate() {
                let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let w = weak.clone();
                let label = format!("{} ({}/{})", fname, i + 1, opus_paths.len());
                let _ = w.upgrade_in_event_loop(move |ui| {
                    ui.set_source_name(label.into());
                });

                let res = (|| -> anyhow::Result<()> {
                    let wav_path = path.with_extension("wav");
                    let ffmpeg_status = std::process::Command::new("ffmpeg")
                        .args(["-y", "-i"])
                        .arg(&path)
                        .args(["-ar", "16000", "-ac", "1"])
                        .arg(&wav_path)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                    let audio_path = match ffmpeg_status {
                        Ok(s) if s.success() => wav_path,
                        _ => path.clone(),
                    };
                    let decoded = scriba_core::audio::decode_audio_file(&audio_path)?;
                    let dur = decoded.samples.len() as f64 / decoded.sample_rate as f64;
                    total_duration += dur;
                    let pcm = scriba_core::audio::resample_to_16khz(&decoded.samples, decoded.sample_rate)?;

                    let t_start = Instant::now();
                    let weak3 = weak.clone();
                    let pcb: Arc<dyn Fn(f32) + Send + Sync> = Arc::new(move |p: f32| {
                        let eta = eta_string(t_start, p);
                        let w3 = weak3.clone();
                        let _ = w3.upgrade_in_event_loop(move |ui| {
                            ui.set_transcribe_progress(p);
                            ui.set_timer_text(eta.into());
                        });
                    });

                    let pr = run_transcription(
                        &model_id, &lang, &pcm, dur, do_diarize, dd, pcb, weak.clone(), t,
                    )?;
                    total_transcription_time += pr.transcription_time_secs;
                    total_diarization_time += pr.diarization_time_secs;
                    if !all_text.is_empty() { all_text.push_str("\n\n"); }
                    all_text.push_str(&pr.text);
                    all_segments.extend(pr.segments);
                    Ok(())
                })();

                if let Err(e) = res {
                    log::warn!("transcription error {}: {e}", path.display());
                }
            }

            let has_ble_files = !ble_file_ids.lock().unwrap().is_empty();
            let pr_val = PendingResult {
                text: all_text.clone(),
                segments: all_segments,
                audio_duration_secs: total_duration,
                model: model_id,
                language: lang,
                diarized: do_diarize && total_diarization_time > 0.0,
                transcription_time_secs: total_transcription_time,
                diarization_time_secs: total_diarization_time,
            };

            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.set_transcript(all_text.into());
                *pending_result.lock().unwrap() = Some(pr_val);
                ui.set_transcribe_progress(0.0);
                ui.set_timer_text("00:00".into());
                ui.set_show_delete_prompt(has_ble_files);
                ui.set_state(AppState::Result);
            });
        }
        Ok(_) => {
            let _ = weak2.upgrade_in_event_loop(|ui| {
                ui.set_transcript(t.ble_no_files.into());
                ui.set_state(AppState::Result);
            });
        }
        Err(e) => {
            let _ = weak2.upgrade_in_event_loop(move |ui| {
                ui.set_transcript(format!("\u{26a0}\u{fe0e} {e}").into());
                ui.set_state(AppState::Result);
            });
        }
    }
}

#[cfg(feature = "recorder")]
async fn ble_download_files(
    weak: slint::Weak<AppWindow>,
    t: &'static Translations,
) -> anyhow::Result<(Vec<u32>, Vec<std::path::PathBuf>)> {
    use std::time::Duration;

    let devices = mic_rs::Recorder::scan(None, Duration::from_secs(5)).await
        .map_err(|e| anyhow::anyhow!("{} {e}", t.ble_scanning))?;

    if devices.is_empty() {
        anyhow::bail!("{}", t.ble_not_found);
    }

    let dev = devices.into_iter().next().unwrap();
    let dev_label = format!("{} ({})", dev.name, dev.address);
    {
        let w = weak.clone();
        let dl = dev_label.clone();
        let _ = w.upgrade_in_event_loop(move |ui| {
            ui.set_download_label(format!("{} {dl}\u{2026}", t.ble_connecting).into());
        });
    }

    let mut recorder = mic_rs::Recorder::connect(dev).await
        .map_err(|e| anyhow::anyhow!("{} {dev_label}: {e}", t.ble_connecting))?;

    {
        let w = weak.clone();
        let _ = w.upgrade_in_event_loop(|ui| {
            ui.set_download_label(t.ble_handshake.into());
        });
    }

    recorder.handshake().await
        .map_err(|e| anyhow::anyhow!("{} {dev_label}: {e}", t.ble_handshake))?;

    {
        let w = weak.clone();
        let _ = w.upgrade_in_event_loop(|ui| {
            ui.set_download_label(t.ble_reading_files.into());
        });
    }

    let files = recorder.list_files().await
        .map_err(|e| anyhow::anyhow!("{} {e}", t.ble_reading_files))?;

    if files.is_empty() {
        let _ = recorder.disconnect().await;
        return Ok((vec![], vec![]));
    }

    let file_ids: Vec<u32> = files.iter().map(|f| f.file_id).collect();
    let output_dir = std::env::temp_dir().join("scriba_ble");
    let _ = tokio::fs::create_dir_all(&output_dir).await;
    let mut opus_paths = Vec::new();
    let total_files = files.len();

    for (i, file) in files.iter().enumerate() {
        {
            let w = weak.clone();
            let label = format!(
                "Download {}/{} ({:.0}s)\u{2026}",
                i + 1, total_files,
                file.duration_ms as f64 / 1000.0,
            );
            let _ = w.upgrade_in_event_loop(move |ui| {
                ui.set_download_label(label.into());
                ui.set_transcribe_progress(0.0);
            });
        }

        let w = weak.clone();
        let idx = i;
        let last_posted = std::sync::atomic::AtomicU32::new(0);
        let progress_cb = move |p: mic_rs::DownloadProgress| {
            let prev = last_posted.load(std::sync::atomic::Ordering::Relaxed);
            if p.bytes_received.saturating_sub(prev) < 500 && p.bytes_received < p.total_bytes {
                return;
            }
            last_posted.store(p.bytes_received, std::sync::atomic::Ordering::Relaxed);
            let frac = if p.total_bytes > 0 { p.bytes_received as f32 / p.total_bytes as f32 } else { 0.0 };
            let label = format!(
                "Download {}/{} \u{2014} {} / {}",
                idx + 1, total_files,
                models::human_bytes(p.bytes_received as u64),
                models::human_bytes(p.total_bytes as u64),
            );
            let w2 = w.clone();
            let _ = w2.upgrade_in_event_loop(move |ui| {
                ui.set_transcribe_progress(frac);
                ui.set_download_label(label.into());
            });
        };

        match recorder.download_file(file.file_id, &output_dir, progress_cb).await {
            Ok(path) => opus_paths.push(path),
            Err(e) => log::warn!("download file {}: {e}", file.file_id),
        }
    }

    let _ = recorder.disconnect().await;
    Ok((file_ids, opus_paths))
}

#[cfg(feature = "recorder")]
fn ble_delete_from_device(file_ids: Vec<u32>, weak: slint::Weak<AppWindow>, t: &'static Translations) {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            log::warn!("tokio runtime for delete: {e}");
            return;
        }
    };
    rt.block_on(async {
        let _ = weak.upgrade_in_event_loop(|ui| {
            ui.set_source_name(t.ble_connect_delete.into());
        });
        let devices = match mic_rs::Recorder::scan(None, std::time::Duration::from_secs(5)).await {
            Ok(d) if !d.is_empty() => d,
            Ok(_) => {
                log::warn!("device not found for delete");
                let _ = weak.upgrade_in_event_loop(|ui| {
                    ui.set_source_name(t.ble_device_not_found.into());
                });
                return;
            }
            Err(e) => {
                log::warn!("scan for delete: {e}");
                return;
            }
        };
        let dev = devices.into_iter().next().unwrap();
        let mut rec = match mic_rs::Recorder::connect(dev).await {
            Ok(r) => r,
            Err(e) => { log::warn!("connect for delete: {e}"); return; }
        };
        if let Err(e) = rec.handshake().await {
            log::warn!("handshake for delete: {e}");
            let _ = rec.disconnect().await;
            return;
        }
        for id in &file_ids {
            if let Err(e) = rec.delete_file(*id).await {
                log::warn!("delete file {id}: {e}");
            }
        }
        let _ = rec.disconnect().await;
        log::info!("deleted {} files from device", file_ids.len());
        let _ = weak.upgrade_in_event_loop(|ui| {
            ui.set_source_name(t.ble_files_deleted.into());
        });
    });
}

// ── System tray icon ──────────────────────────────────────

fn create_tray_icon_image(r: u8, g: u8, b: u8) -> tray_icon::Icon {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let center = size as f32 / 2.0;
    let radius = center - 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center + 0.5;
            let dy = y as f32 - center + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= radius {
                let idx = ((y * size + x) * 4) as usize;
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }
    }
    tray_icon::Icon::from_rgba(rgba, size, size).unwrap()
}

struct TrayState {
    _tray: tray_icon::TrayIcon,
    _timer: Timer,
    show_item: tray_icon::menu::MenuItem,
    about_item: tray_icon::menu::MenuItem,
}

fn setup_tray_icon(
    ui: &AppWindow,
    cfg: &crate::ScribaConfig,
    t: &'static Translations,
) -> Option<TrayState> {
    use tray_icon::menu::{Menu, MenuItem, MenuEvent, PredefinedMenuItem};

    let menu = Menu::new();
    let show_item = MenuItem::new(t.tray_show, true, None);
    let about_item = MenuItem::new(t.tray_about, true, None);
    if menu.append(&show_item).is_err() { return None; }
    if menu.append(&PredefinedMenuItem::separator()).is_err() { return None; }
    if menu.append(&about_item).is_err() { return None; }

    let show_id = show_item.id().clone();
    let about_id = about_item.id().clone();

    let (r, g, b) = cfg.accent_color;
    let icon = create_tray_icon_image(r, g, b);
    let tray = match tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Scriba")
        .with_icon(icon)
        .build()
    {
        Ok(t) => t,
        Err(e) => {
            log::warn!("tray icon creation failed: {e}");
            return None;
        }
    };

    let tray_timer = Timer::default();
    {
        let weak = ui.as_weak();
        tray_timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id == show_id {
                    if let Some(ui) = weak.upgrade() {
                        let _ = ui.window().show();
                        use i_slint_backend_winit::WinitWindowAccessor;
                        ui.window().with_winit_window(|w: &winit::window::Window| {
                            w.set_minimized(false);
                            w.focus_window();
                        });
                    }
                } else if event.id == about_id {
                    rfd::MessageDialog::new()
                        .set_title(t.about_title)
                        .set_description(
                            "Scriba v20260606\n\n\
                             \u{00a9} 2026 Dario Finardi\n\n\
                             Licensed under AGPL-3.0-or-later\n\
                             (GNU Affero General Public License)"
                        )
                        .set_level(rfd::MessageLevel::Info)
                        .show();
                }
            }
        });
    }

    Some(TrayState { _tray: tray, _timer: tray_timer, show_item, about_item })
}
