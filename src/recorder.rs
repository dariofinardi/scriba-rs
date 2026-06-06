use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct Recorder {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

impl Recorder {
    pub fn start(device_name: &str) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host.input_devices()?
            .find(|d| d.name().map(|n| n == device_name).unwrap_or(false))
            .or_else(|| host.default_input_device())
            .ok_or_else(|| anyhow::anyhow!("no input device"))?;

        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
        let sink = buffer.clone();
        let err_fn = |e| eprintln!("audio error: {e}");

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| sink.lock().unwrap().extend_from_slice(data),
                err_fn, None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| {
                    sink.lock().unwrap().extend(data.iter().map(|s| *s as f32 / 32768.0));
                },
                err_fn, None,
            )?,
            fmt => anyhow::bail!("unsupported: {fmt:?}"),
        };
        stream.play()?;
        Ok(Self { _stream: stream, buffer, sample_rate, channels })
    }

    pub fn finish(self) -> Vec<f32> {
        drop(self._stream);
        let raw = self.buffer.lock().unwrap().clone();
        let mono = downmix(&raw, self.channels);
        resample_linear(&mono, self.sample_rate, 16_000)
    }
}

pub fn input_device_names() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devs) => devs.filter_map(|d| d.name().ok()).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn default_input_name() -> Option<String> {
    cpal::default_host().default_input_device().and_then(|d| d.name().ok())
}

fn downmix(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 { return samples.to_vec(); }
    let ch = channels as usize;
    samples.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect()
}

fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() { return input.to_vec(); }
    let ratio = to as f64 / from as f64;
    let out_len = (input.len() as f64 * ratio).round() as usize;
    (0..out_len).map(|i| {
        let src = i as f64 / ratio;
        let idx = src.floor() as usize;
        let frac = (src - idx as f64) as f32;
        let a = input[idx.min(input.len() - 1)];
        let b = input[(idx + 1).min(input.len() - 1)];
        a + (b - a) * frac
    }).collect()
}
