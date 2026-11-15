//! Microphone capture to 16 kHz mono, plus WAV reading and writing.

use std::f32::consts::PI;
use std::fs;
use std::path::Path;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};

/// The rate whisper expects.
pub const SAMPLE_RATE: u32 = 16_000;

/// Maps RMS amplitude onto 0...1 for metering.
pub fn meter(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    let rms = (sum / samples.len() as f32).sqrt();
    let db = 20.0 * rms.max(1e-7).log10();
    ((db + 55.0) / 45.0).clamp(0.0, 1.0)
}

/// Band-limited resampling to 16 kHz (Hann-windowed sinc), so downsampling a 44.1 or
/// 48 kHz microphone does not fold high frequencies back into the speech band.
pub fn resample(input: &[f32], from_rate: u32) -> Vec<f32> {
    if from_rate == SAMPLE_RATE || input.is_empty() || from_rate == 0 {
        return input.to_vec();
    }
    let step = from_rate as f64 / SAMPLE_RATE as f64; // input samples per output sample
    let cutoff = (1.0 / step).min(1.0) as f32; // relative to the input Nyquist
    let half_width = (16.0 / cutoff).ceil() as isize; // 16 zero crossings each side
    let out_len = (input.len() as f64 / step).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for n in 0..out_len {
        let centre = n as f64 * step;
        let base = centre.floor() as isize;
        let mut acc = 0.0f32;
        for i in (base - half_width)..=(base + half_width) {
            if i < 0 || i as usize >= input.len() {
                continue;
            }
            let x = (i as f64 - centre) as f32;
            let sinc = if x.abs() < 1e-6 { 1.0 } else { (PI * cutoff * x).sin() / (PI * cutoff * x) };
            let window = 0.5 + 0.5 * (PI * x / half_width as f32).cos();
            acc += input[i as usize] * cutoff * sinc * window;
        }
        out.push(acc);
    }
    out
}

/// Writes 16 kHz mono samples as a 16-bit PCM WAV.
pub fn write_wav(samples: &[f32], path: &Path) -> std::io::Result<()> {
    let bytes = (samples.len() * 2) as u32;
    let mut data = Vec::with_capacity(44 + samples.len() * 2);
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + bytes).to_le_bytes());
    data.extend_from_slice(b"WAVEfmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes()); // PCM
    data.extend_from_slice(&1u16.to_le_bytes()); // mono
    data.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    data.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(b"data");
    data.extend_from_slice(&bytes.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        data.extend_from_slice(&v.to_le_bytes());
    }
    fs::write(path, data)
}

