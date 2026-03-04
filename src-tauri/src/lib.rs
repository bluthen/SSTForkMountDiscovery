mod discovery;
mod prober;
mod scanner;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Listener, Manager, State};
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceInfo {
    pub hostname: String,
    pub ip: String,
    #[serde(rename = "deviceType")]
    pub device_type: String,
}

/// Application state shared across commands.
pub struct AppState {
    /// All IPs discovered through any method (mDNS, scan, known, previous).
    pub discovered_ips: Arc<Mutex<Vec<String>>>,
    /// The last set of probed/verified devices.
    pub devices: Arc<Mutex<Vec<DeviceInfo>>>,
    /// Path to the storage file for previously connected IPs.
    pub storage_path: Mutex<PathBuf>,
}

/// Get the path to the connected IPs storage file.
fn get_storage_path(app: &AppHandle) -> PathBuf {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    fs::create_dir_all(&data_dir).ok();
    data_dir.join("connected.json")
}

/// Load previously connected IPs from storage.
fn load_previous_connections(path: &PathBuf) -> Vec<String> {
    if let Ok(data) = fs::read_to_string(path) {
        if let Ok(ips) = serde_json::from_str::<Vec<String>>(&data) {
            return ips;
        }
    }
    Vec::new()
}

/// Save connected IPs to storage.
fn save_connections(path: &PathBuf, ips: &[String]) {
    if let Ok(json) = serde_json::to_string(ips) {
        let _ = fs::write(path, json);
    }
}

/// Tauri command: Start the discovery process (mDNS + network scan).
/// Called once when the frontend mounts.
#[tauri::command]
async fn start_discovery(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let ips = state.discovered_ips.clone();

    // Load previous connections and add to discovered IPs
    {
        let storage_path = state.storage_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let previous = load_previous_connections(&storage_path);
        let mut discovered = ips.lock().unwrap_or_else(|e| e.into_inner());
        for ip in previous {
            if !discovered.contains(&ip) {
                discovered.push(ip);
            }
        }
    }

    // Start mDNS discovery in background
    discovery::start_mdns_discovery(app.clone(), ips.clone());

    // Start network scan loop in background
    let rt = tauri::async_runtime::handle();
    scanner::start_scan_loop(app.clone(), ips.clone(), rt);

    // Do an initial probe of known + previous IPs
    let initial_ips: Vec<String> = {
        let discovered = ips.lock().unwrap_or_else(|e| e.into_inner());
        discovered.clone()
    };

    if !initial_ips.is_empty() {
        let devices = prober::probe_all_ips(&initial_ips).await;
        let mut dev_list = state.devices.lock().unwrap_or_else(|e| e.into_inner());
        *dev_list = devices.clone();
        let _ = app.emit("devices-updated", devices);
    }

    Ok(())
}

/// Tauri command: Get the current list of discovered devices.
#[tauri::command]
async fn get_devices(state: State<'_, AppState>) -> Result<Vec<DeviceInfo>, String> {
    let devices = state.devices.lock().unwrap_or_else(|e| e.into_inner()).clone();
    Ok(devices)
}

/// Tauri command: Refresh the device list by probing all known IPs.
#[tauri::command]
async fn refresh_devices(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<DeviceInfo>, String> {
    let ips: Vec<String> = {
        let discovered = state.discovered_ips.lock().unwrap_or_else(|e| e.into_inner());
        discovered.clone()
    };

    let devices = prober::probe_all_ips(&ips).await;
    {
        let mut dev_list = state.devices.lock().unwrap_or_else(|e| e.into_inner());
        *dev_list = devices.clone();
    }
    let _ = app.emit("devices-updated", devices.clone());
    Ok(devices)
}

/// Tauri command: Check a specific IP (for manual connect).
#[tauri::command]
async fn check_ip(ip: String) -> Result<DeviceInfo, String> {
    match prober::probe_device(&ip).await {
        Some(device) => Ok(device),
        None => Err(format!("Unable to connect to {}", ip)),
    }
}

/// Tauri command: Save an IP as a previously connected device and open in browser.
#[tauri::command]
async fn connect_to_device(
    app: AppHandle,
    state: State<'_, AppState>,
    ip: String,
) -> Result<(), String> {
    // Save to persistent storage
    let storage_path = state.storage_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut connections = load_previous_connections(&storage_path);
    if !connections.contains(&ip) {
        connections.push(ip.clone());
    }
    save_connections(&storage_path, &connections);

    // Also add to discovered IPs
    {
        let mut discovered = state.discovered_ips.lock().unwrap_or_else(|e| e.into_inner());
        if !discovered.contains(&ip) {
            discovered.push(ip.clone());
        }
    }

    // Open in system browser
    let url = format!("http://{}:5000", ip);
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| format!("Failed to open browser: {}", e))?;

    Ok(())
}

/// Tauri command: Get previously connected IPs.
#[tauri::command]
async fn get_previous_connections(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let storage_path = state.storage_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
    Ok(load_previous_connections(&storage_path))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let storage_path = get_storage_path(&app.handle());

            app.manage(AppState {
                discovered_ips: Arc::new(Mutex::new(Vec::new())),
                devices: Arc::new(Mutex::new(Vec::new())),
                storage_path: Mutex::new(storage_path),
            });

            // Get the Tauri-managed async runtime handle so we can spawn
            // async tasks from non-Tokio threads (event listener callbacks
            // run on Tauri's event thread, which has no Tokio runtime context).
            let rt = tauri::async_runtime::handle();

            // Listen for mDNS discovery events and trigger device probing
            let handle = app.handle().clone();
            let rt1 = rt.clone();
            handle.clone().listen("mdns-discovered", move |event: tauri::Event| {
                let app = handle.clone();
                let ip_str = event.payload().to_string();
                // Remove quotes from the JSON string
                let ip = ip_str.trim_matches('"').to_string();

                rt1.spawn(async move {
                    if let Some(device) = prober::probe_device(&ip).await {
                        let state = app.state::<AppState>();
                        let mut devices = state.devices.lock().unwrap_or_else(|e| e.into_inner());
                        if !devices.iter().any(|d| d.ip == device.ip) {
                            devices.push(device.clone());
                            let all_devices = devices.clone();
                            drop(devices);
                            let _ = app.emit("devices-updated", all_devices);
                        }
                    }
                });
            });

            // Listen for scan-found IPs and trigger probing
            let handle2 = app.handle().clone();
            let rt2 = rt.clone();
            handle2.clone().listen("ips-updated", move |_event: tauri::Event| {
                let app = handle2.clone();
                rt2.spawn(async move {
                    let state = app.state::<AppState>();
                    let ips: Vec<String> = {
                        let discovered = state.discovered_ips.lock().unwrap_or_else(|e| e.into_inner());
                        discovered.clone()
                    };
                    let devices = prober::probe_all_ips(&ips).await;
                    {
                        let mut dev_list = state.devices.lock().unwrap_or_else(|e| e.into_inner());
                        *dev_list = devices.clone();
                    }
                    let _ = app.emit("devices-updated", devices);
                });
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_discovery,
            get_devices,
            refresh_devices,
            check_ip,
            connect_to_device,
            get_previous_connections,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
