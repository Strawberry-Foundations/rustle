use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::{env, time::Duration, collections::HashMap};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} [send|receive|scan]", args[0]);
        return;
    }

    match args[1].as_str() {
        "send" => {
            println!("Send mode selected.");
            
            // Create the mDNS daemon
            let mdns = ServiceDaemon::new().expect("Failed to create mDNS daemon");
            
            // Create service info
            let mut properties = HashMap::new();
            properties.insert("path".to_string(), "/".to_string());
            properties.insert("version".to_string(), "1.0".to_string());
            
            let service_info = ServiceInfo::new(
                "_rustle._tcp.local.",  // Service type (mit Punkt am Ende!)
                "Rustle File Server",   // Instance name
                "localhost.local.",     // Hostname (muss mit .local. enden!)
                (), // Default IP addresses
                8080,                   // Port
                properties              // TXT properties
            ).expect("Failed to create service info");

            // Register the service
            mdns.register(service_info).expect("Failed to register service");
            
            println!("Service registriert: _rustle._tcp.local auf Port 8080");
            println!("Instance: Rustle File Server");
            println!("Warte auf Verbindungen...");

            // Keep the service alive
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        }
        "receive" => {
            println!("Receive mode selected.");
            
            // Create the mDNS daemon
            let mdns = ServiceDaemon::new().expect("Failed to create mDNS daemon");
            
            // Browse for services
            let receiver = mdns.browse("_rustle._tcp.local.").expect("Failed to browse");
            
            println!("Starte Discovery nach Services: _rustle._tcp.local.");
            println!("Suche läuft...");
            
            let mut found_services = false;
            let timeout = tokio::time::sleep(Duration::from_secs(10));
            tokio::pin!(timeout);
            
            loop {
                tokio::select! {
                    event = receiver.recv_async() => {
                        match event {
                            Ok(event) => {
                                use mdns_sd::ServiceEvent;
                                match event {
                                    ServiceEvent::ServiceResolved(info) => {
                                        found_services = true;
                                        println!("Service gefunden! Details:");
                                        println!("  Name: {}", info.get_fullname());
                                        println!("  Hostname: {}", info.get_hostname());
                                        println!("  Port: {}", info.get_port());
                                        println!("  Adressen:");
                                        for addr in info.get_addresses() {
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
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                println!("Fehler beim Empfangen: {:?}", e);
                            }
                        }
                    }
                    _ = &mut timeout => {
                        println!("Timeout erreicht.");
                        break;
                    }
                }
            }
            
            if !found_services {
                println!("Keine Services gefunden. Überprüfen Sie:");
                println!("1. Dass der Send-Modus auf einem anderen Gerät läuft");
                println!("2. Dass beide Geräte im gleichen Netzwerk sind");
                println!("3. Dass keine Firewall mDNS blockiert");
            }
        }
        "scan" => {
            println!("Scanning for all mDNS services...");
            
            let mdns = ServiceDaemon::new().expect("Failed to create mDNS daemon");
            let receiver = mdns.browse("_services._dns-sd._udp.local").expect("Failed to browse");
            
            let timeout = tokio::time::sleep(Duration::from_secs(10));
            tokio::pin!(timeout);
            
            loop {
                tokio::select! {
                    event = receiver.recv_async() => {
                        match event {
                            Ok(event) => {
                                use mdns_sd::ServiceEvent;
                                match event {
                                    ServiceEvent::ServiceResolved(info) => {
                                        println!("Found service type: {}", info.get_fullname());
                                    }
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                println!("Error: {:?}", e);
                            }
                        }
                    }
                    _ = &mut timeout => {
                        println!("Scan timeout.");
                        break;
                    }
                }
            }
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
