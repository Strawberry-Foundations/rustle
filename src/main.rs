use mdns_sd::{ServiceDaemon, ServiceInfo, ServiceEvent};
use std::{env, time::Duration};

#[tokio::main]
async fn main() {
    // Setup logging (optional)
    env_logger::builder().format_timestamp_millis().init();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} [send|receive|scan]", args[0]);
        return;
    }

    match args[1].as_str() {
        "send" => {
            println!("Send mode selected.");
            
            // Create a new mDNS daemon
            let mdns = ServiceDaemon::new().expect("Failed to create daemon");
            
            let service_type = "_rustle._tcp.local.";
            let instance_name = "Rustle File Server";
            let hostname = format!("{}.local.", whoami::hostname());
            let port = 8080;
            
            // TXT properties
            let properties = [("path", "/"), ("version", "1.0")];
            
            // Create service info with auto address detection
            let service_info = ServiceInfo::new(
                service_type,
                instance_name,
                &hostname,
                "", // Empty means auto-detect addresses
                port,
                &properties[..],
            )
            .expect("valid service info")
            .enable_addr_auto();
            
            println!("Service registriert: {} als '{}'", service_type, instance_name);
            println!("Hostname: {}", hostname);
            println!("Port: {}", port);
            println!("Warte auf Verbindungen...");
            
            // Register the service
            mdns.register(service_info).expect("Failed to register service");
            
            // Keep the service alive
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        }
        "receive" => {
            println!("Receive mode selected.");
            
            let mdns = ServiceDaemon::new().expect("Failed to create daemon");
            
            let service_type = "_rustle._tcp.local.";
            let receiver = mdns.browse(service_type).expect("Failed to browse");
            
            println!("Starte Discovery nach Services: {}", service_type);
            println!("Suche läuft...");
            
            let mut found_services = false;
            let start_time = std::time::Instant::now();
            let timeout = Duration::from_secs(10);
            
            while start_time.elapsed() < timeout {
                // Use recv_timeout to avoid blocking forever
                match receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(event) => {
                        match event {
                            ServiceEvent::ServiceResolved(info) => {
                                found_services = true;
                                println!("Service gefunden! Details:");
                                println!("  Name: {}", info.get_fullname());
                                println!("  Hostname: {}", info.get_hostname());
                                println!("  Port: {}", info.get_port());
                                println!("  Adressen:");
                                for addr in info.get_addresses().iter() {
                                    println!("    {}", addr);
                                }
                                println!("  TXT-Records:");
                                let properties = info.get_properties();
                                for property in properties.iter() {
                                    println!("    {}", property);
                                }
                                println!("---");
                            }
                            ServiceEvent::ServiceRemoved(_, fullname) => {
                                println!("Service entfernt: {}", fullname);
                            }
                            other_event => {
                                println!("Event: {:?}", other_event);
                            }
                        }
                    }
                    Err(_) => {
                        // Timeout, continue loop
                        continue;
                    }
                }
            }
            
            if !found_services {
                println!("Keine Services gefunden. Überprüfen Sie:");
                println!("1. Dass der Send-Modus auf einem anderen Gerät läuft");
                println!("2. Dass beide Geräte im gleichen Netzwerk sind");
                println!("3. Dass keine Firewall mDNS blockiert");
                println!("4. Versuchen Sie 'cargo run scan' um alle Services zu sehen");
            }
        }
        "scan" => {
            println!("Scanning for all mDNS services...");
            
            let mdns = ServiceDaemon::new().expect("Failed to create daemon");
            
            let service_type = "_services._dns-sd._udp.local.";
            let receiver = mdns.browse(service_type).expect("Failed to browse");
            
            let start_time = std::time::Instant::now();
            let timeout = Duration::from_secs(10);
            
            println!("Suche nach verfügbaren Service-Typen...");
            
            while start_time.elapsed() < timeout {
                match receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(event) => {
                        match event {
                            ServiceEvent::ServiceResolved(info) => {
                                println!("Found service type: {}", info.get_fullname());
                            }
                            other_event => {
                                println!("Event: {:?}", other_event);
                            }
                        }
                    }
                    Err(_) => {
                        continue;
                    }
                }
            }
            
            println!("Scan abgeschlossen.");
        }
        _ => {
            println!("Unknown command: {}", args[1]);
            println!("Usage: {} [send|receive|scan]", args[0]);
        }
    }
}

/* fn main() {
    let notifier = Notifier::new(
        "File Transfer",
        "julian wants to send you 'test.txt'",
        "Rustle",
        "normal",
        "/home/julian/Bilder/970063240790962206-2.png",
        None,
        30000,
        false,
    )
    .build();

    let actions = vec![
        ("yes".to_string(), "Allow".to_string()),
        ("no".to_string(), "Decline".to_string()),
    ];

    match notifier.send_with_actions_and_wait(actions) {
        Ok(Some(action)) => {
            println!("User clicked: {}", action);
            match action.as_str() {
                "yes" => println!("Action confirmed!"),
                "no" => println!("Action declined."),
                _ => println!("Unknown action: {}", action),
            }
        },
        Ok(None) => {
            println!("Notification closed without action.");
        },
        Err(e) => {
            println!("Error occurred: {:?}", e);
        }
    }
}
*/
