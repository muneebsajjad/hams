//! macOS: a CoreGraphics event tap for the key, a synthetic ⌘V for typing.
//! Both are covered by the single Accessibility permission.

use std::ffi::{c_char, c_void, CStr};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Duration;

use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use objc2::rc::autoreleasepool;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

use crate::{HotkeyOption, MicStatus, Sound};

pub const HOTKEYS: &[HotkeyOption] = &[
    HotkeyOption { id: "rightOption", label: "Right Option ⌥" },
    HotkeyOption { id: "rightCommand", label: "Right Command ⌘" },
    HotkeyOption { id: "rightControl", label: "Right Control ⌃" },
    HotkeyOption { id: "fn", label: "Fn / Globe" },
];

/// (virtual key code, modifier flag) for each entry of `HOTKEYS`.
const KEYS: [(i64, u64); 4] = [
    (61, 0x0008_0000), // right option  / kCGEventFlagMaskAlternate
    (54, 0x0010_0000), // right command / kCGEventFlagMaskCommand
    (62, 0x0004_0000), // right control / kCGEventFlagMaskControl
    (63, 0x0080_0000), // fn            / kCGEventFlagMaskSecondaryFn
];

pub const INPUT_PERMISSION: &str = "Accessibility";
pub const INPUT_HELP: &str = "Open System Settings → Privacy & Security → Accessibility, add hams and switch it on, then restart hams.";

type CGEventRef = *mut c_void;
type CFMachPortRef = *mut c_void;
type TapCallback = extern "C" fn(*mut c_void, u32, CGEventRef, *mut c_void) -> CGEventRef;

const EVENT_KEY_DOWN: u32 = 10;
const EVENT_FLAGS_CHANGED: u32 = 12;
const EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const EVENT_TAP_DISABLED_BY_USER: u32 = 0xFFFF_FFFF;
const FIELD_KEYCODE: u32 = 9;
const KEYCODE_ESCAPE: i64 = 53;
const KEYCODE_V: u16 = 9;
const FLAG_COMMAND: u64 = 0x0010_0000;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: TapCallback,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventSourceCreate(state_id: i32) -> *mut c_void;
    fn CGEventCreateKeyboardEvent(source: *mut c_void, keycode: u16, key_down: bool) -> CGEventRef;
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventPost(tap: u32, event: CGEventRef);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopCommonModes: *const c_void;
    fn CFMachPortCreateRunLoopSource(allocator: *const c_void, port: CFMachPortRef, order: isize) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRun();
    fn CFRelease(cf: *const c_void);
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
}

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {
    static AVMediaTypeAudio: *const AnyObject;
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

// MARK: - Push-to-talk key

static TAP: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

extern "C" fn tap_callback(_proxy: *mut c_void, kind: u32, event: CGEventRef, _user: *mut c_void) -> CGEventRef {
    match kind {
        EVENT_TAP_DISABLED_BY_TIMEOUT | EVENT_TAP_DISABLED_BY_USER => {
            let tap = TAP.load(Ordering::SeqCst);
            if !tap.is_null() {
                unsafe { CGEventTapEnable(tap, true) };
            }
        }
        EVENT_FLAGS_CHANGED => {
            let (code, flag) = KEYS[crate::key_index().min(KEYS.len() - 1)];
            if unsafe { CGEventGetIntegerValueField(event, FIELD_KEYCODE) } == code {
                crate::modifier_changed(unsafe { CGEventGetFlags(event) } & flag != 0);
            }
        }
        EVENT_KEY_DOWN => {
            let code = unsafe { CGEventGetIntegerValueField(event, FIELD_KEYCODE) };
            crate::other_key_down(code == KEYCODE_ESCAPE);
        }
        _ => {}
    }
    event
}

/// Installs the tap and runs its loop. Returns only if the tap could not be created.
fn run_tap() {
    let mask: u64 = (1 << EVENT_KEY_DOWN) | (1 << EVENT_FLAGS_CHANGED);
    unsafe {
        // Session tap, head insert. Listen-only first so hams never sits in the path of
        // the user's keystrokes; an active tap is the fallback.
        let mut tap = CGEventTapCreate(1, 0, 1, mask, tap_callback, ptr::null_mut());
        if tap.is_null() {
            tap = CGEventTapCreate(1, 0, 0, mask, tap_callback, ptr::null_mut());
        }
        if tap.is_null() {
            return;
        }
        TAP.store(tap, Ordering::SeqCst);
        let source = CFMachPortCreateRunLoopSource(ptr::null(), tap, 0);
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        CFRunLoopRun();
        TAP.store(ptr::null_mut(), Ordering::SeqCst);
        CFRelease(source);
        CFRelease(tap);
    }
}

pub fn start_hotkeys() {
    std::thread::Builder::new()
        .name("hams-hotkey".into())
        .spawn(|| loop {
            if input_trusted() {
                run_tap();
            }
            std::thread::sleep(Duration::from_secs(2));
        })
        .expect("spawn hotkey thread");
}

// MARK: - Permissions

pub fn input_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

pub fn request_input_access() {
    let key = CFString::from_static_string("AXTrustedCheckOptionPrompt");
    let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as *const c_void) };
}

