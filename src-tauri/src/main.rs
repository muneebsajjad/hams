// hams: local voice dictation. Hold a key, speak, release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod core;
mod download;
mod polish;
mod transcriber;

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use tauri::image::Image;
use tauri::ipc::Response;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, RunEvent, State, WindowEvent};

use crate::app::{App, Snapshot};
use crate::core::{DictWord, ModelInfo, PolishModel, Settings, Snippet, MODELS, POLISH_MODELS};

type AppState<'a> = State<'a, Arc<App>>;

// MARK: - Commands the window can call

#[tauri::command]
fn snapshot(app: AppState) -> Snapshot {
    app.snapshot()
}

#[tauri::command]
fn toggle_recording(app: AppState) {
    app.toggle_recording();
}

#[tauri::command]
fn set_settings(app: AppState, settings: Settings) {
    app.set_settings(settings);
}

#[tauri::command]
fn set_dictionary(app: AppState, words: Vec<DictWord>) {
    app.set_dictionary(words);
}

#[tauri::command]
fn set_snippets(app: AppState, snippets: Vec<Snippet>) {
    app.set_snippets(snippets);
}

#[tauri::command]
fn set_scratchpad(app: AppState, text: String) {
    app.set_scratchpad(text);
}

#[tauri::command]
fn delete_entry(app: AppState, id: String) {
    app.delete_entry(&id);
}

#[tauri::command]
fn clear_history(app: AppState) {
    app.clear_history();
}

#[tauri::command]
fn copy_text(app: AppState, text: String) {
    app.copy_text(&text);
}

#[tauri::command]
fn read_audio(app: AppState, file: String) -> Result<Response, String> {
    app.read_audio(&file).map(Response::new)
}

#[tauri::command]
fn download_model(app: AppState, name: String) {
    app.download_model(&name);
}

#[tauri::command]
fn delete_model(app: AppState, name: String) {
    app.delete_model(&name);
}

#[tauri::command]
fn request_input_access() {
    hams_platform::request_input_access();
    hams_platform::open_input_settings();
}

#[tauri::command]
fn open_input_settings() {
    hams_platform::open_input_settings();
}

#[tauri::command]
fn open_mic_settings() {
    hams_platform::open_mic_settings();
}

#[tauri::command]
fn open_data_folder() {
    hams_platform::open_path(&core::paths::root());
}

#[tauri::command]
fn restart_app(handle: AppHandle) {
    handle.restart();
}

// MARK: - Window, tray, lifecycle

fn show_main(handle: &AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn build_tray(handle: &AppHandle, app: &Arc<App>) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(handle, "toggle", "Start dictation", true, None::<&str>)?;
    let paste = MenuItem::with_id(handle, "paste", "Paste last transcript", true, None::<&str>)?;
    let open = MenuItem::with_id(handle, "open", "Open hams", true, None::<&str>)?;
    let quit = MenuItem::with_id(handle, "quit", "Quit hams", true, Some("CmdOrCtrl+Q"))?;
    let menu = Menu::with_items(
        handle,
        &[&toggle, &paste, &PredefinedMenuItem::separator(handle)?, &open, &quit],
    )?;
    app.set_tray_toggle(toggle);

    let app = app.clone();
    TrayIconBuilder::with_id("hams")
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("hams")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |handle, event| match event.id().as_ref() {
            // Dictation work is kept off the UI thread.
            "toggle" => {
                let app = app.clone();
                std::thread::spawn(move || app.toggle_recording());
            }
            "paste" => {
                let app = app.clone();
                std::thread::spawn(move || app.paste_last());
            }
            "open" => show_main(handle),
            "quit" => handle.exit(0),
            _ => {}
        })
        .build(handle)?;
    Ok(())
}

