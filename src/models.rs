use std::path::{Path, PathBuf};

pub struct ModelDef {
    pub id: &'static str,
    pub name: &'static str,
    pub size: &'static str,
    pub note: &'static str,
    pub speed: i32,
    pub quality: i32,
    pub file: &'static str,
    pub url: &'static str,
}

pub fn registry() -> Vec<ModelDef> {
    vec![
        ModelDef {
            id: "lite", name: "Lite", size: "190 MB",
            note: "Veloce \u{00b7} ottimo per appunti rapidi",
            speed: 4, quality: 2,
            file: "ggml-small-q5_1.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin",
        },
        ModelDef {
            id: "medium", name: "Medium", size: "515 MB",
            note: "Bilanciato tra velocit\u{00e0} e accuratezza",
            speed: 3, quality: 3,
            file: "ggml-medium-q5_0.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium-q5_0.bin",
        },
        ModelDef {
            id: "turbo", name: "Large Turbo", size: "574 MB",
            note: "Massima accuratezza, ottimizzato",
            speed: 3, quality: 4,
            file: "ggml-large-v3-turbo-q5_0.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        },
    ]
}

pub fn model_dir(data_dir: Option<&Path>) -> PathBuf {
    if let Some(d) = data_dir {
        return d.join("models");
    }
    let local = PathBuf::from("models");
    if local.exists() { return local; }
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("scriba");
    dir.push("models");
    dir
}

pub fn model_path(id: &str, data_dir: Option<&Path>) -> Option<PathBuf> {
    registry().into_iter().find(|m| m.id == id).map(|m| model_dir(data_dir).join(m.file))
}

pub fn is_installed(id: &str, data_dir: Option<&Path>) -> bool {
    model_path(id, data_dir).map(|p| p.exists()).unwrap_or(false)
}

pub fn human_bytes(n: u64) -> String {
    const GB: f64 = 1_073_741_824.0;
    const MB: f64 = 1_048_576.0;
    const KB: f64 = 1_024.0;
    let n = n as f64;
    if n >= GB { format!("{:.1} GB", n / GB).replace('.', ",") }
    else if n >= MB { format!("{:.1} MB", n / MB).replace('.', ",") }
    else if n >= KB { format!("{:.0} KB", n / KB) }
    else { format!("{:.0} B", n) }
}

pub fn download_model<F: FnMut(u64, Option<u64>)>(id: &str, data_dir: Option<&Path>, mut progress: F) -> anyhow::Result<()> {
    use std::io::{Read, Write};
    let m = registry().into_iter().find(|m| m.id == id)
        .ok_or_else(|| anyhow::anyhow!("unknown model {id}"))?;

    let dir = model_dir(data_dir);
    std::fs::create_dir_all(&dir)?;

    let mut resp = reqwest::blocking::get(m.url)?.error_for_status()?;
    let total = resp.content_length();
    let tmp = dir.join(format!("{}.part", m.file));
    let mut file = std::fs::File::create(&tmp)?;
    let mut buf = [0u8; 65536];
    let mut downloaded = 0u64;

    loop {
        let n = resp.read(&mut buf)?;
        if n == 0 { break; }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
        progress(downloaded, total);
    }
    file.flush()?;
    std::fs::rename(tmp, dir.join(m.file))?;
    Ok(())
}
