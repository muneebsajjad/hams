//! Linux: the key is read straight from the kernel input devices (evdev) and typing
//! goes through a virtual keyboard (uinput). That works the same on X11 and Wayland,
//! where a compositor would otherwise refuse both. The price is one-time setup:
//! the user needs read access to /dev/input/event* and write access to /dev/uinput.

use std::collections::HashSet;
use std::ffi::c_void;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, Device, EventType, InputEvent, KeyCode};

use crate::{HotkeyOption, MicStatus, Sound};

pub const HOTKEYS: &[HotkeyOption] = &[
    HotkeyOption { id: "rightOption", label: "Right Alt" },
    HotkeyOption { id: "rightControl", label: "Right Ctrl" },
    HotkeyOption { id: "rightCommand", label: "Right Super" },
];

const KEYS: [KeyCode; 3] = [KeyCode::KEY_RIGHTALT, KeyCode::KEY_RIGHTCTRL, KeyCode::KEY_RIGHTMETA];

pub const INPUT_PERMISSION: &str = "Input device access";
pub const INPUT_HELP: &str = "Run `sudo usermod -aG input $USER`, then add a udev rule `KERNEL==\"uinput\", GROUP=\"input\", MODE=\"0660\"` in /etc/udev/rules.d/99-hams.rules, and log out and back in.";

const VIRTUAL_NAME: &str = "hams virtual keyboard";

// MARK: - Push-to-talk key

/// Device nodes that already have a watcher thread.
static OPEN: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

fn is_keyboard(device: &Device) -> bool {
    if device.name() == Some(VIRTUAL_NAME) {
        return false;
    }
    device
        .supported_keys()
        .map(|keys| keys.contains(KeyCode::KEY_A) && keys.contains(KeyCode::KEY_ENTER))
        .unwrap_or(false)
}

fn watch(path: PathBuf, mut device: Device) {
    std::thread::spawn(move || {
        // Ends when the device is unplugged; the scanner picks it up again if it returns.
        while let Ok(events) = device.fetch_events() {
            for event in events {
                if event.event_type() != EventType::KEY {
                    continue;
                }
                let target = KEYS[crate::key_index().min(KEYS.len() - 1)];
                let code = KeyCode(event.code());
                match event.value() {
                    1 if code == target => crate::modifier_changed(true),
                    0 if code == target => crate::modifier_changed(false),
                    1 => crate::other_key_down(code == KeyCode::KEY_ESC),
                    _ => {}
                }
            }
        }
        OPEN.lock().unwrap_or_else(|e| e.into_inner()).remove(&path);
    });
}

pub fn start_hotkeys() {
    std::thread::Builder::new()
        .name("hams-hotkey".into())
        .spawn(|| loop {
            // Re-scan so keyboards plugged in later, or access granted later, are picked up.
            for (path, device) in evdev::enumerate() {
                if !is_keyboard(&device) {
                    continue;
                }
                let fresh = OPEN.lock().unwrap_or_else(|e| e.into_inner()).insert(path.clone());
                if fresh {
                    watch(path, device);
                }
            }
            std::thread::sleep(Duration::from_secs(3));
        })
        .expect("spawn hotkey thread");
}

// MARK: - Permissions

fn can_write_uinput() -> bool {
    OpenOptions::new().write(true).open("/dev/uinput").is_ok()
}

pub fn input_trusted() -> bool {
    can_write_uinput() && evdev::enumerate().any(|(_, device)| is_keyboard(&device))
}

pub fn request_input_access() {}

pub fn open_input_settings() {}

pub fn mic_status() -> MicStatus {
    MicStatus::Granted
}

pub fn open_mic_settings() {}

// MARK: - Typing

static KEYBOARD: Mutex<Option<VirtualDevice>> = Mutex::new(None);

fn build_keyboard() -> std::io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    keys.insert(KeyCode::KEY_LEFTCTRL);
    keys.insert(KeyCode::KEY_LEFTSHIFT);
    keys.insert(KeyCode::KEY_V);
    let device = VirtualDevice::builder()?.name(VIRTUAL_NAME).with_keys(&keys)?.build()?;
    // The compositor needs a moment to notice a new keyboard before it accepts its keys.
    std::thread::sleep(Duration::from_millis(300));
    Ok(device)
}

fn key(code: KeyCode, value: i32) -> InputEvent {
    InputEvent::new(EventType::KEY.0, code.code(), value)
}

pub fn send_paste() -> Result<(), String> {
    let mut guard = KEYBOARD.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(build_keyboard().map_err(|e| format!("Could not open /dev/uinput: {e}"))?);
    }
    let keyboard = guard.as_mut().unwrap();
    let steps =
        [key(KeyCode::KEY_LEFTCTRL, 1), key(KeyCode::KEY_V, 1), key(KeyCode::KEY_V, 0), key(KeyCode::KEY_LEFTCTRL, 0)];
    for step in steps {
        keyboard.emit(&[step]).map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(8));
    }
    Ok(())
}

// MARK: - Conveniences

pub fn frontmost_app() -> Option<String> {
    None
}

pub fn play_sound(sound: Sound) {
    let id = match sound {
        Sound::Start => "message-new-instant",
        Sound::Stop => "message",
    };
    crate::run_quiet("canberra-gtk-play", &["-i", id]);
}

pub fn user_full_name() -> Option<String> {
    let user = std::env::var("USER").ok()?;
    let entry = crate::output_of("getent", &["passwd", &user])?;
    let gecos = entry.split(':').nth(4)?.split(',').next()?.trim().to_owned();
    if gecos.is_empty() {
        None
    } else {
        Some(gecos)
    }
}

pub fn open_path(path: &Path) {
    if let Some(p) = path.to_str() {
        crate::run_quiet("xdg-open", &[p]);
    }
}

pub unsafe fn configure_overlay(_handle: *mut c_void) {}
