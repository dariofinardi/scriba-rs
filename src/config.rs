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

pub fn load_config(data_dir: Option<&Path>) -> (String, bool) {
    let path = config_path(data_dir);
    if let Ok(data) = std::fs::read_to_string(&path) {
        let model = data.lines()
            .find(|l| l.contains("\"model\""))
            .and_then(|l| l.split('"').nth(3))
            .unwrap_or("lite")
            .to_string();
        let diarize = data.contains("\"diarize\":true") || data.contains("\"diarize\": true");
        (model, diarize)
    } else {
        ("lite".to_string(), false)
    }
}

pub fn save_config(model: &str, diarize: bool, data_dir: Option<&Path>) {
    let path = config_path(data_dir);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = format!("{{\"model\":\"{model}\",\"diarize\":{diarize}}}");
    let _ = std::fs::write(path, json);
}
