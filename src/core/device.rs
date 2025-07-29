use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;

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

    LOGGER.info(format!(
        "Active mDNS scan for '{}' for up to {} seconds...",
        service_type,
        timeout.as_secs()
    ));

    let poll_interval = Duration::from_secs(2);
    while start_time.elapsed() < timeout {
        let receiver = match mdns.browse(service_type) {
            Ok(r) => r,
            Err(e) => {
                LOGGER.error(format!("Failed to browse: {e}"));
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
                                    .map(|ip| match ip {
                                        std::net::IpAddr::V4(v4) => v4.to_string(),
                                        std::net::IpAddr::V6(v6) => v6.to_string(),
                                    })
                                    .next()
                            });
                            
                        if let Some(ip) = ip_addr {
                            let device = DiscoveredDevice {
                                name: extract_instance_name(info.get_fullname()).to_string(),
                                hostname: info.get_hostname().trim_end_matches('.').to_string(),
                                ip,
                                port: info.get_port(),
                            };
                            LOGGER.info(format!(
                                "Discovered device: {} at {}",
                                device.name, device.ip
                            ));
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
        LOGGER.info(format!(
            "Discovery finished. Found {} device(s).",
            devices.len()
        ));
    }

    devices.into_values().collect()
}

pub async fn discover_devices_continuously(devices: Arc<Mutex<Vec<DiscoveredDevice>>>) {
    let service_type = "_rustle._tcp.local.";
    LOGGER.info(format!(
        "Starting continuous mDNS scan for '{service_type}'..."
    ));

    loop {
        let mdns = ServiceDaemon::new().expect("Failed to create daemon");
        let mut local_devices = HashMap::new();
        let scan_duration = Duration::from_secs(8);
        let start_time = std::time::Instant::now();

        let poll_interval = Duration::from_secs(2);
        while start_time.elapsed() < scan_duration {
            let receiver = match mdns.browse(service_type) {
                Ok(r) => r,
                Err(e) => {
                    LOGGER.error(format!("Failed to browse: {e}"));
                    break;
                }
            };

            let poll_start = std::time::Instant::now();
            while poll_start.elapsed() < poll_interval {
                match receiver.recv_timeout(Duration::from_millis(300)) {
                    Ok(event) => {
                        let info = match event {
                            ServiceEvent::ServiceResolved(info) => Some(info),
                            _ => None,
                        };
                        if let Some(info) = info {
                            let device_id = format!("{}:{}", info.get_hostname(), info.get_port());
                            if local_devices.contains_key(&device_id) {
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
                                        .map(|ip| match ip {
                                            std::net::IpAddr::V4(v4) => v4.to_string(),
                                            std::net::IpAddr::V6(v6) => v6.to_string(),
                                        })
                                        .next()
                                });

                            if let Some(ip) = ip_addr {
                                let device = DiscoveredDevice {
                                    name: extract_instance_name(info.get_fullname()).to_string(),
                                    hostname: info.get_hostname().trim_end_matches('.').to_string(),
                                    ip,
                                    port: info.get_port(),
                                };
                                local_devices.insert(device_id, device);
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        if !local_devices.is_empty() {
            let mut shared_devices = devices.lock().unwrap();
            for new_device in local_devices.values() {
                if !shared_devices
                    .iter()
                    .any(|d| d.hostname == new_device.hostname && d.ip == new_device.ip)
                {
                    LOGGER.info(format!(
                        "Found new device: {} at {}",
                        new_device.name, new_device.ip
                    ));
                    shared_devices.push(new_device.clone());
                }
            }
        }

        sleep(Duration::from_secs(5)).await;
    }
}

pub fn extract_instance_name(fullname: &str) -> &str {
    fullname.split('.').next().unwrap_or(fullname)
}
