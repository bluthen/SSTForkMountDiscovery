use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

const SERVICE_TYPE: &str = "_sstmount._tcp.local.";

/// Starts the mDNS discovery daemon in a background thread.
/// Discovered service IPs are added to the shared device list and
/// events are emitted to the frontend.
pub fn start_mdns_discovery(app: AppHandle, discovered_ips: Arc<Mutex<Vec<String>>>) {
    thread::spawn(move || {
        let mdns = match ServiceDaemon::new() {
            Ok(d) => d,
            Err(e) => {
                log::error!("Failed to create mDNS daemon: {}", e);
                return;
            }
        };

        let receiver = match mdns.browse(SERVICE_TYPE) {
            Ok(r) => r,
            Err(e) => {
                log::error!("Failed to browse mDNS services: {}", e);
                return;
            }
        };

        log::info!("mDNS discovery started for {}", SERVICE_TYPE);

        loop {
            match receiver.recv() {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    let addresses = info.get_addresses();
                    for addr in addresses {
                        let ip = addr.to_string();
                        log::info!("mDNS resolved service at {}", ip);

                        let mut ips = discovered_ips.lock().unwrap_or_else(|e| e.into_inner());
                        if !ips.contains(&ip) {
                            ips.push(ip.clone());
                            drop(ips);

                            // Trigger a device probe for this IP
                            let _ = app.emit("mdns-discovered", ip);
                        }
                    }
                }
                Ok(ServiceEvent::ServiceRemoved(_type, fullname)) => {
                    log::info!("mDNS service removed: {}", fullname);
                }
                Ok(_) => {
                    // SearchStarted, ServiceFound (before resolution), etc.
                }
                Err(e) => {
                    log::error!("mDNS receiver error: {}", e);
                    break;
                }
            }
        }
    });
}
