// NB Sons HR Server — desktop app for the main (server) PC.
// Starts the built-in HR server (hr-server.exe), shows it in a window, and keeps
// running in the system tray so other PCs and phones can always connect.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_shell::process::CommandChild;
use tauri_plugin_shell::ShellExt;

const PORT: u16 = 5000;

struct ServerChild(Mutex<Option<CommandChild>>);

/// Folder that holds hr.db, backups and the secret key: Documents\NB Sons HR Data
fn data_dir(app: &AppHandle) -> PathBuf {
    let base = app
        .path()
        .document_dir()
        .or_else(|_| app.path().home_dir())
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("NB Sons HR Data")
}

/// This PC's address on the office network (used to show "connect other PCs to ...").
fn lan_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("10.255.255.255:1")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

/// True when an NB Sons HR server answers on 127.0.0.1:PORT.
fn server_ready() -> bool {
    let addr: SocketAddr = ([127, 0, 0, 1], PORT).into();
    let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else { return false };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = s.write_all(b"GET /api/ping HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n");
    let mut buf = String::new();
    let _ = s.read_to_string(&mut buf);
    buf.contains("nbsons-hr")
}

fn port_taken() -> bool {
    let addr: SocketAddr = ([127, 0, 0, 1], PORT).into();
    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn splash_error(app: &AppHandle, msg: &str) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(&format!("window.showError && showError({})", serde_json::json!(msg)));
        let _ = w.show();
    }
}

fn open_folder(path: &PathBuf) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(not(target_os = "windows"))]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

fn stop_server(app: &AppHandle) {
    if let Some(state) = app.try_state::<ServerChild>() {
        if let Some(child) = state.0.lock().unwrap().take() {
            let _ = child.kill();
        }
    }
}

/// Start hr-server.exe (unless one is already running) and open the window when it answers.
fn start_server(app: &AppHandle) {
    let data = data_dir(app);
    let _ = std::fs::create_dir_all(&data);

    if !server_ready() {
        if port_taken() {
            splash_error(app, &format!(
                "Port {PORT} is already used by another program on this PC.\n\
                 If the old 'Start Server' window is open, close it and restart NB Sons HR Server."));
            return;
        }
        match app.shell().sidecar("hr-server") {
            Ok(cmd) => {
                let cmd = cmd
                    .env("HR_DATA", data.to_string_lossy().to_string())
                    .env("HR_PORT", PORT.to_string())
                    .env("HR_PARENT_PID", std::process::id().to_string());
                match cmd.spawn() {
                    Ok((_rx, child)) => {
                        *app.state::<ServerChild>().0.lock().unwrap() = Some(child);
                    }
                    Err(e) => {
                        splash_error(app, &format!("Could not start the HR server: {e}"));
                        return;
                    }
                }
            }
            Err(e) => {
                splash_error(app, &format!("HR server program is missing: {e}"));
                return;
            }
        }
    }

    let handle = app.clone();
    std::thread::spawn(move || {
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(60) {
            if server_ready() {
                if let Some(w) = handle.get_webview_window("main") {
                    let url = format!("http://127.0.0.1:{PORT}/").parse().unwrap();
                    let _ = w.navigate(url);
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(400));
        }
        splash_error(&handle, "The HR server did not start within 60 seconds. \
            Open the data folder from the tray icon and send the file server.log to your IT person.");
    });
}

/// Show a Windows notification (called by the HR pages from the server).
#[tauri::command]
fn notify(app: AppHandle, title: String, body: String, important: bool, sound: bool, loud: bool) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    let title: String = title.chars().take(120).collect();
    let body: String = body.chars().take(400).collect();
    let mut n = app.notification().builder().title(title).body(body);
    if sound {
        // normal alerts: the usual Windows "ding"; important alerts: a distinct alarm tone
        n = n.sound(if important && loud { "Alarm2" } else { "Default" });
    }
    n.show().map_err(|e| e.to_string())
}

#[tauri::command]
fn retry_start(app: AppHandle) {
    start_server(&app);
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let ip = lan_ip();
    let addr = MenuItem::with_id(app, "addr", format!("Other PCs connect to:  {ip}"), false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open HR System", true, None::<&str>)?;
    let folder = MenuItem::with_id(app, "folder", "Open data folder (database & backups)", true, None::<&str>)?;
    let auto_on = app.autolaunch().is_enabled().unwrap_or(false);
    let auto = CheckMenuItem::with_id(app, "autostart", "Start automatically with Windows", true, auto_on, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Stop server and quit", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&addr, &sep1, &open, &folder, &auto, &sep2, &quit])?;

    let auto_item = auto.clone();
    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip(format!("NB Sons HR Server is running — other PCs: http://{ip}:{PORT}"))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "folder" => open_folder(&data_dir(app)),
            "autostart" => {
                let mgr = app.autolaunch();
                let now = mgr.is_enabled().unwrap_or(false);
                let _ = if now { mgr.disable() } else { mgr.enable() };
                let _ = auto_item.set_checked(mgr.is_enabled().unwrap_or(!now));
            }
            "quit" => {
                stop_server(app);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .manage(ServerChild(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![retry_start, notify])
        .setup(|app| {
            let handle = app.handle().clone();
            // First run: start with Windows by default (the server should always be on).
            let marker = data_dir(&handle).join(".autostart-set");
            if !marker.exists() {
                let _ = std::fs::create_dir_all(data_dir(&handle));
                let _ = handle.autolaunch().enable();
                let _ = std::fs::write(&marker, "1");
            }
            build_tray(&handle)?;
            let minimized = std::env::args().any(|a| a == "--minimized");
            if !minimized {
                show_main(&handle);
            }
            start_server(&handle);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the server running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building NB Sons HR Server");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            stop_server(app);
        }
    });
}
