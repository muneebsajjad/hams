//! Platforms without an implementation yet: dictation still works from the window
//! and results land on the clipboard, but there is no global key and no auto-typing.

use std::ffi::c_void;
use std::path::Path;

use crate::{HotkeyOption, MicStatus, Sound};

pub const HOTKEYS: &[HotkeyOption] = &[HotkeyOption { id: "none", label: "Not available on this system" }];
pub const INPUT_PERMISSION: &str = "Input access";
pub const INPUT_HELP: &str = "A global key and auto-typing are not available on this system yet.";

pub fn start_hotkeys() {}

pub fn input_trusted() -> bool {
    false
}

pub fn request_input_access() {}

pub fn open_input_settings() {}

pub fn mic_status() -> MicStatus {
    MicStatus::Granted
}

pub fn open_mic_settings() {}

pub fn send_paste() -> Result<(), String> {
    Err("Typing into other apps is not available on this system".into())
}

pub fn frontmost_app() -> Option<String> {
    None
}

pub fn play_sound(_sound: Sound) {}

pub fn user_full_name() -> Option<String> {
    std::env::var("USERNAME").ok()
}

pub fn open_path(_path: &Path) {}

pub unsafe fn configure_overlay(_handle: *mut c_void) {}
