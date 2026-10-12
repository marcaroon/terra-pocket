//! Terra companion GUI (M10): window, tray, auto connect and startup option.
//! A thin shell over companion-core; no Bluetooth, media or audio logic here.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod status;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use companion_core::session::{Control, Exit, Phase, SessionOptions, Update, run_forever};
use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use status::Status;

/// Passed by the Windows startup entry: start in the tray without a window.
const MINIMIZED_ARG: &str = "--minimized";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Stored {
    auto_connect: bool,
}

impl Default for Stored {
    fn default() -> Self {
        Self { auto_connect: true }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    auto_connect: bool,
    start_with_windows: bool,
}

/// One running `run_forever` thread.
struct Service {
    generation: u64,
    control: Arc<Control>,
}

struct AppState {
    status: Mutex<Status>,
    service: Mutex<Option<Service>>,
    /// Updates from an older, stopping service carry an older generation and are ignored.
    generation: AtomicU64,
    stored: Mutex<Stored>,
    settings_path: PathBuf,
    tray_status: MenuItem<Wry>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn load_settings(path: &Path) -> Stored {
    std::fs::read(path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

fn save_settings(path: &Path, stored: Stored) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(&stored).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

fn publish(app: &AppHandle, generation: u64, update: Update) {
    let state = app.state::<AppState>();
    if state.generation.load(Ordering::SeqCst) != generation {
        return;
    }
    let status = {
        let mut status = lock(&state.status);
        status.apply(update);
        status.clone()
    };
    let _ = state.tray_status.set_text(status.tray_text());
    let _ = app.emit("status", &status);
}

fn start_service(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut service = lock(&state.service);
    if service.is_some() {
        return;
    }
    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let control = Arc::new(Control::default());
    *service = Some(Service { generation, control: control.clone() });
    let thread_app = app.clone();
    let spawned = std::thread::Builder::new().name("terra-session".into()).spawn(move || {
        let app = thread_app;
        let options = SessionOptions { address: None, scan_timeout: Duration::from_secs(10) };
        let exit = run_forever(&options, &control, |update| publish(&app, generation, update));
        if let Exit::Mismatch(error) = exit {
            publish(&app, generation, Update::Log(error.to_string()));
        }
        let state = app.state::<AppState>();
        let mut service = lock(&state.service);
        if service.as_ref().is_some_and(|s| s.generation == generation) {
            *service = None; // lets Reconnect start a fresh service after a mismatch
        }
    });
    if let Err(error) = spawned {
        *service = None;
        drop(service);
        publish(app, generation, Update::Log(format!("cannot start session: {error}")));
    }
}

fn stop_service(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(service) = lock(&state.service).take() else { return };
    service.control.stop();
    // The stopping thread may still be scanning; show the new state right away.
    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    publish(app, generation, Update::Phase(Phase::Stopped));
}

fn reconnect_now(app: &AppHandle) {
    let state = app.state::<AppState>();
    let running = lock(&state.service).as_ref().map(|s| s.control.clone());
    match running {
        Some(control) => control.reconnect(),
        None => start_service(app),
    }
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn settings(app: &AppHandle) -> Settings {
    let state = app.state::<AppState>();
    Settings {
        auto_connect: lock(&state.stored).auto_connect,
        start_with_windows: app.autolaunch().is_enabled().unwrap_or(false),
    }
}

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> Status {
    lock(&state.status).clone()
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    settings(&app)
}

#[tauri::command]
fn set_auto_connect(app: AppHandle, enabled: bool) -> Result<Settings, String> {
    {
        let state = app.state::<AppState>();
        let mut stored = lock(&state.stored);
        stored.auto_connect = enabled;
        save_settings(&state.settings_path, *stored)?;
    }
    if enabled { start_service(&app) } else { stop_service(&app) }
    Ok(settings(&app))
}

#[tauri::command]
fn set_start_with_windows(app: AppHandle, enabled: bool) -> Result<Settings, String> {
    let autolaunch = app.autolaunch();
    let result = if enabled { autolaunch.enable() } else { autolaunch.disable() };
    result.map_err(|error| format!("startup entry: {error}"))?;
    Ok(settings(&app))
}

#[tauri::command]
fn reconnect(app: AppHandle) {
    reconnect_now(&app);
}

fn main() {
    tauri::Builder::default()
        // Two companions would compete for the one Terra connection.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![MINIMIZED_ARG])))
        .invoke_handler(tauri::generate_handler![get_status, get_settings, set_auto_connect, set_start_with_windows, reconnect])
        .setup(|app| {
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let stored = load_settings(&settings_path);

            let tray_status = MenuItem::with_id(app, "status", Status::default().tray_text(), false, None::<&str>)?;
            let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let reconnect = MenuItem::with_id(app, "reconnect", "Reconnect", true, None::<&str>)?;
            let exit = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&tray_status, &separator, &open, &reconnect, &exit])?;
            let icon = app.default_window_icon().cloned().ok_or("missing application icon")?;
            TrayIconBuilder::with_id("terra")
                .icon(icon)
                .tooltip("Terra")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => show_main(app),
                    "reconnect" => reconnect_now(app),
                    "exit" => {
                        stop_service(app);
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } =
                        event
                    {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            app.manage(AppState {
                status: Mutex::new(Status::default()),
                service: Mutex::new(None),
                generation: AtomicU64::new(0),
                stored: Mutex::new(stored),
                settings_path,
                tray_status,
            });
            if !std::env::args().any(|arg| arg == MINIMIZED_ARG) {
                show_main(app.handle());
            }
            if stored.auto_connect {
                start_service(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps Terra running in the tray (spec §28).
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running the Terra companion");
}
