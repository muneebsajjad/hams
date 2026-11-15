//! Thin wrapper around whisper.cpp. Everything runs on this machine; nothing leaves it.

use std::sync::Mutex;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

use crate::audio::SAMPLE_RATE;
use crate::core::text;

struct Loaded {
    path: String,
    // Declared before the context so it is dropped first.
    state: WhisperState,
    _context: WhisperContext,
}

pub struct Transcriber {
    loaded: Mutex<Option<Loaded>>,
}

impl Transcriber {
    pub fn new(verbose: bool) -> Self {
        if !verbose {
            // Without a logging backend enabled this silences whisper.cpp's chatter.
            whisper_rs::install_logging_hooks();
        }
        Transcriber { loaded: Mutex::new(None) }
    }

    pub fn load(&self, path: &str) -> Result<(), String> {
        let mut loaded = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
        if loaded.as_ref().is_some_and(|l| l.path == path) {
            return Ok(());
        }
        *loaded = None; // free the previous model before loading the next
        let mut params = WhisperContextParameters::default();
        // Metal only on Apple Silicon: on Intel Macs with AMD/Intel GPUs whisper.cpp's Metal
        // backend produces garbage transcripts (and the CPU is faster there anyway).
        params.use_gpu(cfg!(all(target_os = "macos", target_arch = "aarch64")));
        let context = WhisperContext::new_with_params(path, params)
            .map_err(|e| format!("Could not load model at {path} ({e})"))?;
        let state = context.create_state().map_err(|e| format!("Could not load model at {path} ({e})"))?;
        *loaded = Some(Loaded { path: path.to_owned(), state, _context: context });
        Ok(())
    }

    /// `samples` must be 16 kHz mono. `language` is an ISO code or "auto".
    pub fn transcribe(&self, samples: &[f32], language: &str, prompt: &str) -> Result<String, String> {
        let mut loaded = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
        let loaded = loaded.as_mut().ok_or("No speech model loaded")?;

        // whisper rejects very short clips; pad with silence.
        let mut samples = samples.to_vec();
        let min_len = (SAMPLE_RATE as f32 * 1.1) as usize;
        if samples.len() < min_len {
            samples.resize(min_len, 0.0);
        }

        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) as i32;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads((cores - 2).clamp(2, 8));
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_print_special(false);
        params.set_translate(false);
        params.set_no_context(true);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        // Timestamps are only needed to stitch windows on clips longer than one 30 s window.
        params.set_no_timestamps(samples.len() < SAMPLE_RATE as usize * 28);
        params.set_language(Some(language));
        params.set_detect_language(false);
        if !prompt.is_empty() {
            params.set_initial_prompt(prompt);
        }

        loaded.state.full(params, &samples).map_err(|e| format!("Transcription failed ({e})"))?;

        let mut out = String::new();
        for segment in loaded.state.as_iter() {
            if let Ok(piece) = segment.to_str_lossy() {
                out.push_str(&piece);
            }
        }
        Ok(text::clean(&out))
    }
}
