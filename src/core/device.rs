use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::{collections::HashMap, time::Duration};

use crate::core::constants::LOGGER;

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub name: String,
    pub hostname: String,
    pub ip: String,
    pub port: u16,
}

pub async fn discover_devices() -> Vec<DiscoveredDevice> {
    let mdns = ServiceDaemon::new().expect("Failed to create daemon");
    let service_type = "_rustle._tcp.local.";
    let mut devices = HashMap::new();
    let timeout = Duration::from_secs(10);
    let start_time = std::time::Instant::now();

    LOGGER.info(&format!("Active mDNS scan for '{}' for up to {} seconds...", service_type, timeout.as_secs()));

    let poll_interval = Duration::from_secs(2);
    while start_time.elapsed() < timeout {
        let receiver = match mdns.browse(service_type) {
            Ok(r) => r,
            Err(e) => {
                LOGGER.error(&format!("Failed to browse: {}", e));
                break;
            }
        };
        let poll_start = std::time::Instant::now();
        while poll_start.elapsed() < poll_interval {
            match receiver.recv_timeout(Duration::from_millis(300)) {
                Ok(event) => {
                    // LOGGER.info(&format!("Received mDNS event: {:?}", event));
                    let info = match event {
                        ServiceEvent::ServiceResolved(info) => Some(info),
                        _ => None,
                    };
                    if let Some(info) = info {
                        let device_id = format!("{}:{}", info.get_hostname(), info.get_port());
                        if devices.contains_key(&device_id) {
                            continue;
                        }
                        let ip_addr = info
                            .get_addresses_v4()
                            .iter()
                            .find(|addr| !addr.is_loopback())
                            .map(|ip| ip.to_string())
                            .or_else(|| {
                                info.get_addresses()
                                    .iter()
                                    .find_map(|ip| match ip {
                                        std::net::IpAddr::V4(v4) => Some(v4.to_string()),
                                        std::net::IpAddr::V6(v6) => Some(v6.to_string()),
                                    })
                            });
                        if let Some(ip) = ip_addr {
                            let device = DiscoveredDevice {
                                name: extract_instance_name(info.get_fullname()).to_string(),
                                hostname: info.get_hostname().trim_end_matches('.').to_string(),
                                ip,
                                port: info.get_port(),
                            };
                            LOGGER.info(&format!("Discovered device: {} at {}", device.name, device.ip));
                            devices.insert(device_id, device);
                        }
                    }
                }
                Err(_) => continue,
            }
        }
    }

    if devices.is_empty() {
        LOGGER.warning("Discovery finished. No devices found.");
    } else {
        LOGGER.info(&format!("Discovery finished. Found {} device(s).", devices.len()));
    }

    devices.into_values().collect()
}

pub fn extract_instance_name(fullname: &str) -> &str {
    fullname.split('.').next().unwrap_or(fullname)
}
