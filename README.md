# Scriba

**Trascrizione vocale locale, privata e multilingue per desktop.**

![License](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)
![Rust](https://img.shields.io/badge/language-Rust-orange)

## Perché Scriba

I servizi di trascrizione cloud sollevano problemi di privacy, dipendenza dalla rete e costi ricorrenti. Scriba nasce per offrire un'alternativa completamente locale: la voce non lascia mai il dispositivo, la trascrizione avviene interamente on-device grazie a [Whisper](https://github.com/openai/whisper) di OpenAI, eseguito nativamente via [whisper.cpp](https://github.com/ggerganov/whisper.cpp).

L'obiettivo è fornire uno strumento semplice e integrabile: un widget di trascrizione che qualsiasi applicazione Rust può incorporare con una singola chiamata a funzione. Scriba gestisce autonomamente l'interfaccia, il download dei modelli, la registrazione dal microfono, e restituisce al chiamante un risultato strutturato in JSON.

## Cosa fa

- **Trascrizione da file audio** — importa WAV, MP3, OGG, FLAC tramite file picker
- **Registrazione dal microfono** — seleziona il dispositivo di input, cattura in tempo reale con timer e barra di progresso
- **Rilevamento automatico della lingua** — oppure selezione manuale tra italiano, inglese, spagnolo, francese, tedesco, portoghese
- **Identificazione degli speaker** — diarizzazione opzionale: chi sta parlando in ogni segmento (via [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx))
- **Gestione modelli integrata** — scarica, seleziona e cambia modello direttamente dal pannello di setup, senza uscire dall'app
- **Copia negli appunti** — un click per copiare la trascrizione
- **Risultato strutturato** — `ScribaResult` con testo completo, segmenti con timing, lingua, speaker, tempi di elaborazione — serializzabile JSON
- **Supporto registratori BLE** — download e trascrizione automatica da dispositivi Bluetooth (feature opzionale `recorder`, richiede [mic-rs](https://github.com/dariofinardi/mic-rs))

### Interfaccia

Finestra **borderless** e **always-on-top**, pensata per restare sovrapposta all'applicazione di lavoro senza ingombrare. Title bar custom con drag, minimizza e chiusura con conferma. Icona nella system tray con menu "Show Scriba" e "About…". Tema chiaro/scuro con colore accent personalizzabile.

L'applicazione parte minimizzata nella taskbar. La chiusura richiede sempre conferma per evitare perdite accidentali.

## Modelli Whisper

Scriba utilizza modelli GGML quantizzati, scaricati automaticamente al primo utilizzo da [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp/tree/main):

| ID | Modello | File | Dimensione | Velocità | Qualità | Uso consigliato |
|----|---------|------|-----------|----------|---------|-----------------|
| `lite` | Small Q5 | `ggml-small-q5_1.bin` | 190 MB | ★★★★ | ★★ | Appunti rapidi, bozze, meeting informali |
| `medium` | Medium Q5 | `ggml-medium-q5_0.bin` | 515 MB | ★★★ | ★★★ | Buon compromesso velocità/accuratezza |
| `turbo` | Large V3 Turbo Q5 | `ggml-large-v3-turbo-q5_0.bin` | 574 MB | ★★★ | ★★★★ | Massima accuratezza, trascrizioni professionali |

Tutti i modelli supportano oltre 90 lingue. La quantizzazione Q5 riduce l'occupazione di memoria mantenendo una qualità prossima ai modelli float16 originali.

I modelli vengono salvati in:
- `models/` nella directory di lavoro (se presente)
- Altrimenti in `%APPDATA%/scriba/models` (Windows) / `~/Library/Application Support/scriba/models` (macOS) / `~/.local/share/scriba/models` (Linux)

## Quick start

Aggiungi al tuo `Cargo.toml`:

```toml
[dependencies]
scriba-rs = { git = "https://github.com/dariofinardi/scriba-rs.git", tag = "v20260606" }
```

Lancia la finestra di trascrizione:

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

`run()` blocca fino alla chiusura della finestra. Il callback viene invocato quando l'utente preme "Conferma" sul risultato.

## API

### `ScribaConfig`

| Campo | Tipo | Default | Descrizione |
|-------|------|---------|-------------|
| `accent_color` | `(u8, u8, u8)` | `(232, 89, 12)` | Colore accent RGB per il tema UI |
| `dark_mode` | `Option<bool>` | `None` | `None` = segue il sistema operativo |
| `default_language` | `String` | `"auto"` | Codice lingua o `"auto"` per rilevamento |
| `model` | `String` | `"lite"` | ID modello: `"lite"`, `"medium"`, `"turbo"` |
| `diarize` | `bool` | `false` | Abilita identificazione speaker |
| `app_name` | `String` | `"Scriba"` | Nome mostrato nella title bar |
| `data_dir` | `Option<PathBuf>` | `None` | Directory custom per modelli e config |
| `always_on_top` | `bool` | `true` | Finestra sempre in primo piano |

### `ScribaResult`

Restituito al callback `on_result`, serializzabile JSON via serde.

| Campo | Tipo | Descrizione |
|-------|------|-------------|
| `text` | `String` | Testo completo della trascrizione |
| `segments` | `Vec<ResultSegment>` | Segmenti con timing e speaker opzionale |
| `audio_duration_secs` | `f64` | Durata audio sorgente in secondi |
| `model` | `String` | Modello utilizzato |
| `language` | `String` | Lingua rilevata o selezionata |
| `diarized` | `bool` | Se la diarizzazione è stata applicata |
| `transcription_time_secs` | `f64` | Tempo di inferenza Whisper |
| `diarization_time_secs` | `f64` | Tempo di diarizzazione (0.0 se disabilitata) |

### Entry point

```rust
pub fn run<F>(config: ScribaConfig, on_result: F) -> anyhow::Result<()>

pub fn run_with_cancel<F, G>(
    config: ScribaConfig,
    on_result: F,
    on_cancel: Option<G>,
) -> anyhow::Result<()>
```

Entrambe bloccano fino alla chiusura. Devono essere chiamate dal main thread (requisito macOS).

## Feature flag

| Flag | Descrizione |
|------|-------------|
| `diarize` | Identificazione speaker via sherpa-rs + ONNX Runtime |
| `recorder` | Supporto registratore BLE via mic-rs |
| `cuda` | Accelerazione GPU NVIDIA (CUDA) |
| `metal` | Accelerazione GPU Apple (Metal) |
| `vulkan` | Accelerazione GPU cross-platform (Vulkan) |

## Build

### Build standard

```sh
cargo build --release
```

### Con tutte le feature opzionali

```sh
cargo build --release --features "recorder,diarize"
```

### Windows ARM64 (Qualcomm Snapdragon)

Richiede Ninja e clang-cl. Consultare le [istruzioni di build di scriba-core](https://github.com/dariofinardi/scriba-core-rs#windows-arm64-qualcomm-snapdragon) per la configurazione dell'ambiente.

**Nota:** con la feature `diarize` abilitata, compilare sempre in release (`--release`). ONNX Runtime va in crash in modalità debug su ARM64.

### Eseguire l'esempio

```sh
cargo run --release --example basic
```

## Architettura

```
scriba-rs
├── src/
│   ├── lib.rs        # API pubblica (ScribaConfig, ScribaResult, run)
│   ├── gui.rs        # Orchestrazione Slint, callback, system tray
│   ├── recorder.rs   # Cattura microfono (cpal)
│   ├── models.rs     # Registro modelli, download, gestione path
│   ├── whisper.rs    # Trascrizione Whisper con progresso
│   └── config.rs     # Persistenza configurazione JSON
├── slint-ui/
│   ├── app.slint     # Layout finestra principale
│   ├── theme.slint   # Colori, icone, enumerazioni
│   ├── widgets.slint # Componenti UI riutilizzabili
│   └── setup.slint   # Pannello configurazione modelli
└── examples/
    └── basic.rs      # Esempio minimo di integrazione
```

### Dipendenze principali

| Crate | Ruolo |
|-------|-------|
| [scriba-core](https://github.com/dariofinardi/scriba-core-rs) | Audio decoding, resampling, inferenza Whisper, diarizzazione |
| [whisper-rs](https://github.com/dariofinardi/whisper-rs) | Binding Rust per whisper.cpp (fork con fix Windows) |
| [slint](https://slint.dev) | Framework UI nativo, backend winit |
| [cpal](https://crates.io/crates/cpal) | Cattura audio cross-platform |
| [tray-icon](https://crates.io/crates/tray-icon) | Icona system tray |
| [rfd](https://crates.io/crates/rfd) | Dialog nativi (file picker, conferme) |
| [sherpa-rs](https://crates.io/crates/sherpa-rs) | Diarizzazione speaker (opzionale) |
| [mic-rs](https://github.com/dariofinardi/mic-rs) | Registratore BLE Soundcore (opzionale) |

## Piattaforme supportate

- Windows x86_64
- Windows ARM64 (Qualcomm Snapdragon X Elite)
- macOS (Apple Silicon / Intel)
- Linux x86_64

## Licenza

AGPL-3.0-or-later — vedi [LICENSE](LICENSE).

Copyright © 2026 Dario Finardi
