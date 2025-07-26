use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::time::Duration;

use crate::core::constants::LOGGER;

pub async fn scan_services() {
    LOGGER.info("Scanning for all mDNS services...");

    let mdns = ServiceDaemon::new().unwrap_or_else(|_| {
        LOGGER.error_panic("Failed to create daemon");
    });

    let receiver = mdns
        .browse("_services._dns-sd._udp.local.")
        .unwrap_or_else(|_| {
            LOGGER.error_panic("Failed to browse all service types");
        });

    let mut service_types = std::collections::HashSet::new();
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(8);

    while start_time.elapsed() < timeout {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(event) => {
                if let ServiceEvent::ServiceFound(service_type, _) = event {
                    if service_types.insert(service_type.clone()) {
                        let clean_name = service_type.trim_end_matches(".local.");
                        LOGGER.info(format!("Found service type: {clean_name}"));
                    }
                }
            }
            Err(_) => continue,
        }
    }

    LOGGER.default(format!(
        "Scan completed in {:.2?}. Found {} service types.",
        start_time.elapsed(),
        service_types.len()
    ));

    let mut all_instances = std::collections::HashSet::new();
    for service_type in &service_types {
        LOGGER.info(format!("Browsing instances for: {service_type}"));
        if let Ok(inst_receiver) = mdns.browse(service_type) {
            let inst_start = std::time::Instant::now();
            let inst_timeout = Duration::from_secs(3);

            while inst_start.elapsed() < inst_timeout {
                match inst_receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(event) => match event {
                        ServiceEvent::ServiceFound(_, fullname) => {
                            if all_instances.insert(fullname.clone()) {
                                LOGGER.info(format!("  Found: {fullname}"));
                            }
                        }
                        ServiceEvent::ServiceResolved(info) => {
                            let fullname = info.get_fullname().to_string();
                            if all_instances.insert(fullname.clone()) {
                                LOGGER.info(format!(
                                    "  Resolved: {} at {}:{}",
                                    fullname,
                                    info.get_hostname().trim_end_matches('.'),
                                    info.get_port()
                                ));
                            }
                        }
                        _ => {}
                    },
                    Err(_) => continue,
                }
            }
        }
    }

    LOGGER.default(format!(
        "Scan completed. Found {} service instances.",
        all_instances.len()
    ));
}