fn run_app() {
    let built = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            snapshot,
            toggle_recording,
            set_settings,
            set_dictionary,
            set_snippets,
            set_scratchpad,
            delete_entry,
            clear_history,
            copy_text,
            read_audio,
            download_model,
            delete_model,
            request_input_access,
            open_input_settings,
            open_mic_settings,
            open_data_folder,
            restart_app,
        ])
        .setup(|tauri_app| {
            let handle = tauri_app.handle().clone();
            let app = App::new(handle.clone());
            tauri_app.manage(app.clone());

            if let Some(pill) = handle.get_webview_window("pill") {
                // The pill is always on screen but transparent and click-through while
                // idle. It is never shown or hidden, so it can never take focus away
                // from the app being dictated into.
                let _ = pill.set_ignore_cursor_events(true);
                #[cfg(target_os = "macos")]
                if let Ok(ns_window) = pill.ns_window() {
                    // SAFETY: setup runs on the main thread and the pointer comes from a live window.
                    unsafe { hams_platform::configure_overlay(ns_window) };
                }
            }

            app.place_pill();
            build_tray(&handle, &app)?;
            app.start();
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps hams running in the tray, ready for the key.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building hams");

    built.run(|_handle, _event| {
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = _event {
            show_main(_handle);
        }
    });
}

// MARK: - Headless engine check

/// `hams --transcribe clip.wav [--model name] [--lang en] [--prompt text] [--verbose]`
fn transcribe_cli(file: &str, args: &[String]) -> i32 {
    let option = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let model = match option("--model") {
        Some(name) => ModelInfo::named(&name),
        None => MODELS.iter().find(|m| m.is_downloaded()).unwrap_or(&MODELS[0]),
    };
    let run = || -> Result<(), String> {
        let engine = transcriber::Transcriber::new(args.iter().any(|a| a == "--verbose"));
        let started = Instant::now();
        engine.load(&model.local_path().to_string_lossy())?;
        let load_time = started.elapsed().as_secs_f64();
        let samples = audio::read_wav(Path::new(file))?;
        let started = Instant::now();
        let text = engine.transcribe(
            &samples,
            &option("--lang").unwrap_or_else(|| "en".into()),
            &option("--prompt").unwrap_or_default(),
        )?;
        println!("{text}");
        eprintln!(
            "model={} load={:.2}s audio={:.1}s transcribe={:.2}s",
            model.name,
            load_time,
            samples.len() as f64 / audio::SAMPLE_RATE as f64,
            started.elapsed().as_secs_f64()
        );
        Ok(())
    };
    match run() {
        Ok(()) => 0,
        Err(why) => {
            eprintln!("error: {why}");
            1
        }
    }
}

/// `hams --polish notes.txt [--model name]`: runs the long-dictation cleanup on a text
/// file exactly as a dictation would get it, and says whether the model's rewrite was kept.
fn polish_cli(file: &str, args: &[String]) -> i32 {
    let option = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let model = match option("--model") {
        Some(name) => PolishModel::named(&name),
        None => POLISH_MODELS.iter().find(|m| m.is_downloaded()).unwrap_or(&POLISH_MODELS[0]),
    };
    let run = || -> Result<(), String> {
        let input = std::fs::read_to_string(file).map_err(|e| format!("could not read {file} ({e})"))?;
        let input = core::text::clean(&input);
        let polisher = polish::Polisher::new();
        let started = Instant::now();
        polisher.load(&model.local_path().to_string_lossy())?;
        let load_time = started.elapsed().as_secs_f64();
        let started = Instant::now();
        let mut verdict = "not long enough to polish";
        let applied = core::text::apply_with(&[], &[], true, &input, |tidied| {
            let rewrite = polisher.polish(tidied, &[]);
            verdict = match &rewrite {
                Err(_) => "model failed, rules only",
                Ok(r) if core::text::faithful(tidied, r).is_some() => "rewrite kept",
                Ok(_) => "rewrite rejected as unfaithful, rules only",
            };
            if let Ok(r) = &rewrite {
                eprintln!("model said: {r}");
            }
            rewrite.ok()
        });
        println!("{}", applied.text);
        eprintln!(
            "model={} load={:.2}s polish={:.2}s fixes={} verdict: {}",
            model.name,
            load_time,
            started.elapsed().as_secs_f64(),
            applied.tidy_fixes,
            verdict
        );
        Ok(())
    };
    match run() {
        Ok(()) => 0,
        Err(why) => {
            eprintln!("error: {why}");
            1
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(file) = args.iter().position(|a| a == "--polish").and_then(|i| args.get(i + 1)) {
        std::process::exit(polish_cli(file, &args));
    }
    if let Some(file) = args.iter().position(|a| a == "--transcribe").and_then(|i| args.get(i + 1)) {
        std::process::exit(transcribe_cli(file, &args));
    }
    run_app();
}
