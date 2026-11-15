//! OS glue for hams: the push-to-talk key, typing into the focused app, and the
//! permissions both of those need. Everything above this crate is platform-neutral.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod os;
#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod os;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
#[path = "other.rs"]
mod os;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyEvent {
    Press,
    Release,
    Cancel,
}

pub struct HotkeyOption {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicStatus {
    Granted,
    Denied,
    Undetermined,
}

#[derive(Clone, Copy, Debug)]
pub enum Sound {
    Start,
    Stop,
}

// MARK: - Push-to-talk key

type Handler = Box<dyn Fn(HotkeyEvent) + Send + Sync>;

static HANDLER: OnceLock<Handler> = OnceLock::new();
static KEY_INDEX: AtomicUsize = AtomicUsize::new(0);
static KEY_DOWN: AtomicBool = AtomicBool::new(false);

/// The modifier keys that can be used for push-to-talk on this OS. The first is the default.
pub fn hotkey_options() -> &'static [HotkeyOption] {
    os::HOTKEYS
}

pub fn set_hotkey(id: &str) {
    let index = os::HOTKEYS.iter().position(|k| k.id == id).unwrap_or(0);
    KEY_INDEX.store(index, Ordering::SeqCst);
    KEY_DOWN.store(false, Ordering::SeqCst);
}

/// Starts watching the push-to-talk key system-wide. Safe to call before the OS has
/// granted access: the watcher keeps retrying and arms itself once access appears.
pub fn start_hotkeys(handler: impl Fn(HotkeyEvent) + Send + Sync + 'static) {
    if HANDLER.set(Box::new(handler)).is_ok() {
        os::start_hotkeys();
    }
}

pub(crate) fn key_index() -> usize {
    KEY_INDEX.load(Ordering::SeqCst)
}

fn send(event: HotkeyEvent) {
    if let Some(handler) = HANDLER.get() {
        handler(event);
    }
}

/// The chosen modifier changed state: press starts, release stops.
pub(crate) fn modifier_changed(pressed: bool) {
    let down = KEY_DOWN.load(Ordering::SeqCst);
    if !down && pressed {
        KEY_DOWN.store(true, Ordering::SeqCst);
        send(HotkeyEvent::Press);
    } else if down && !pressed {
        KEY_DOWN.store(false, Ordering::SeqCst);
        send(HotkeyEvent::Release);
    }
}

/// Any other key went down. While the modifier is held that cancels, so ordinary
/// shortcuts using the same modifier keep working. Escape always cancels.
pub(crate) fn other_key_down(is_escape: bool) {
    if KEY_DOWN.swap(false, Ordering::SeqCst) || is_escape {
        send(HotkeyEvent::Cancel);
    }
}

// MARK: - Permissions

/// Whether the OS lets this process watch the keyboard globally and type into other apps.
pub fn input_trusted() -> bool {
    os::input_trusted()
}

/// Asks the OS to show its own permission prompt, where there is one.
pub fn request_input_access() {
    os::request_input_access();
}

pub fn open_input_settings() {
    os::open_input_settings();
}

/// One or two sentences telling the user how to grant input access on this OS.
pub fn input_help() -> &'static str {
    os::INPUT_HELP
}

/// Name of the permission as the OS calls it.
pub fn input_permission_name() -> &'static str {
    os::INPUT_PERMISSION
}

pub fn mic_status() -> MicStatus {
    os::mic_status()
}

pub fn open_mic_settings() {
    os::open_mic_settings();
}

// MARK: - Clipboard and typing

#[cfg(not(target_os = "linux"))]
fn with_clipboard<T>(
    f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
) -> Result<T, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    f(&mut clipboard).map_err(|e| e.to_string())
}

/// On Linux the clipboard contents live in the owning process, so one handle is kept
/// for the lifetime of the app.
#[cfg(target_os = "linux")]
fn with_clipboard<T>(
    f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
) -> Result<T, String> {
    static CLIPBOARD: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);
    let mut guard = CLIPBOARD.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
    }
    f(guard.as_mut().unwrap()).map_err(|e| e.to_string())
}

static PASTE_LOCK: Mutex<()> = Mutex::new(());

pub fn copy(text: &str) -> Result<(), String> {
    with_clipboard(|c| c.set_text(text.to_owned()))
}

/// Puts `text` on the clipboard, sends the paste shortcut to the focused app, then
/// restores whatever was on the clipboard before.
pub fn paste(text: &str) -> Result<(), String> {
    let _guard = PASTE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let previous = with_clipboard(|c| c.get_text()).ok();
    copy(text)?;
    // Give the pasteboard a moment to publish the new contents before the keystroke lands.
    std::thread::sleep(Duration::from_millis(40));
    os::send_paste()?;
    if let Some(previous) = previous {
        let text = text.to_owned();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(800));
            let unchanged = with_clipboard(|c| c.get_text()).map(|now| now == text).unwrap_or(false);
            if unchanged {
                let _ = copy(&previous);
            }
        });
    }
    Ok(())
}

// MARK: - Small conveniences

/// Display name of the app that currently has keyboard focus, if the OS can tell us.
pub fn frontmost_app() -> Option<String> {
    os::frontmost_app()
}

pub fn play_sound(sound: Sound) {
    os::play_sound(sound);
}

pub fn user_first_name() -> Option<String> {
    let full = os::user_full_name()?;
    let first = full.split_whitespace().next()?.trim_matches(',').to_owned();
    if first.is_empty() { None } else { Some(first) }
}

/// Opens a folder in the file manager.
pub fn open_path(path: &Path) {
    os::open_path(path);
}

/// Makes a window behave like a floating indicator: above everything, on every
/// desktop, left out of window switching. `handle` is the native window pointer
/// (an `NSWindow*` on macOS); it is ignored where the toolkit already does the job.
///
/// # Safety
/// `handle` must be a valid native window pointer and, on macOS, this must be called
/// on the main thread.
pub unsafe fn configure_overlay(handle: *mut std::ffi::c_void) {
    os::configure_overlay(handle);
}

#[allow(dead_code)]
pub(crate) fn run_quiet(program: &str, args: &[&str]) {
    let _ = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[allow(dead_code)]
pub(crate) fn output_of(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if text.is_empty() { None } else { Some(text) }
}
