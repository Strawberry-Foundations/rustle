use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::time::Duration;

use crate::core::constants::LOGGER;

pub async fn scan_services() {
    LOGGER.info("Scanning for all mDNS services... (Press Ctrl+C to stop)");

    let mdns = ServiceDaemon::new().unwrap_or_else(|_| {
        LOGGER.panic("Failed to create daemon");
    });

    // Start browsing for all services
    let receiver = mdns
        .browse("_services._dns-sd._udp.local.")
        .unwrap_or_else(|_| {
            LOGGER.panic("Failed to browse all service types");
        });

    let mut service_types = std::collections::HashSet::new();

    LOGGER.info("Listening for services and instances concurrently...");
    
    let mdns = std::sync::Arc::new(mdns);
    
    loop {
        // Poll for new service types
        if let Ok(event) = receiver.recv_timeout(Duration::from_millis(500)) {
            match event {
                ServiceEvent::ServiceFound(service_type, _) => {
                    if service_types.insert(service_type.clone()) {
                        let clean_name = service_type.trim_end_matches(".local.");
                        LOGGER.info(format!("Found NEW service type: {clean_name}"));
                        
                        // Spawn a browser for this type
                        let mdns_clone = mdns.clone();
                        let type_clone = service_type.clone();
                        
                        tokio::spawn(async move {
                             if let Ok(inst_receiver) = mdns_clone.browse(&type_clone) {
                                 while let Ok(event) = inst_receiver.recv_async().await {
                                     match event {
                                         ServiceEvent::ServiceResolved(info) => {
                                             LOGGER.info(format!(
                                                 "  [Resolved] {} at {}:{}",
                                                 info.get_fullname(),
                                                 info.get_hostname().trim_end_matches('.'),
                                                 info.get_port()
                                             ));
                                         }
                                          _ => {}
                                     }
                                 }
                             }
                        });
                    }
                }
                _ => {}
            }
        }
    }
}
