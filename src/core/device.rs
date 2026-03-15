use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use ipnet::Ipv4Net;
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncBufReadExt};
use tokio::net::TcpStream;

use crate::core::constants::{CFG, LOGGER};

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub name: String,
    pub hostname: String,
    pub ip: String,
    pub port: u16,
    pub display_name: Option<String>,
    pub avatar_data: Option<Vec<u8>>,
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

    // Browse services
    let receiver = mdns.browse(service_type).expect("Failed to browse");

    let poll_interval = Duration::from_secs(2);
    while start_time.elapsed() < timeout {
        let poll_start = std::time::Instant::now();

        while poll_start.elapsed() < poll_interval {
            match receiver.recv_timeout(Duration::from_millis(300)) {
                Ok(event) => {
                    if let ServiceEvent::ServiceResolved(info) = event {
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
                            let hostname_str = info.get_hostname()
                                .trim_end_matches('.')
                                .trim_end_matches(".local");
                            
                            // Filter out local IPs
                            if is_local_ip(&ip) {
                                continue;
                            }

                            let mut device = DiscoveredDevice {
                                name: extract_instance_name(info.get_fullname()).to_string(),
                                hostname: hostname_str.to_string(),
                                ip: ip.clone(),
                                port: info.get_port(),
                                display_name: None,
                                avatar_data: None,
                            };

                            // Try to fetch avatar (sync for CLI scan? No, keep async)
                            // We can't await easily inside this loop if we want to process fast.
                            // But for CLI scan, it's fine.
                            
                            // Note: for cleaner code we could spawn but for CLI we just want results.
                            // Let's cheat a bit and just block or use block_on if discover_devices is called from main via tokio.
                            // discover_devices is async so we can await. But we are in a loop.
                            // We will do it sequentially for CLI scan simplicity.
                            
                            // BUT wait, fetch_avatar is async.
                            // We cannot block scan loop too long.
                            // We will spawn a task? No, `devices` is local HashMap.
                            // We can just await here.
                            if let Some(avatar) = fetch_avatar(&device.ip, device.port).await {
                                device.avatar_data = Some(avatar);
                            }

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
    
    let addr_str = if host_or_ip.contains(':') {
        host_or_ip.to_string()
    } else {
        format!("{}:{}", host_or_ip, CFG.config.network.port)
    };
    
    if let Ok(result) = tokio::time::timeout(
        Duration::from_millis(1500), 
        TcpStream::connect(&addr_str)
    ).await
        && let Ok(mut stream) = result {
            let peer_addr = stream.peer_addr().ok()?;
            
            if stream.write_all(b"INFO\n").await.is_ok() {
                let mut buffer = [0; 1024];
                if let Ok(n) = stream.read(&mut buffer).await
                    && n > 0 {
                        let response = String::from_utf8_lossy(&buffer[..n]);
                        let parts: Vec<&str> = response.trim().split('|').collect();
                        if parts.len() >= 3 && parts[0] == "INFO" {
                            let hostname = parts[1].to_string();

                            let ip = peer_addr.ip().to_string();

                            // Filter out local IPs
                            if is_local_ip(&ip) {
                                return None;
                            }

                            let port = peer_addr.port();
                            
                            let mut device = DiscoveredDevice {
                                name: hostname.clone(),
                                hostname,
                                ip: ip.clone(),
                                port,
                                display_name: None,
                                avatar_data: None,
                            };
                            
                            if let Some(avatar) = fetch_avatar(&ip, port).await {
                                device.avatar_data = Some(avatar);
                            }
                            
                            return Some(device);
                        }
                    }
            }
        }
    None
}

async fn fetch_avatar(ip: &str, port: u16) -> Option<Vec<u8>> {
    let addr = format!("{}:{}", ip, port);
    // Short timeout for avatar fetch
    if let Ok(Ok(mut stream)) = tokio::time::timeout(Duration::from_millis(500), TcpStream::connect(addr)).await {
        if stream.write_all(b"GET_AVATAR\n").await.is_ok() {
            let mut reader = tokio::io::BufReader::new(&mut stream);
            let mut line = String::new();
            if reader.read_line(&mut line).await.is_ok() {
                let parts: Vec<&str> = line.trim().split('|').collect();
                if parts.len() >= 2 && parts[0] == "AVATAR" {
                     if let Ok(size) = parts[1].trim().parse::<u64>() {
                         if size > 0 && size < 10_000_000 { // Limit size just in case
                             let mut buffer = vec![0u8; size as usize];
                             if reader.read_exact(&mut buffer).await.is_ok() {
                                 return Some(buffer);
                             }
                         }
                     }
                }
            }
        }
    }
    None
}

fn is_local_ip(ip: &str) -> bool {
    std::net::UdpSocket::bind(format!("{}:0", ip)).is_ok()
}

pub async fn discover_devices_continuously(devices: Arc<Mutex<Vec<DiscoveredDevice>>>) {
    let service_type = "_rustle._tcp.local.";
    LOGGER.info(format!(
        "Starting continuous mDNS scan for '{service_type}'..."
    ));

    let mdns = ServiceDaemon::new().expect("Failed to create daemon");
    let receiver = mdns.browse(service_type).expect("Failed to browse");

    loop {
        // --- 1. Static Peers Scanning ---
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

        if !scanning_targets.is_empty() {
            let devices_clone = devices.clone();
            let targets = scanning_targets.clone();

            tokio::spawn(async move {
                let mut tasks = Vec::new();
                for target in targets {
                    let dev_clone = devices_clone.clone();
                    tasks.push(tokio::spawn(async move {
                        if let Some(device) = check_static_peer(&target).await {
                             let mut shared = dev_clone.lock().unwrap();
                             if !shared.iter().any(|d| d.hostname == device.hostname && d.ip == device.ip) {
                                 LOGGER.info(format!("Found static/subnet device: {} ({})", device.name, device.ip));
                                 shared.push(device);
                             }
                        }
                    }));
                }
                // Wait for all checks to complete so we don't spam network
                futures::future::join_all(tasks).await;
            });
        }

        // --- 2. mDNS Event Loop ---
        let loop_start = std::time::Instant::now();
        while loop_start.elapsed() < Duration::from_secs(5) {
            match receiver.recv_timeout(Duration::from_millis(500)) {
                Ok(event) => {
                    if let ServiceEvent::ServiceResolved(info) = event {
                         let ip_addr = info.get_addresses_v4()
                            .iter()
                            .find(|addr| !addr.is_loopback())
                            .map(|ip| ip.to_string())
                            .or_else(|| info.get_addresses().iter().map(|ip| ip.to_string()).next());

                         if let Some(ip) = ip_addr {
                             let ip_clone = ip.clone();
                             let port = info.get_port();
                             let name = extract_instance_name(info.get_fullname()).to_string();
                             let hostname = info.get_hostname()
                                .trim_end_matches('.')
                                .trim_end_matches(".local")
                                .to_string();

                             // Filter out local IPs logic
                             if is_local_ip(&ip_clone) {
                                 continue;
                             }
                             
                             let devices_ref = devices.clone();
                             
                             // Already known?
                             {
                                 let shared = devices_ref.lock().unwrap();
                                 if shared.iter().any(|d| d.hostname == hostname && d.ip == ip_clone) {
                                     continue;
                                 }
                             }
                             
                             tokio::spawn(async move {
                                 let mut device = DiscoveredDevice {
                                     name: name.clone(),
                                     hostname: hostname.clone(),
                                     ip: ip_clone.clone(),
                                     port,
                                     display_name: None,
                                     avatar_data: None,
                                 };
                                 
                                 // Fetch avatar
                                 if let Some(avatar) = fetch_avatar(&device.ip, device.port).await {
                                     device.avatar_data = Some(avatar);
                                 }
                                 
                                 // Add to list
                                 let mut shared = devices_ref.lock().unwrap();
                                 if !shared.iter().any(|d| d.hostname == device.hostname && d.ip == device.ip) {
                                     LOGGER.info(format!("Found mDNS device: {} ({}) [Avatar: {}]", device.name, device.ip, device.avatar_data.is_some()));
                                     shared.push(device);
                                 }
                             });
                         }
                    }
                }
                Err(_) => continue,
            }
        }
    }
}

pub fn extract_instance_name(fullname: &str) -> &str {
    fullname.split('.').next().unwrap_or(fullname)
}
