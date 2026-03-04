use crate::DeviceInfo;
use std::time::Duration;

/// Probe an IP address to determine what device is running on port 5000.
/// Returns a DeviceInfo if a recognized device is found.
pub async fn probe_device(ip: &str) -> Option<DeviceInfo> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .ok()?;

    let hostname_url = format!("http://{}:5000/api/hostname", ip);

    match client.get(&hostname_url).send().await {
        Ok(response) => {
            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                // 401 indicates an AllSky camera
                return Some(DeviceInfo {
                    hostname: "allsky".to_string(),
                    ip: ip.to_string(),
                    device_type: "allsky".to_string(),
                });
            }

            if response.status().is_success() {
                // Try to parse JSON response with hostname field
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    if let Some(hostname) = json.get("hostname").and_then(|v| v.as_str()) {
                        let device_type = if hostname.to_lowercase().contains("allsky") {
                            "allsky"
                        } else {
                            "sst"
                        };
                        return Some(DeviceInfo {
                            hostname: hostname.to_string(),
                            ip: ip.to_string(),
                            device_type: device_type.to_string(),
                        });
                    }
                }
            }

            // Fallback: try /api/config endpoint
            try_config_fallback(&client, ip).await
        }
        Err(_) => {
            // Connection failed, try config fallback
            try_config_fallback(&client, ip).await
        }
    }
}

/// Fallback: check /api/config endpoint. If it responds, assume AllSky.
async fn try_config_fallback(client: &reqwest::Client, ip: &str) -> Option<DeviceInfo> {
    let config_url = format!("http://{}:5000/api/config", ip);
    match client.get(&config_url).send().await {
        Ok(response) if response.status().is_success() => Some(DeviceInfo {
            hostname: "allsky".to_string(),
            ip: ip.to_string(),
            device_type: "allsky".to_string(),
        }),
        _ => None,
    }
}

/// Probe multiple IPs and return all recognized devices.
pub async fn probe_all_ips(ips: &[String]) -> Vec<DeviceInfo> {
    let mut handles = Vec::new();

    for ip in ips {
        let ip = ip.clone();
        let handle = tokio::spawn(async move { probe_device(&ip).await });
        handles.push(handle);
    }

    let mut devices = Vec::new();
    for handle in handles {
        if let Ok(Some(device)) = handle.await {
            devices.push(device);
        }
    }

    devices
}