pub fn open_input_settings() {
    crate::run_quiet("open", &["x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"]);
}

pub fn mic_status() -> MicStatus {
    // AVAuthorizationStatus: 0 not determined, 1 restricted, 2 denied, 3 authorized.
    let status: isize = unsafe { msg_send![class!(AVCaptureDevice), authorizationStatusForMediaType: AVMediaTypeAudio] };
    match status {
        3 => MicStatus::Granted,
        0 => MicStatus::Undetermined,
        _ => MicStatus::Denied,
    }
}

pub fn open_mic_settings() {
    crate::run_quiet("open", &["x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"]);
}

// MARK: - Typing

pub fn send_paste() -> Result<(), String> {
    if !input_trusted() {
        return Err("Accessibility access is not granted".into());
    }
    unsafe {
        let source = CGEventSourceCreate(0); // combined session state
        let down = CGEventCreateKeyboardEvent(source, KEYCODE_V, true);
        let up = CGEventCreateKeyboardEvent(source, KEYCODE_V, false);
        if down.is_null() || up.is_null() {
            return Err("Could not create key events".into());
        }
        CGEventSetFlags(down, FLAG_COMMAND);
        CGEventSetFlags(up, FLAG_COMMAND);
        CGEventPost(0, down); // HID event tap
        CGEventPost(0, up);
        CFRelease(down);
        CFRelease(up);
        if !source.is_null() {
            CFRelease(source);
        }
    }
    Ok(())
}

// MARK: - Conveniences

pub fn frontmost_app() -> Option<String> {
    autoreleasepool(|_| unsafe {
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return None;
        }
        let app: *mut AnyObject = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return None;
        }
        let name: *mut AnyObject = msg_send![app, localizedName];
        if name.is_null() {
            return None;
        }
        let utf8: *const c_char = msg_send![name, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
    })
}

pub fn play_sound(sound: Sound) {
    let file = match sound {
        Sound::Start => "/System/Library/Sounds/Tink.aiff",
        Sound::Stop => "/System/Library/Sounds/Pop.aiff",
    };
    crate::run_quiet("afplay", &[file]);
}

pub fn user_full_name() -> Option<String> {
    crate::output_of("id", &["-F"])
}

pub fn open_path(path: &Path) {
    if let Some(p) = path.to_str() {
        crate::run_quiet("open", &[p]);
    }
}

pub unsafe fn configure_overlay(handle: *mut c_void) {
    let window = handle as *mut AnyObject;
    if window.is_null() {
        return;
    }
    // NSStatusWindowLevel, so the indicator floats above ordinary and full-screen windows.
    let _: () = msg_send![window, setLevel: 25isize];
    // canJoinAllSpaces | stationary | ignoresCycle | fullScreenAuxiliary
    let behavior: usize = (1 << 0) | (1 << 4) | (1 << 6) | (1 << 8);
    let _: () = msg_send![window, setCollectionBehavior: behavior];
    let _: () = msg_send![window, setHasShadow: false];
}
