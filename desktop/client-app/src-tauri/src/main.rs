// NB Sons HR — client app for the office PCs.
// Connects to the one NB Sons HR Server on the network and shows the HR system in its own window.
// Stores no HR data: everything is read from and saved to the server.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Url};

const DEFAULT_PORT: u16 = 5000;

#[derive(Default)]
struct State {
    connected: AtomicBool,          // true while the HR pages from the server are showing
    server: Mutex<Option<String>>,  // e.g. http://192.168.1.20:5000
    home: Mutex<Option<Url>>,       // the built-in setup page
}

#[derive(Serialize, Deserialize, Default)]
struct Config {
    server: Option<String>,
}

#[derive(Serialize, Clone)]
struct ServerInfo {
    url: String,
    company: String,
    version: String,
}

fn config_path(app: &AppHandle) -> PathBuf {
    app.path().app_config_dir().unwrap_or_else(|_| PathBuf::from(".")).join("config.json")
}

fn load_config(app: &AppHandle) -> Config {
    std::fs::read_to_string(config_path(app))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(app: &AppHandle, cfg: &Config) {
    let p = config_path(app);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, serde_json::to_string_pretty(cfg).unwrap_or_default());
}

/// "192.168.1.20", "192.168.1.20:5000", "http://office-pc:5000/login" -> (host, port)
fn parse_addr(input: &str) -> Result<(String, u16), String> {
    let mut s = input.trim().to_string();
    for p in ["http://", "https://"] {
        if s.to_lowercase().starts_with(p) {
            s = s[p.len()..].to_string();
        }
    }
    let s = s.split('/').next().unwrap_or("").trim().to_string();
    if s.is_empty() {
        return Err("Please type the server address, for example 192.168.1.20".into());
    }
    match s.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() => {
            let port = p.parse::<u16>().map_err(|_| format!("'{p}' is not a valid port number"))?;
            Ok((h.to_string(), port))
        }
        _ => Ok((s, DEFAULT_PORT)),
    }
}

/// Ask a host for /api/ping. Ok(info) only if it is an NB Sons HR server.
fn ping(host: &str, port: u16, timeout: Duration) -> Result<ServerInfo, String> {
    let addr: SocketAddr = (host, port)
        .to_socket_addrs()
        .map_err(|_| format!("Can't find a computer called '{host}' on the network"))?
        .find(|a| a.is_ipv4())
        .ok_or_else(|| format!("Can't find a computer called '{host}' on the network"))?;
    let mut s = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|_| format!("No answer from {host}:{port}. Is the server PC switched on and NB Sons HR Server running?"))?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = s.set_write_timeout(Some(Duration::from_secs(3)));
    s.write_all(format!("GET /api/ping HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes())
        .map_err(|e| e.to_string())?;
    let mut buf = String::new();
    let _ = s.read_to_string(&mut buf);
    let body = buf.split("\r\n\r\n").nth(1).unwrap_or("");
    let v: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| format!("{host}:{port} answered, but it is not the NB Sons HR server"))?;
    if v.get("app").and_then(|a| a.as_str()) != Some("nbsons-hr") {
        return Err(format!("{host}:{port} answered, but it is not the NB Sons HR server"));
    }
    Ok(ServerInfo {
        url: format!("http://{host}:{port}"),
        company: v.get("company").and_then(|c| c.as_str()).unwrap_or("").to_string(),
        version: v.get("version").and_then(|c| c.as_str()).unwrap_or("").to_string(),
    })
}

fn local_ipv4() -> Option<Ipv4Addr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("10.255.255.255:1").ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() => Some(ip),
        _ => None,
    }
}

/// Look for NB Sons HR servers on this PC's network (x.x.x.1 – x.x.x.254, port 5000).
fn scan() -> Vec<ServerInfo> {
    let Some(me) = local_ipv4() else { return vec![] };
    let [a, b, c, _] = me.octets();
    let (tx, rx) = mpsc::channel();
    let mut handles = vec![];
    for chunk in (1u8..=254).collect::<Vec<_>>().chunks(16) {
        let chunk = chunk.to_vec();
        let tx = tx.clone();
        handles.push(std::thread::spawn(move || {
            for d in chunk {
                let host = format!("{a}.{b}.{c}.{d}");
                if let Ok(info) = ping(&host, DEFAULT_PORT, Duration::from_millis(250)) {
                    let _ = tx.send(info);
                }
            }
        }));
    }
    drop(tx);
    for h in handles {
        let _ = h.join();
    }
    let mut found: Vec<ServerInfo> = rx.into_iter().collect();
    // also check this PC itself (useful when the server runs here too)
    if let Ok(info) = ping("127.0.0.1", DEFAULT_PORT, Duration::from_millis(250)) {
        if found.is_empty() {
            found.push(info);
        }
    }
    found
}

