use ipnetwork::IpNetwork;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::net::TcpStream;
use tokio::time::timeout;

const PORT: u16 = 5000;
const CONNECT_TIMEOUT_MS: u64 = 40;
const KNOWN_IPS: &[&str] = &["192.168.45.1", "192.168.46.2"];
const SCAN_INTERVAL_SECS: u64 = 180;
const MAX_CONCURRENT_SCANS: usize = 200;

/// Check if a TCP port is open on the given IP address.
async fn is_port_open(ip: Ipv4Addr, port: u16) -> bool {
    let addr = SocketAddr::new(IpAddr::V4(ip), port);
    match timeout(
        Duration::from_millis(CONNECT_TIMEOUT_MS),
        TcpStream::connect(addr),
    )
    .await
    {
        Ok(Ok(_stream)) => true,
        _ => false,
    }
}

/// Get all local IPv4 subnets (excluding loopback).
fn get_local_subnets() -> Vec<IpNetwork> {
    let mut subnets = Vec::new();
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces {
            if iface.is_loopback() {
                continue;
            }
            match iface.addr {
                if_addrs::IfAddr::V4(ref v4) => {
                    let ip = v4.ip;
                    let netmask = v4.netmask;

                    // Calculate prefix length from netmask
                    let mask_bits: u32 = u32::from(netmask);
                    let prefix = mask_bits.count_ones() as u8;

                    if let Ok(network) =
                        IpNetwork::new(IpAddr::V4(ip), prefix)
                    {
                        subnets.push(network);
                    }
                }
                _ => {} // Skip IPv6
            }
        }
    }
    subnets
}

/// Enumerate all IPs in a subnet (excluding network and broadcast addresses).
fn enumerate_subnet_ips(network: &IpNetwork) -> Vec<Ipv4Addr> {
    let mut ips = Vec::new();

    // Skip subnets >= /16 (too large)
    let size: u128 = match network.size() {
        ipnetwork::NetworkSize::V4(s) => s as u128,
        ipnetwork::NetworkSize::V6(s) => s,
    };
    if size >= 65534 {
        log::warn!(
            "Skipping large subnet {} ({} hosts)",
            network,
            size
        );
        return ips;
    }

    for ip in network.iter() {
        if let IpAddr::V4(v4) = ip {
            // Skip network address and broadcast
            if v4 != network.network().to_string().parse::<Ipv4Addr>().unwrap_or(Ipv4Addr::UNSPECIFIED) {
                ips.push(v4);
            }
        }
    }

    // Cap at 20000 to prevent excessive scanning
    if ips.len() > 20000 {
        log::warn!("Network too large to scan ({} IPs), truncating", ips.len());
        ips.truncate(20000);
    }

    ips
}

/// Run a network scan: check all IPs in local subnets + known IPs for open port 5000.
/// Returns list of IPs with port 5000 open.
pub async fn scan_network(
    app: AppHandle,
    found_ips: Arc<Mutex<Vec<String>>>,
) -> Vec<String> {
    let subnets = get_local_subnets();
    let mut all_ips: Vec<Ipv4Addr> = Vec::new();

    // Add known/hardcoded IPs first
    for ip_str in KNOWN_IPS {
        if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
            all_ips.push(ip);
        }
    }

    // Add previously found IPs
    {
        let existing = found_ips.lock().unwrap_or_else(|e| e.into_inner());
        for ip_str in existing.iter() {
            if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                if !all_ips.contains(&ip) {
                    all_ips.push(ip);
                }
            }
        }
    }

    // Enumerate all subnet IPs
    let mut has_large_network = false;
    for subnet in &subnets {
        let subnet_size: u128 = match subnet.size() {
            ipnetwork::NetworkSize::V4(s) => s as u128,
            ipnetwork::NetworkSize::V6(s) => s,
        };
        if subnet_size >= 65534 {
            has_large_network = true;
            continue;
        }
        let subnet_ips = enumerate_subnet_ips(subnet);
        for ip in subnet_ips {
            if !all_ips.contains(&ip) {
                all_ips.push(ip);
            }
        }
    }

    if all_ips.is_empty() && has_large_network {
        let _ = app.emit(
            "scan-progress",
            ScanProgress {
                percentage: 100,
                message: "Error: network too large to scan".to_string(),
            },
        );
        return Vec::new();
    }

    let total = all_ips.len();
    log::info!("Starting network scan of {} IPs", total);

    let open_ips: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // Scan in batches with concurrency limit
    let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_SCANS));
    let mut handles = Vec::new();
    let scanned_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    for ip in all_ips {
        let sem = semaphore.clone();
        let open_ips = open_ips.clone();
        let app = app.clone();
        let count = scanned_count.clone();
        let total = total;

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            if is_port_open(ip, PORT).await {
                let ip_str = ip.to_string();
                log::info!("Found open port {} on {}", PORT, ip_str);
                open_ips.lock().unwrap_or_else(|e| e.into_inner()).push(ip_str);
            }

            let done = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            // Emit progress every 1%
            if total > 0 && (done % (total / 100).max(1) == 0 || done == total) {
                let pct = (100 * done / total) as u32;
                let _ = app.emit(
                    "scan-progress",
                    ScanProgress {
                        percentage: pct,
                        message: format!("Long Scan: {}%", pct),
                    },
                );
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }

    let result = open_ips.lock().unwrap_or_else(|e| e.into_inner()).clone();
    log::info!("Network scan complete. Found {} open ports.", result.len());
    result
}

/// Start the recurring network scan loop.
pub fn start_scan_loop(
    app: AppHandle,
    found_ips: Arc<Mutex<Vec<String>>>,
    rt: tauri::async_runtime::RuntimeHandle,
) {
    std::thread::spawn(move || {
        loop {
            let app_clone = app.clone();
            let ips_clone = found_ips.clone();
            let new_ips = rt.block_on(scan_network(app_clone.clone(), ips_clone));

            // Merge new IPs and trigger device updates
            {
                let mut ips = found_ips.lock().unwrap_or_else(|e| e.into_inner());
                let mut changed = false;
                for ip in &new_ips {
                    if !ips.contains(ip) {
                        ips.push(ip.clone());
                        changed = true;
                    }
                }
                if changed {
                    let _ = app_clone.emit("ips-updated", ());
                }
            }

            std::thread::sleep(Duration::from_secs(SCAN_INTERVAL_SECS));
        }
    });
}

#[derive(Clone, serde::Serialize)]
pub struct ScanProgress {
    pub percentage: u32,
    pub message: String,
}
