#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autorun;
mod state;

use chrono::Local;
use countdown_reminders::Scheduler;
use serde::Serialize;
use serde_json::Value;
use state::Store;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::NewWindowResponse,
    AppHandle, Manager, RunEvent, State, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_notification::NotificationExt;

const PRODUCT: &str = "Countdown to Death";
const MAIN: &str = "main";
const PAGE: &str = "ui-prototype.html";
const TRAY: &str = "countdown-tray";
const HEARTBEAT: Duration = Duration::from_secs(10);

struct Data {
    store: Store,
    changed: u64,
    stopping: bool,
}

struct Shared {
    data: Mutex<Data>,
    wake: Condvar,
}

#[derive(Default)]
struct WindowRequests {
    requested: AtomicBool,
    opening: AtomicBool,
}

struct Runtime {
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Runtime {
    fn start(app: &AppHandle, store: Store) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            data: Mutex::new(Data {
                store,
                changed: 0,
                stopping: false,
            }),
            wake: Condvar::new(),
        });
        let (app, worker_state) = (app.clone(), shared.clone());
        let worker = thread::Builder::new()
            .name("countdown-reminders".into())
            .spawn(move || schedule(&app, &worker_state))?;
        Ok(Self {
            shared,
            worker: Mutex::new(Some(worker)),
        })
    }

    fn stop(&self) -> bool {
        let mut data = self.shared.data.lock().unwrap();
        if data.stopping {
            return false;
        }
        data.stopping = true;
        self.shared.wake.notify_one();
        true
    }

    fn stopping(&self) -> bool {
        self.shared.data.lock().unwrap().stopping
    }

    fn join(&self) {
        self.stop();
        if let Some(worker) = self.worker.lock().unwrap().take() {
            if worker.join().is_err() {
                eprintln!("Reminder worker panicked");
            }
        }
    }
}

fn schedule(app: &AppHandle, shared: &Shared) {
    let mut scheduler = Scheduler::default();
    let mut data = shared.data.lock().unwrap();
    while !data.stopping {
        let (state, changed) = (data.store.value.clone(), data.changed);
        drop(data);
        let scan = scheduler.scan(state.as_deref().unwrap_or(&Value::Null), Local::now());
        for alert in scan.due {
            if shared.data.lock().unwrap().stopping {
                return;
            }
            if let Err(error) = app
                .notification()
                .builder()
                .title(alert.title)
                .body(alert.body)
                .show()
            {
                eprintln!("Could not show reminder: {error}");
            }
        }
        // Bound the sleep so wall-clock changes and resume are noticed even if the
        // next deadline is far away. Saves and shutdown wake the same worker early.
        let delay = scan.next_at.map_or(HEARTBEAT, |at| {
            (at - Local::now())
                .to_std()
                .unwrap_or_default()
                .min(HEARTBEAT)
        });
        data = shared
            .wake
            .wait_timeout_while(shared.data.lock().unwrap(), delay, |data| {
                !data.stopping && data.changed == changed
            })
            .unwrap()
            .0;
    }
}

#[tauri::command]
fn get_state(runtime: State<'_, Runtime>) -> Option<Value> {
    runtime
        .shared
        .data
        .lock()
        .unwrap()
        .store
        .value
        .as_deref()
        .cloned()
}

#[tauri::command]
fn save_state(app: AppHandle, runtime: State<'_, Runtime>, state: Value) -> Result<(), String> {
    let mut data = runtime.shared.data.lock().unwrap();
    if data.stopping {
        return Err("Countdown to Death is quitting".into());
    }
    let prior = data.store.autorun();
    data.store.save(state)?;
    data.changed = data.changed.wrapping_add(1);
    runtime.shared.wake.notify_one();
    if let Some(enabled) = data
        .store
        .autorun()
        .filter(|enabled| Some(*enabled) != prior)
    {
        update_autorun(&app, enabled);
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    notifications: bool,
    exact_alarms: bool,
}

#[tauri::command]
fn get_status(app: AppHandle) -> Status {
    Status {
        notifications: matches!(
            app.notification().permission_state(),
            Ok(tauri_plugin_notification::PermissionState::Granted)
        ),
        exact_alarms: false,
    }
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    quit(&app);
}

fn quit(app: &AppHandle) {
    if app.state::<Runtime>().stop() {
        app.exit(0);
    }
}

fn update_autorun(app: &AppHandle, enabled: bool) {
    if let Err(error) = autorun::update(app, enabled) {
        eprintln!("Could not update launch at login: {error}");
    }
}

fn allowed_navigation(url: &Url) -> bool {
    // No dev server is configured. Windows uses WebView2's HTTP-backed app origin.
    let origin = if cfg!(windows) {
        "http://tauri.localhost/"
    } else {
        "tauri://localhost/"
    };
    let mut document = url.clone();
    document.set_fragment(None);
    document.as_str() == format!("{origin}{PAGE}")
}

fn show_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN) {
        window.show()?;
        window.unminimize()?;
        return window.set_focus();
    }
    WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App(PAGE.into()))
        .title(PRODUCT)
        .inner_size(1160.0, 820.0)
        .min_inner_size(480.0, 450.0)
        .background_color(tauri::window::Color(22, 24, 21, 255))
        .on_navigation(allowed_navigation)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .build()?;
    Ok(())
}