fn go_home(app: &AppHandle, query: &str) {
    let state = app.state::<Arc<State>>();
    state.connected.store(false, Ordering::SeqCst);
    let home = state.home.lock().unwrap().clone();
    if let (Some(mut url), Some(w)) = (home, app.get_webview_window("main")) {
        url.set_query(if query.is_empty() { None } else { Some(query) });
        let _ = w.navigate(url);
    }
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
fn get_saved(app: AppHandle) -> Option<String> {
    load_config(&app).server
}

#[tauri::command]
async fn connect(app: AppHandle, addr: String) -> Result<ServerInfo, String> {
    let (host, port) = parse_addr(&addr)?;
    let info = tauri::async_runtime::spawn_blocking(move || ping(&host, port, Duration::from_secs(3)))
        .await
        .map_err(|e| e.to_string())??;
    save_config(&app, &Config { server: Some(info.url.clone()) });
    let state = app.state::<Arc<State>>();
    *state.server.lock().unwrap() = Some(info.url.clone());
    state.connected.store(true, Ordering::SeqCst);
    if let Some(w) = app.get_webview_window("main") {
        let url: Url = format!("{}/", info.url).parse().map_err(|e: <Url as std::str::FromStr>::Err| e.to_string())?;
        w.navigate(url).map_err(|e| e.to_string())?;
    }
    Ok(info)
}

#[tauri::command]
async fn find_servers() -> Vec<ServerInfo> {
    tauri::async_runtime::spawn_blocking(scan).await.unwrap_or_default()
}

/// Every 10 s check the server; if it stops answering twice in a row, show the
/// "can't reach the server" page (which reconnects by itself when the server is back).
fn start_monitor(app: AppHandle) {
    std::thread::spawn(move || {
        let mut fails = 0;
        loop {
            std::thread::sleep(Duration::from_secs(10));
            let state = app.state::<Arc<State>>();
            if !state.connected.load(Ordering::SeqCst) {
                fails = 0;
                continue;
            }
            let server = state.server.lock().unwrap().clone();
            let Some(server) = server else { continue };
            let ok = parse_addr(&server)
                .map(|(h, p)| ping(&h, p, Duration::from_secs(3)).is_ok())
                .unwrap_or(false);
            fails = if ok { 0 } else { fails + 1 };
            if fails >= 2 {
                fails = 0;
                go_home(&app, "offline=1");
            }
        }
    });
}

fn main() {
    let state = Arc::new(State::default());
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .manage(state)
        .menu(|app| {
            let reload = MenuItem::with_id(app, "reload", "Reload page", true, Some("F5"))?;
            let change = MenuItem::with_id(app, "change", "Change server…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let sub = Submenu::with_items(app, "HR System", true, &[&reload, &change, &sep, &quit])?;
            Menu::with_items(app, &[&sub])
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "reload" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.eval("location.reload()");
                }
            }
            "change" => go_home(app, "change=1"),
            "quit" => app.exit(0),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![get_saved, connect, find_servers, notify])
        .setup(|app| {
            let handle = app.handle().clone();
            if let Some(w) = handle.get_webview_window("main") {
                if let Ok(url) = w.url() {
                    *handle.state::<Arc<State>>().home.lock().unwrap() = Some(url);
                }
            }
            start_monitor(handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running NB Sons HR");
}

#[cfg(test)]
mod tests {
    use super::parse_addr;
    #[test]
    fn addresses() {
        assert_eq!(parse_addr("192.168.1.20").unwrap(), ("192.168.1.20".into(), 5000));
        assert_eq!(parse_addr(" http://192.168.1.20:5000/login ").unwrap(), ("192.168.1.20".into(), 5000));
        assert_eq!(parse_addr("office-pc:8080").unwrap(), ("office-pc".into(), 8080));
        assert!(parse_addr("").is_err());
        assert!(parse_addr("1.2.3.4:abc").is_err());
    }
}

#[cfg(test)]
mod live {
    #[test]
    #[ignore] // needs a running NB Sons HR server on this network
    fn scan_finds_server() {
        let found = super::scan();
        println!("found: {:?}", found.iter().map(|s| (&s.url, &s.company)).collect::<Vec<_>>());
        assert!(!found.is_empty());
    }
}
