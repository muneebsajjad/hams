//! Application state and the dictation flow: record → transcribe → deliver.

use std::collections::BTreeMap;
use std::fs;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use hams_platform::{self as platform, HotkeyEvent, MicStatus, Sound};
use serde::Serialize;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Wry};

use crate::audio::{self, Recorder, SAMPLE_RATE};
use crate::core::{self, disk, paths, text, DictWord, Entry, ModelInfo, PolishModel, Settings, Snippet, MODELS, POLISH_MODELS};
use crate::download;
use crate::polish::Polisher;
use crate::transcriber::Transcriber;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Idle,
    Recording,
    Transcribing,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum ModelStatus {
    Missing,
    Loading,
    Ready,
    Failed(String),
}

struct Inner {
    entries: Vec<Entry>,
    dictionary: Vec<DictWord>,
    snippets: Vec<Snippet>,
    scratchpad: String,
    settings: Settings,

    phase: Phase,
    notice: Option<String>,
    notice_token: u64,
    model_status: ModelStatus,
    polish_status: ModelStatus,
    input_trusted: bool,
    mic: MicStatus,
    downloads: BTreeMap<String, f64>,
    target_app: Option<String>,
}

pub struct App {
    handle: AppHandle,
    inner: Mutex<Inner>,
    recorder: Recorder,
    transcriber: Transcriber,
    polisher: Polisher,
    /// Held while the microphone is being opened, so two starts never overlap.
    mic_gate: Mutex<()>,
    tray_toggle: Mutex<Option<MenuItem<Wry>>>,
}

// MARK: - What the window sees

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelView {
    name: &'static str,
    title: &'static str,
    detail: &'static str,
    size_mb: u32,
    downloaded: bool,
    english_only: bool,
}

