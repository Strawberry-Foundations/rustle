use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::{collections::HashMap, time::Duration};

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
    let receiver = mdns.browse(service_type).expect("Failed to browse");

    let mut devices = HashMap::new();
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(8);

    while start_time.elapsed() < timeout {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(event) => {
                if let ServiceEvent::ServiceResolved(info) = event {
                    let device_id = format!("{}:{}", info.get_hostname(), info.get_port());

                    if let std::collections::hash_map::Entry::Vacant(e) = devices.entry(device_id) {
                        if let Some(ip) = info.get_addresses().iter().next() {
                            let device = DiscoveredDevice {
                                name: extract_instance_name(info.get_fullname()).to_string(),
                                hostname: info.get_hostname().trim_end_matches('.').to_string(),
                                ip: ip.to_string(),
                                port: info.get_port(),
                            };
                            e.insert(device);
                        }
                    }
                }
            }
            Err(_) => continue,
        }
    }

    devices.into_values().collect()
}

pub fn extract_instance_name(fullname: &str) -> &str {
    fullname.split('.').next().unwrap_or(fullname)
}
