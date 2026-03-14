use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;
use ipnet::Ipv4Net;

use crate::core::constants::{CFG, LOGGER};

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
                                    .map(|ip| ip.to_string())
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

async fn check_static_peer(host_or_ip: &str) -> Option<DiscoveredDevice> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    
    let addr_str = if host_or_ip.contains(':') {
        host_or_ip.to_string()
    } else {
        format!("{}:{}", host_or_ip, CFG.config.network.port)
    };
    
    if let Ok(result) = tokio::time::timeout(
        Duration::from_millis(1500), 
        TcpStream::connect(&addr_str)
    ).await {
        if let Ok(mut stream) = result {
            let peer_addr = stream.peer_addr().ok()?;
            
            if stream.write_all(b"INFO\n").await.is_ok() {
                let mut buffer = [0; 1024];
                if let Ok(n) = stream.read(&mut buffer).await {
                    if n > 0 {
                        let response = String::from_utf8_lossy(&buffer[..n]);
                        let parts: Vec<&str> = response.trim().split('|').collect();
                        if parts.len() >= 3 && parts[0] == "INFO" {
                            let hostname = parts[1].to_string();
                            let ip = peer_addr.ip().to_string();
                            let port = peer_addr.port();
                            
                            return Some(DiscoveredDevice {
                                name: hostname.clone(),
                                hostname,
                                ip,
                                port
                            });
                        }
                    }
                }
            }
        }
    }
    None
}

pub async fn discover_devices_continuously(devices: Arc<Mutex<Vec<DiscoveredDevice>>>) {
    let service_type = "_rustle._tcp.local.";
    LOGGER.info(format!(
        "Starting continuous mDNS scan for '{service_type}'..."
    ));

    // Initial mDNS setup
    let mdns = ServiceDaemon::new().expect("Failed to create daemon");
    let receiver = mdns.browse(service_type).expect("Failed to browse");

    loop {
        // --- 1. Static Peers & Subnet Scanning (Background Task) ---
        let devices_clone = devices.clone();
        
        let mut scanning_targets = Vec::new();
        if let Some(static_peers) = &CFG.config.network.static_peers {
            for peer in static_peers {
                scanning_targets.push(peer.clone());
            }
        }
        if let Some(subnets) = &CFG.config.network.scan_subnets {
            for subnet_str in subnets {
                if let Ok(net) = subnet_str.parse::<Ipv4Net>() {
                    for ip in net.hosts() {
                        scanning_targets.push(ip.to_string());
                    }
                }
            }
        }
        scanning_targets.sort();
        scanning_targets.dedup();

                        // Spawn background task for active scanning
        let devices_cloned_for_static = devices.clone();
        if !scanning_targets.is_empty() {
            tokio::spawn(async move {
                let mut set = tokio::task::JoinSet::new();
                for target in scanning_targets {
                    set.spawn(async move {
                        check_static_peer(&target).await
                    });
                }
                
                while let Some(res) = set.join_next().await {
                    if let Ok(Some(device)) = res {
                         let mut shared = devices_cloned_for_static.lock().unwrap();
                         if !shared.iter().any(|d| d.hostname == device.hostname && d.ip == device.ip) {
                             LOGGER.info(format!("Found static/subnet device: {} ({})", device.name, device.ip));
                             shared.push(device);
                         }
                    }
                }
            });
        }

        // --- 2. mDNS Event Loop (Main Task) ---
        // Run this loop for a while, processing events immediately
        let loop_duration = Duration::from_secs(10);
        let start_time = std::time::Instant::now();
        let mdns = ServiceDaemon::new().expect("Failed to create daemon");
        let receiver = mdns.browse(service_type).expect("Failed to browse");

        while start_time.elapsed() < loop_duration {
            match receiver.recv_timeout(Duration::from_millis(500)) {
                Ok(event) => {
                    if let ServiceEvent::ServiceResolved(info) = event {
                         let ip_addr = info.get_addresses_v4()
                            .iter()
                            .find(|addr| !addr.is_loopback())
                            .map(|ip| ip.to_string())
                            .or_else(|| info.get_addresses().iter().map(|ip| ip.to_string()).next());

                         if let Some(ip) = ip_addr {
                             let new_device = DiscoveredDevice {
                                 name: extract_instance_name(info.get_fullname()).to_string(),
                                 hostname: info.get_hostname().trim_end_matches('.').to_string(),
                                 ip: ip.clone(),
                                 port: info.get_port(),
                             };

                             let mut shared = devices.lock().unwrap();
                             // Check if already exists to avoid duplicates/flicker
                             if !shared.iter().any(|d| d.hostname == new_device.hostname && d.ip == new_device.ip) {
                                 LOGGER.info(format!("Found mDNS device: {} ({})", new_device.name, new_device.ip));
                                 shared.push(new_device);
                             }
                         }
                    }
                }
                Err(_) => continue, // Timeout, just loop again
            }
        }
    }
}

pub fn extract_instance_name(fullname: &str) -> &str {
    fullname.split('.').next().unwrap_or(fullname)
}