fn status_name(status: &ModelStatus) -> (&'static str, Option<String>) {
    match status {
        ModelStatus::Missing => ("missing", None),
        ModelStatus::Loading => ("loading", None),
        ModelStatus::Ready => ("ready", None),
        ModelStatus::Failed(why) => ("failed", Some(why.clone())),
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    phase: Phase,
    notice: Option<String>,
    /// "missing" | "loading" | "ready" | "failed"
    model_status: &'static str,
    model_error: Option<String>,
    /// Same values as `model_status`, for the cleanup model. "missing" also covers "switched off".
    polish_status: &'static str,
    polish_error: Option<String>,
    polish_models: Vec<ModelView>,
    input_trusted: bool,
    mic_granted: bool,
    downloads: BTreeMap<String, f64>,
    settings: Settings,
    models: Vec<ModelView>,
}

#[derive(Serialize, Clone)]
pub struct HotkeyView {
    id: &'static str,
    label: &'static str,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    status: Status,
    entries: Vec<Entry>,
    dictionary: Vec<DictWord>,
    snippets: Vec<Snippet>,
    scratchpad: String,
    hotkeys: Vec<HotkeyView>,
    platform: &'static str,
    first_name: Option<String>,
    input_permission: &'static str,
    input_help: &'static str,
}

impl App {
    pub fn new(handle: AppHandle) -> Arc<App> {
        let mut settings: Settings = disk::load("settings.json").unwrap_or_default();
        if !platform::hotkey_options().iter().any(|k| k.id == settings.hotkey) {
            settings.hotkey = platform::hotkey_options()[0].id.to_owned();
        }
        if !MODELS.iter().any(|m| m.name == settings.model) {
            settings.model = MODELS.iter().find(|m| m.is_downloaded()).unwrap_or(&MODELS[0]).name.to_owned();
        }

        let level_handle = handle.clone();
        Arc::new(App {
            recorder: Recorder::new(move |level| {
                let _ = level_handle.emit("level", level);
            }),
            transcriber: Transcriber::new(false),
            polisher: Polisher::new(),
            mic_gate: Mutex::new(()),
            tray_toggle: Mutex::new(None),
            inner: Mutex::new(Inner {
                entries: disk::load("history.json").unwrap_or_default(),
                dictionary: disk::load("dictionary.json").unwrap_or_default(),
                snippets: disk::load("snippets.json").unwrap_or_default(),
                scratchpad: disk::load_text("scratchpad.txt"),
                settings,
                phase: Phase::Idle,
                notice: None,
                notice_token: 0,
                model_status: ModelStatus::Missing,
                polish_status: ModelStatus::Missing,
                input_trusted: platform::input_trusted(),
                mic: platform::mic_status(),
                downloads: BTreeMap::new(),
                target_app: None,
            }),
            handle,
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    // MARK: Lifecycle

    pub fn start(self: &Arc<Self>) {
        platform::set_hotkey(&self.lock().settings.hotkey);
        let app = self.clone();
        platform::start_hotkeys(move |event| match event {
            HotkeyEvent::Press => app.begin_recording(),
            HotkeyEvent::Release => app.end_recording(),
            HotkeyEvent::Cancel => app.cancel_recording(),
        });

        // Permissions can be granted at any time; keep the window's view of them current.
        let app = self.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(2));
            app.refresh_permissions();
        });

        // Opening the microphone once is what makes the OS ask for permission.
        if self.lock().mic == MicStatus::Undetermined {
            let app = self.clone();
            std::thread::spawn(move || {
                let _gate = app.mic_gate.lock().unwrap_or_else(|e| e.into_inner());
                if app.lock().phase == Phase::Idle && app.recorder.start().is_ok() {
                    std::thread::sleep(Duration::from_millis(250));
                    app.recorder.stop();
                }
            });
        }

        self.load_model();
        self.load_polish();
    }

    fn refresh_permissions(&self) {
        let trusted = platform::input_trusted();
        let mic = platform::mic_status();
        let changed = {
            let mut inner = self.lock();
            let changed = inner.input_trusted != trusted || inner.mic != mic;
            inner.input_trusted = trusted;
            inner.mic = mic;
            changed
        };
        if changed {
            self.emit_status();
        }
    }

    pub fn set_tray_toggle(&self, item: MenuItem<Wry>) {
        *self.tray_toggle.lock().unwrap_or_else(|e| e.into_inner()) = Some(item);
    }

    // MARK: State for the window

    fn status_of(inner: &Inner) -> Status {
        let (model_status, model_error) = status_name(&inner.model_status);
        let (polish_status, polish_error) = status_name(&inner.polish_status);
        Status {
            polish_status,
            polish_error,
            polish_models: POLISH_MODELS
                .iter()
                .map(|m| ModelView {
                    name: m.name,
                    title: m.title,
                    detail: m.detail,
                    size_mb: m.size_mb,
                    downloaded: m.is_downloaded(),
                    english_only: false,
                })
                .collect(),
            phase: inner.phase,
            notice: inner.notice.clone(),
            model_status,
            model_error,
            input_trusted: inner.input_trusted,
            mic_granted: inner.mic == MicStatus::Granted,
            downloads: inner.downloads.clone(),
            settings: inner.settings.clone(),
            models: MODELS
                .iter()
                .map(|m| ModelView {
                    name: m.name,
                    title: m.title,
                    detail: m.detail,
                    size_mb: m.size_mb,
                    downloaded: m.is_downloaded(),
                    english_only: m.english_only(),
                })
                .collect(),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let inner = self.lock();
        Snapshot {
            status: Self::status_of(&inner),
            entries: inner.entries.clone(),
            dictionary: inner.dictionary.clone(),
            snippets: inner.snippets.clone(),
            scratchpad: inner.scratchpad.clone(),
            hotkeys: platform::hotkey_options().iter().map(|k| HotkeyView { id: k.id, label: k.label }).collect(),
            platform: std::env::consts::OS,
            first_name: platform::user_first_name(),
            input_permission: platform::input_permission_name(),
            input_help: platform::input_help(),
        }
    }

    fn emit_status(&self) {
        let status = Self::status_of(&self.lock());
        if let Some(item) = self.tray_toggle.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let _ = item.set_text(if status.phase == Phase::Recording { "Stop dictation" } else { "Start dictation" });
            let _ = item.set_enabled(status.phase != Phase::Transcribing);
        }
        let _ = self.handle.emit("status", status);
    }

    fn emit_entries(&self) {
        let entries = self.lock().entries.clone();
        let _ = self.handle.emit("entries", entries);
    }

    /// Shows a short message in the floating pill.
    pub fn flash(self: &Arc<Self>, message: impl Into<String>) {
        let token = {
            let mut inner = self.lock();
            inner.notice = Some(message.into());
            inner.notice_token += 1;
            inner.notice_token
        };
        self.place_pill();
        self.emit_status();
        let app = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(2200));
            let cleared = {
                let mut inner = app.lock();
                let current = inner.notice_token == token;
                if current {
                    inner.notice = None;
                }
                current
            };
            if cleared {
                app.emit_status();
            }
        });
    }

    /// Moves the pill to the bottom centre of whichever screen the pointer is on.
    pub fn place_pill(&self) {
        let Some(pill) = self.handle.get_webview_window("pill") else { return };
        let monitor = self
            .handle
            .cursor_position()
            .ok()
            .and_then(|p| self.handle.monitor_from_point(p.x, p.y).ok().flatten())
            .or_else(|| pill.primary_monitor().ok().flatten());
        let Some(monitor) = monitor else { return };
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let (width, height, margin) = (260.0 * scale, 48.0 * scale, 28.0 * scale);
        let x = area.position.x as f64 + (area.size.width as f64 - width) / 2.0;
        let y = area.position.y as f64 + area.size.height as f64 - height - margin;
        let _ = pill.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    }

    // MARK: Model

    pub fn load_model(self: &Arc<Self>) {
        let info = ModelInfo::named(&self.lock().settings.model);
        if !info.is_downloaded() {
            self.lock().model_status = ModelStatus::Missing;
            self.emit_status();
            return;
        }
        self.lock().model_status = ModelStatus::Loading;
        self.emit_status();
        let app = self.clone();
        std::thread::spawn(move || {
            let path = info.local_path().to_string_lossy().into_owned();
            let result = match app.transcriber.load(&path) {
                Ok(()) => ModelStatus::Ready,
                Err(why) => ModelStatus::Failed(why),
            };
            {
                let mut inner = app.lock();
                if inner.settings.model != info.name {
                    return; // the user picked another model meanwhile
                }
                inner.model_status = result;
            }
            app.emit_status();
        });
    }

    /// Starts or stops the cleanup model to match the settings.
    pub fn load_polish(self: &Arc<Self>) {
        let (enabled, info) = {
            let inner = self.lock();
            (inner.settings.polish, PolishModel::named(&inner.settings.polish_model))
        };
        if !enabled || !info.is_downloaded() {
            self.polisher.stop();
            self.lock().polish_status = ModelStatus::Missing;
            self.emit_status();
            return;
        }
        self.lock().polish_status = ModelStatus::Loading;
        self.emit_status();
        let app = self.clone();
        std::thread::spawn(move || {
            let path = info.local_path().to_string_lossy().into_owned();
            let result = match app.polisher.load(&path) {
                Ok(()) => ModelStatus::Ready,
                Err(why) => ModelStatus::Failed(why),
            };
            {
                let mut inner = app.lock();
                if !inner.settings.polish || inner.settings.polish_model != info.name {
                    return; // switched off or changed meanwhile; that call owns the state now
                }
                inner.polish_status = result;
            }
            app.emit_status();
        });
    }

    fn download_polish_model(self: &Arc<Self>, info: &'static PolishModel) {
        {
            let mut inner = self.lock();
            if inner.downloads.contains_key(info.name) {
                return;
            }
            inner.downloads.insert(info.name.to_owned(), 0.0);
        }
        self.emit_status();
        let app = self.clone();
        std::thread::spawn(move || {
            let progress_app = app.clone();
            let result = download::fetch(info.url, &info.local_path(), move |fraction| {
                progress_app.lock().downloads.insert(info.name.to_owned(), fraction);
                progress_app.emit_status();
            });
            app.lock().downloads.remove(info.name);
            match result {
                Err(why) => app.flash(format!("Download failed: {why}")),
                Ok(()) => {
                    // Downloading a cleanup model is asking for it to be used.
                    {
                        let mut inner = app.lock();
                        inner.settings.polish = true;
                        inner.settings.polish_model = info.name.to_owned();
                        disk::save(&inner.settings, "settings.json");
                    }
                    app.load_polish();
                }
            }
        });
    }

    pub fn download_model(self: &Arc<Self>, name: &str) {
        if let Some(polish) = POLISH_MODELS.iter().find(|m| m.name == name) {
            return self.download_polish_model(polish);
        }
        let info = ModelInfo::named(name);
        {
            let mut inner = self.lock();
            if inner.downloads.contains_key(info.name) {
                return;
            }
            inner.downloads.insert(info.name.to_owned(), 0.0);
        }
        self.emit_status();
        let app = self.clone();
        std::thread::spawn(move || {
            let progress_app = app.clone();
            let result = download::fetch(&info.url(), &info.local_path(), move |fraction| {
                progress_app.lock().downloads.insert(info.name.to_owned(), fraction);
                progress_app.emit_status();
            });
            let select = {
                let mut inner = app.lock();
                inner.downloads.remove(info.name);
                result.is_ok() && (inner.settings.model == info.name || inner.model_status != ModelStatus::Ready)
            };
            match result {
                Err(why) => app.flash(format!("Download failed: {why}")),
                Ok(()) if select => {
                    app.lock().settings.model = info.name.to_owned();
                    disk::save(&app.lock().settings, "settings.json");
                    app.load_model();
                }
                Ok(()) => app.emit_status(),
            }
        });
    }

    pub fn delete_model(self: &Arc<Self>, name: &str) {
        if let Some(polish) = POLISH_MODELS.iter().find(|m| m.name == name) {
            if self.lock().settings.polish_model == polish.name {
                self.polisher.stop();
            }
            let _ = fs::remove_file(polish.local_path());
            return self.load_polish();
        }
        let info = ModelInfo::named(name);
        let _ = fs::remove_file(info.local_path());
        {
            let mut inner = self.lock();
            if inner.settings.model == info.name {
                inner.model_status = ModelStatus::Missing;
            }
        }
        self.emit_status();
    }

    // MARK: Dictation

    pub fn toggle_recording(self: &Arc<Self>) {
        let phase = self.lock().phase;
        match phase {
            Phase::Idle => self.begin_recording(),
            Phase::Recording => self.end_recording(),
            Phase::Transcribing => {}
        }
    }

    pub fn begin_recording(self: &Arc<Self>) {
        let sounds = {
            let mut inner = self.lock();
            if inner.phase != Phase::Idle {
                return;
            }
            if inner.model_status != ModelStatus::Ready {
                let message = if inner.model_status == ModelStatus::Loading {
                    "Model still loading…"
                } else {
                    "No speech model — open Settings"
                };
                drop(inner);
                self.flash(message);
                return;
            }
            if inner.mic == MicStatus::Denied {
                drop(inner);
                self.flash("Microphone access needed");
                return;
            }
            // Claim the phase before the slow part so a second press cannot start twice.
            inner.phase = Phase::Recording;
            inner.notice = None;
            inner.settings.sounds
        };

        let started = {
            let _gate = self.mic_gate.lock().unwrap_or_else(|e| e.into_inner());
            self.recorder.start()
        };
        if let Err(why) = started {
            self.lock().phase = Phase::Idle;
            self.flash(why);
            return;
        }
        self.lock().target_app = platform::frontmost_app();
        self.place_pill();
        self.emit_status();
        if sounds {
            platform::play_sound(Sound::Start);
        }
    }

    pub fn cancel_recording(self: &Arc<Self>) {
        {
            let mut inner = self.lock();
            if inner.phase != Phase::Recording {
                return;
            }
            inner.phase = Phase::Idle;
        }
        self.recorder.stop();
        self.emit_status();
    }

    pub fn end_recording(self: &Arc<Self>) {
        let (language, hint, dictionary, snippets, sounds, tidy, polish) = {
            let mut inner = self.lock();
            if inner.phase != Phase::Recording {
                return;
            }
            inner.phase = Phase::Transcribing;
            let model = ModelInfo::named(&inner.settings.model);
            let language = if model.english_only() { "en".to_owned() } else { inner.settings.language.clone() };
            (
                language,
                text::prompt(&inner.dictionary),
                inner.dictionary.clone(),
                inner.snippets.clone(),
                inner.settings.sounds,
                inner.settings.tidy,
                inner.settings.polish && inner.polish_status == ModelStatus::Ready,
            )
        };

        let samples = self.recorder.stop();
        let duration = samples.len() as f64 / SAMPLE_RATE as f64;
        if duration < 0.3 {
            self.lock().phase = Phase::Idle;
            self.emit_status();
            return;
        }
        let peak = samples.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
        if peak <= 0.008 {
            self.lock().phase = Phase::Idle;
            self.flash("Didn't hear anything");
            return;
        }
        self.emit_status();
        if sounds {
            platform::play_sound(Sound::Stop);
        }

        let app = self.clone();
        std::thread::spawn(move || {
            let result = app
                .transcriber
                .transcribe(&samples, &language, &hint)
                .map(|raw| {
                    text::apply_with(&dictionary, &snippets, tidy, &raw, |tidied| {
                        if !polish {
                            return None;
                        }
                        let glossary: Vec<String> = dictionary.iter().map(|d| d.word.clone()).collect();
                        app.polisher.polish(tidied, &glossary).ok()
                    })
                });
            // A timeout stops the helper; bring it back for the next dictation.
            if polish && !app.polisher.is_loaded() {
                app.load_polish();
            }
            app.finish(result, samples, duration);
        });
    }

    fn finish(self: &Arc<Self>, result: Result<text::Applied, String>, samples: Vec<f32>, duration: f64) {
        self.lock().phase = Phase::Idle;
        let applied = match result {
            Err(why) => return self.flash(why),
            Ok(a) if a.text.is_empty() => return self.flash("Didn't catch that"),
            Ok(a) => a,
        };
        let transcript = applied.text;

        let mut entry = Entry {
            id: core::new_id(),
            date: core::now_iso(),
            words: text::word_count(&transcript),
            text: transcript.clone(),
            duration,
            audio_file: None,
            app: None,
            dict_fixes: applied.dict_fixes,
            tidy_fixes: applied.tidy_fixes,
        };
        let keep_audio = {
            let mut inner = self.lock();
            entry.app = inner.target_app.take();
            if inner.settings.keep_audio {
                entry.audio_file = Some(format!("{}.wav", entry.id));
            }
            inner.entries.insert(0, entry.clone());
            disk::save(&inner.entries, "history.json");
            inner.settings.keep_audio
        };
        if keep_audio {
            let path = paths::audio().join(format!("{}.wav", entry.id));
            std::thread::spawn(move || {
                let _ = audio::write_wav(&samples, &path);
            });
        }
        self.emit_entries();
        self.emit_status();
        self.deliver(&transcript);
    }

    fn deliver(self: &Arc<Self>, transcript: &str) {
        // Inside hams itself the window decides: it types into a focused text field,
        // and otherwise asks for the text to be copied.
        let own_window = self
            .handle
            .get_webview_window("main")
            .is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
        if own_window {
            let _ = self.handle.emit_to("main", "insert", format!("{transcript} "));
            return;
        }

        let (auto_paste, trusted) = {
            let inner = self.lock();
            (inner.settings.auto_paste, inner.input_trusted)
        };
        if auto_paste && trusted {
            if let Err(why) = platform::paste(&format!("{transcript} ")) {
                let _ = platform::copy(transcript);
                self.flash(format!("Copied — {why}"));
            }
            return;
        }
        let _ = platform::copy(transcript);
        if trusted || !auto_paste {
            self.flash("Copied to clipboard");
        } else {
            self.flash(format!("Copied — allow {} to auto-type", platform::input_permission_name()));
        }
    }

    // MARK: History

    pub fn delete_entry(&self, id: &str) {
        {
            let mut inner = self.lock();
            if let Some(file) = inner.entries.iter().find(|e| e.id == id).and_then(|e| e.audio_file.clone()) {
                let _ = fs::remove_file(paths::audio().join(file));
            }
            inner.entries.retain(|e| e.id != id);
            disk::save(&inner.entries, "history.json");
        }
        self.emit_entries();
    }

    pub fn clear_history(&self) {
        {
            let mut inner = self.lock();
            for file in inner.entries.iter().filter_map(|e| e.audio_file.as_ref()) {
                let _ = fs::remove_file(paths::audio().join(file));
            }
            inner.entries.clear();
            disk::save(&inner.entries, "history.json");
        }
        self.emit_entries();
    }

    pub fn copy_text(self: &Arc<Self>, value: &str) {
        match platform::copy(value) {
            Ok(()) => self.flash("Copied to clipboard"),
            Err(why) => self.flash(format!("Could not copy: {why}")),
        }
    }

    pub fn paste_last(self: &Arc<Self>) {
        let (last, trusted) = {
            let inner = self.lock();
            (inner.entries.first().map(|e| e.text.clone()), inner.input_trusted)
        };
        let Some(last) = last else { return };
        if trusted && platform::paste(&format!("{last} ")).is_ok() {
            return;
        }
        self.copy_text(&last);
    }

    /// Bytes of a kept recording, for playback in the window.
    pub fn read_audio(&self, file: &str) -> Result<Vec<u8>, String> {
        // History only ever stores bare file names; refuse anything that could leave the folder.
        if file.contains(['/', '\\']) || file.contains("..") {
            return Err("Invalid file name".into());
        }
        fs::read(paths::audio().join(file)).map_err(|e| e.to_string())
    }

    // MARK: Editing

    pub fn set_dictionary(&self, words: Vec<DictWord>) {
        let mut inner = self.lock();
        inner.dictionary = words;
        disk::save(&inner.dictionary, "dictionary.json");
    }

    pub fn set_snippets(&self, snippets: Vec<Snippet>) {
        let mut inner = self.lock();
        inner.snippets = snippets;
        disk::save(&inner.snippets, "snippets.json");
    }

    pub fn set_scratchpad(&self, value: String) {
        let mut inner = self.lock();
        if inner.scratchpad != value {
            disk::write_atomic("scratchpad.txt", value.as_bytes());
            inner.scratchpad = value;
        }
    }

    pub fn set_settings(self: &Arc<Self>, mut settings: Settings) {
        if !platform::hotkey_options().iter().any(|k| k.id == settings.hotkey) {
            settings.hotkey = platform::hotkey_options()[0].id.to_owned();
        }
        settings.model = ModelInfo::named(&settings.model).name.to_owned();
        settings.polish_model = PolishModel::named(&settings.polish_model).name.to_owned();
        let (model_changed, polish_changed) = {
            let mut inner = self.lock();
            let model_changed = inner.settings.model != settings.model;
            let polish_changed =
                inner.settings.polish != settings.polish || inner.settings.polish_model != settings.polish_model;
            if inner.settings.hotkey != settings.hotkey {
                platform::set_hotkey(&settings.hotkey);
            }
            inner.settings = settings;
            disk::save(&inner.settings, "settings.json");
            (model_changed, polish_changed)
        };
        if polish_changed {
            self.load_polish();
        }
        if model_changed {
            self.load_model();
        } else {
            self.emit_status();
        }
    }
}