/// Reads a PCM or float WAV file as 16 kHz mono samples.
pub fn read_wav(path: &Path) -> Result<Vec<f32>, String> {
    let data = fs::read(path).map_err(|e| format!("Could not read audio: {e}"))?;
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("Could not read audio: not a WAV file".into());
    }
    let u16_at = |i: usize| u16::from_le_bytes([data[i], data[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);

    let mut format = None; // (tag, channels, rate, bits)
    let mut pcm: Option<&[u8]> = None;
    let mut at = 12;
    while at + 8 <= data.len() {
        let id = &data[at..at + 4];
        let size = u32_at(at + 4) as usize;
        let body = at + 8;
        let end = (body + size).min(data.len());
        if id == b"fmt " && end - body >= 16 {
            let mut tag = u16_at(body);
            if tag == 0xFFFE && end - body >= 26 {
                tag = u16_at(body + 24); // WAVE_FORMAT_EXTENSIBLE: real tag is in the sub-format
            }
            format = Some((tag, u16_at(body + 2) as usize, u32_at(body + 4), u16_at(body + 14)));
        } else if id == b"data" {
            pcm = Some(&data[body..end]);
        }
        at = body + size + (size & 1);
    }
    let (tag, channels, rate, bits) = format.ok_or("Could not read audio: no format chunk")?;
    let pcm = pcm.ok_or("Could not read audio: no data chunk")?;
    if channels == 0 {
        return Err("Could not read audio: no channels".into());
    }
    let width = (bits / 8) as usize;
    let decode: fn(&[u8]) -> f32 = match (tag, bits) {
        (1, 8) => |b| (b[0] as f32 - 128.0) / 128.0,
        (1, 16) => |b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0,
        (1, 24) => |b| (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
        (1, 32) => |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
        (3, 32) => |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        _ => return Err(format!("Could not read audio: unsupported WAV encoding ({tag}, {bits}-bit)")),
    };
    let mono: Vec<f32> = pcm
        .chunks_exact(width * channels)
        .map(|frame| frame.chunks_exact(width).map(decode).sum::<f32>() / channels as f32)
        .collect();
    Ok(resample(&mono, rate))
}

// MARK: - Recorder

enum Command {
    Start(Sender<Result<(), String>>),
    Stop(Sender<Vec<f32>>),
}

type LevelFn = Arc<dyn Fn(f32) + Send + Sync>;

/// Captures the default microphone. The audio stream lives on its own thread because
/// stream handles cannot move between threads on every platform.
pub struct Recorder {
    commands: Sender<Command>,
}

struct Capture {
    _stream: Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    rate: u32,
}

impl Recorder {
    pub fn new(on_level: impl Fn(f32) + Send + Sync + 'static) -> Self {
        let (commands, inbox) = channel::<Command>();
        let on_level: LevelFn = Arc::new(on_level);
        std::thread::Builder::new()
            .name("hams-audio".into())
            .spawn(move || {
                let mut capture: Option<Capture> = None;
                for command in inbox {
                    match command {
                        Command::Start(reply) => {
                            capture = None;
                            let result = open(on_level.clone());
                            let _ = reply.send(match result {
                                Ok(c) => {
                                    capture = Some(c);
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            });
                        }
                        Command::Stop(reply) => {
                            let samples = match capture.take() {
                                Some(c) => {
                                    let Capture { _stream, samples, rate } = c;
                                    drop(_stream);
                                    let raw = std::mem::take(&mut *samples.lock().unwrap_or_else(|e| e.into_inner()));
                                    resample(&raw, rate)
                                }
                                None => Vec::new(),
                            };
                            let _ = reply.send(samples);
                        }
                    }
                }
            })
            .expect("spawn audio thread");
        Recorder { commands }
    }

    pub fn start(&self) -> Result<(), String> {
        let (reply, result) = channel();
        self.commands.send(Command::Start(reply)).map_err(|_| "Audio thread stopped".to_owned())?;
        result.recv().map_err(|_| "Audio thread stopped".to_owned())?
    }

    /// Stops capturing and returns everything recorded, at 16 kHz mono.
    pub fn stop(&self) -> Vec<f32> {
        let (reply, result) = channel();
        if self.commands.send(Command::Stop(reply)).is_err() {
            return Vec::new();
        }
        result.recv().unwrap_or_default()
    }
}

fn open(on_level: LevelFn) -> Result<Capture, String> {
    let device = cpal::default_host().default_input_device().ok_or("No microphone available")?;
    let supported = device.default_input_config().map_err(|e| format!("No microphone available ({e})"))?;
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let rate = config.sample_rate;
    let samples = Arc::new(Mutex::new(Vec::<f32>::with_capacity(rate as usize * 30)));

    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, samples.clone(), on_level),
        SampleFormat::I16 => build::<i16>(&device, config, samples.clone(), on_level),
        SampleFormat::U16 => build::<u16>(&device, config, samples.clone(), on_level),
        SampleFormat::I32 => build::<i32>(&device, config, samples.clone(), on_level),
        SampleFormat::I8 => build::<i8>(&device, config, samples.clone(), on_level),
        SampleFormat::U8 => build::<u8>(&device, config, samples.clone(), on_level),
        SampleFormat::F64 => build::<f64>(&device, config, samples.clone(), on_level),
        other => Err(format!("Unsupported microphone format: {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Capture { _stream: stream, samples, rate })
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    on_level: LevelFn,
) -> Result<Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    let mut mono = Vec::<f32>::new();
    let mut pending = Vec::<f32>::new();
    let mut last_level = Instant::now();
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                mono.clear();
                mono.extend(
                    data.chunks_exact(channels)
                        .map(|frame| frame.iter().map(|s| f32::from_sample_(*s)).sum::<f32>() / channels as f32),
                );
                samples.lock().unwrap_or_else(|e| e.into_inner()).extend_from_slice(&mono);
                // Meter about 25 times a second regardless of the device's buffer size.
                pending.extend_from_slice(&mono);
                if last_level.elapsed() >= Duration::from_millis(40) {
                    on_level(meter(&pending));
                    pending.clear();
                    last_level = Instant::now();
                }
            },
            |err| eprintln!("hams: audio stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, rate: u32, seconds: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize).map(|i| (2.0 * PI * freq * i as f32 / rate as f32).sin() * 0.5).collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn resample_keeps_speech_band_and_length() {
        let out = resample(&tone(1000.0, 48_000, 1.0), 48_000);
        assert_eq!(out.len(), 16_000);
        let level = rms(&out[1000..15_000]);
        assert!((level - 0.3536).abs() < 0.01, "1 kHz tone should pass unchanged, rms {level}");
    }

    #[test]
    fn resample_removes_what_would_alias() {
        // 12 kHz is above the 8 kHz Nyquist of the output and must not fold back in.
        let out = resample(&tone(12_000.0, 48_000, 1.0), 48_000);
        let level = rms(&out[1000..15_000]);
        assert!(level < 0.01, "12 kHz tone should be filtered out, rms {level}");
    }

    #[test]
    fn resample_handles_44100_and_passthrough() {
        assert_eq!(resample(&tone(440.0, 44_100, 1.0), 44_100).len(), 16_000);
        let same = tone(440.0, 16_000, 0.1);
        assert_eq!(resample(&same, 16_000), same);
    }

    #[test]
    fn wav_round_trip() {
        let path = std::env::temp_dir().join(format!("hams-test-{}.wav", std::process::id()));
        let original = tone(440.0, 16_000, 0.25);
        write_wav(&original, &path).unwrap();
        let back = read_wav(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(back.len(), original.len());
        let worst = original.iter().zip(&back).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
        assert!(worst < 1e-3, "16-bit round trip error {worst}");
    }

    #[test]
    fn meter_range() {
        assert_eq!(meter(&[]), 0.0);
        assert_eq!(meter(&[0.0; 64]), 0.0);
        assert_eq!(meter(&[1.0; 64]), 1.0);
    }
}