fn request_show(app: &AppHandle) {
    let requests = app.state::<WindowRequests>();
    requests.requested.store(true, Ordering::Release);
    let Some(runtime) = app.try_state::<Runtime>() else {
        return;
    };
    if runtime.stopping() || requests.opening.swap(true, Ordering::AcqRel) {
        return;
    }
    let handle = app.clone();
    // Tauri dispatches window creation to the UI thread itself. Creating it from
    // inside a synchronous WebView2/native event handler can deadlock on Windows.
    tauri::async_runtime::spawn_blocking(move || {
        let requests = handle.state::<WindowRequests>();
        requests.requested.store(false, Ordering::Release);
        if !handle.state::<Runtime>().stopping() {
            if let Err(error) = show_window(&handle) {
                eprintln!("Could not show countdown window: {error}");
                if handle.tray_by_id(TRAY).is_none() {
                    quit(&handle);
                }
            }
        }
        requests.opening.store(false, Ordering::Release);
        if requests.requested.load(Ordering::Acquire) {
            request_show(&handle);
        }
    });
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit_item])?;
    let mut tray = TrayIconBuilder::with_id(TRAY)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip(PRODUCT)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => request_show(app),
            "quit" => quit(app),
            _ => (),
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                request_show(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn hidden(args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>) -> bool {
    args.into_iter()
        .skip(1)
        .any(|arg| arg.as_ref() == autorun::LOGIN_ARG)
}

fn main() {
    let app = tauri::Builder::default()
        .manage(WindowRequests::default())
        // Acquire the instance before initializing any state, tray or worker.
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !hidden(args) {
                request_show(app);
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            get_state, save_state, get_status, quit_app
        ])
        .setup(|app| {
            let handle = app.handle();
            let paths = app.path();
            let legacy = paths
                .config_dir()
                .map(|config| {
                    [
                        config.join("Countdown to Death/state.json"),
                        config.join("countdown to death/state.json"),
                    ]
                })
                .map_err(|error| error.to_string());
            let store = Store::load(paths.app_config_dir()?.join("state.json"), legacy);
            if let Some(enabled) = store.autorun() {
                update_autorun(handle, enabled);
            }
            app.manage(Runtime::start(handle, store)?);

            let tray_ready = match build_tray(handle) {
                Ok(()) => true,
                Err(error) => {
                    eprintln!("System tray unavailable; closing the window will quit: {error}");
                    false
                }
            };
            if !tray_ready
                || !hidden(std::env::args_os())
                || handle
                    .state::<WindowRequests>()
                    .requested
                    .load(Ordering::Acquire)
            {
                request_show(handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // With a tray, let native close destroy the window and its WebView.
            if window.label() == MAIN
                && matches!(event, WindowEvent::CloseRequested { .. })
                && window.app_handle().tray_by_id(TRAY).is_none()
            {
                quit(window.app_handle());
            }
        })
        .build(tauri::generate_context!())
        .expect("could not initialize Countdown to Death");

    app.run(|app, event| match event {
        RunEvent::ExitRequested { api, .. } => {
            // Tauri requests exit when the last window is actually destroyed.
            if !app
                .try_state::<Runtime>()
                .is_some_and(|runtime| runtime.stopping())
            {
                api.prevent_exit();
            }
        }
        RunEvent::Exit => {
            if let Some(runtime) = app.try_state::<Runtime>() {
                runtime.join();
            }
        }
        _ => (),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_embedded_document_and_its_fragments_can_navigate() {
        let origin = if cfg!(windows) {
            "http://tauri.localhost/"
        } else {
            "tauri://localhost/"
        };
        for suffix in [PAGE, "ui-prototype.html#settings"] {
            assert!(allowed_navigation(
                &Url::parse(&format!("{origin}{suffix}")).unwrap()
            ));
        }
        for url in [
            format!("{origin}ui-prototype.html?redirect=true"),
            format!("{origin}other.html"),
            "https://example.com/ui-prototype.html".into(),
            "file:///ui-prototype.html".into(),
        ] {
            assert!(!allowed_navigation(&Url::parse(&url).unwrap()));
        }
    }

    #[test]
    fn hidden_launch_requires_an_exact_argument() {
        assert!(hidden(["app", "--hidden"]));
        assert!(!hidden(["app", "--hidden=false"]));
        assert!(!hidden(["app"]));
    }
}
