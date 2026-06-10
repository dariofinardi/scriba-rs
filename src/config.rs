use std::path::{Path, PathBuf};

fn config_path(data_dir: Option<&Path>) -> PathBuf {
    match data_dir {
        Some(d) => d.join("config.json"),
        None => {
            let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
            dir.push("scriba");
            dir.join("config.json")
        }
    }
}

pub struct SavedConfig {
    pub model: String,
    pub diarize: bool,
    pub ui_language: Option<String>,
}

pub fn load_config(data_dir: Option<&Path>) -> SavedConfig {
    let path = config_path(data_dir);
    if let Ok(data) = std::fs::read_to_string(&path) {
        let model = data.lines()
            .find(|l| l.contains("\"model\""))
            .and_then(|l| l.split('"').nth(3))
            .unwrap_or("lite")
            .to_string();
        let diarize = data.contains("\"diarize\":true") || data.contains("\"diarize\": true");
        let ui_language = data.lines()
            .find(|l| l.contains("\"ui_language\""))
            .and_then(|l| l.split('"').nth(3))
            .map(|s| s.to_string());
        SavedConfig { model, diarize, ui_language }
    } else {
        SavedConfig { model: "lite".to_string(), diarize: false, ui_language: None }
    }
}

pub fn save_config(model: &str, diarize: bool, ui_language: Option<&str>, data_dir: Option<&Path>) {
    let path = config_path(data_dir);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let lang_part = match ui_language {
        Some(l) => format!(",\"ui_language\":\"{l}\""),
        None => String::new(),
    };
    let json = format!("{{\"model\":\"{model}\",\"diarize\":{diarize}{lang_part}}}");
    let _ = std::fs::write(path, json);
}
