//! fastDev desktop shell: window, menu bar icon, lifecycle and the bridge between
//! the webview and `fastdev_core::Core`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use fastdev_core::control::ControlServer;
use fastdev_core::events::{Event, EventSink};
use fastdev_core::settings::Theme as ThemeSetting;
use fastdev_core::{Core, CoreConfig};
use fastdev_protocol::Caller;
use serde::Serialize;
use serde_json::Value;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, RunEvent, Theme, WebviewWindow, WindowEvent};
use tauri_plugin_liquid_glass::{GlassMaterialVariant, LiquidGlassConfig, LiquidGlassExt};

const EVENT: &str = "fastdev://event";
const QUIT_REQUESTED: &str = "fastdev://quit-requested";

struct TauriSink(AppHandle);

impl EventSink for TauriSink {
    fn emit(&self, event: Event) {
        let _ = self.0.emit(EVENT, &event);
    }
}

struct AppState {
    core: Arc<Core>,
    control: Mutex<Option<ControlServer>>,
    control_error: Mutex<Option<String>>,
    quitting: AtomicBool,
    tray_items: Mutex<Option<(MenuItem<tauri::Wry>, MenuItem<tauri::Wry>)>>,
}

#[derive(Serialize)]
struct CommandError {
    code: String,
    message: String,
}

impl From<fastdev_core::Error> for CommandError {
    fn from(err: fastdev_core::Error) -> Self {
        Self { code: err.code.as_str().to_string(), message: err.message }
    }
}

/// The one command the frontend uses for the whole API (see `fastdev_core::api`).
#[tauri::command]
async fn call(state: tauri::State<'_, AppState>, method: String, args: Option<Value>) -> Result<Value, CommandError> {
    let core = state.core.clone();
    tauri::async_runtime::spawn_blocking(move || core.call(&method, args.unwrap_or(Value::Null), Caller::Ui))
        .await
        .map_err(|err| CommandError { code: "internal".into(), message: err.to_string() })?
        .map_err(CommandError::from)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShellInfo {
    glass_supported: bool,
    control_socket: Option<String>,
    control_error: Option<String>,
    running_processes: usize,
}

#[tauri::command]
fn shell_info(app: AppHandle, state: tauri::State<'_, AppState>) -> ShellInfo {
    ShellInfo {
        glass_supported: app.liquid_glass().is_supported(),
        control_socket: state
            .control
            .lock()
            .expect("control")
            .as_ref()
            .map(|c| c.path().to_string_lossy().into_owned()),
        control_error: state.control_error.lock().expect("control").clone(),
        running_processes: state.core.running_runs().len(),
    }
}

/// Applies the theme to the native window so vibrancy and glass match the UI.
#[tauri::command]
fn set_window_theme(window: WebviewWindow, theme: String) {
    let theme = match theme.as_str() {
        "light" => Some(Theme::Light),
        "dark" => Some(Theme::Dark),
        _ => None,
    };
    let _ = window.set_theme(theme);
}

#[tauri::command]
fn set_tray_labels(state: tauri::State<'_, AppState>, open: String, quit: String) {
    if let Some((open_item, quit_item)) = state.tray_items.lock().expect("tray").as_ref() {
        let _ = open_item.set_text(open);
        let _ = quit_item.set_text(quit);
    }
}

/// Quits after the user confirmed in the UI; stops all running project processes.
#[tauri::command]
fn quit_app(app: AppHandle, state: tauri::State<'_, AppState>) {
    state.quitting.store(true, Ordering::SeqCst);
    let core = state.core.clone();
    std::thread::spawn(move || {
        core.stop_all_runs();
        app.exit(0);
    });
}

/// Quits, or asks the UI to confirm first when project processes are running.
fn request_quit(app: &AppHandle) {
    let state = app.state::<AppState>();
    let running = state.core.running_runs().len();
    if running > 0 && !state.quitting.load(Ordering::SeqCst) {
        show_main_window(app);
        let _ = app.emit(QUIT_REQUESTED, running);
    } else {
        state.quitting.store(true, Ordering::SeqCst);
        state.core.stop_all_runs();
        app.exit(0);
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn bridge_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let path = exe.parent()?.join("fastdev-mcp");
    Some(path)
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let config = CoreConfig {
        data_dir: fastdev_protocol::data_dir(),
        app_version: app.package_info().version.to_string(),
        bridge_path: bridge_path(),
        workspace_override: None,
        registries_override: None,
    };
    let core = Core::new(config, Arc::new(TauriSink(handle.clone()))).map_err(|err| err.message)?;
    core.watch_library();
    core.sync_in_background();

    let (control, control_error) = match ControlServer::start(core.clone(), &fastdev_protocol::socket_path()) {
        Ok(server) => (Some(server), None),
        Err(err) => {
            log::error!("control socket: {}", err.message);
            (None, Some(err.message))
        }
    };

    // Menu bar icon.
    let open_item = MenuItem::with_id(app, "open", "Open fastDev", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit fastDev", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open_item, &separator, &quit_item])?;
    let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray@2x.png"))?;
    TrayIconBuilder::with_id("main")
        .icon(tray_icon)
        .icon_as_template(true)
        .tooltip("fastDev")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "quit" => request_quit(app),
            _ => {}
        })
        .build(app)?;

    // Native glass behind the webview and the theme from settings.
    if let Some(window) = app.get_webview_window("main") {
        let glass =
            LiquidGlassConfig { variant: GlassMaterialVariant::Sidebar, corner_radius: 0.0, ..Default::default() };
        if let Err(err) = app.liquid_glass().set_effect(&window, glass) {
            log::warn!("liquid glass: {err}");
        }
        let theme = match core.settings().theme {
            ThemeSetting::Light => Some(Theme::Light),
            ThemeSetting::Dark => Some(Theme::Dark),
            ThemeSetting::System => None,
        };
        let _ = window.set_theme(theme);
    }

    app.manage(AppState {
        core,
        control: Mutex::new(control),
        control_error: Mutex::new(control_error),
        quitting: AtomicBool::new(false),
        tray_items: Mutex::new(Some((open_item, quit_item))),
    });
    Ok(())
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main_window(app)))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_liquid_glass::init())
        .setup(setup)
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
            {
                let state = window.state::<AppState>();
                if state.core.settings().keep_in_menu_bar && !state.quitting.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![call, shell_info, set_window_theme, set_tray_labels, quit_app])
        .build(tauri::generate_context!())
        .expect("error while building fastDev");

    app.run(|app, event| match event {
        RunEvent::ExitRequested { api, .. } => {
            let state = app.state::<AppState>();
            if !state.quitting.load(Ordering::SeqCst) && !state.core.running_runs().is_empty() {
                api.prevent_exit();
                show_main_window(app);
                let _ = app.emit(QUIT_REQUESTED, state.core.running_runs().len());
            }
        }
        RunEvent::Exit => {
            let state = app.state::<AppState>();
            state.core.stop_all_runs();
            state.control.lock().expect("control").take();
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => show_main_window(app),
        _ => {}
    });
}
