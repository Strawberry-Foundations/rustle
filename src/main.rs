use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::{env, time::Duration, collections::HashSet};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} [send|receive|scan]", args[0]);
        return;
    }

    match args[1].as_str() {
        "send" => {
            println!("🚀 Starte Rustle File Server...");

            let mdns = ServiceDaemon::new().expect("Failed to create daemon");

            let service_type = "_rustle._tcp.local.";
            let instance_name = "Rustle File Server";
            let hostname = format!("{}.local.", whoami::fallible::hostname().unwrap_or_else(|_| "localhost".to_string()));
            let port = 8080;

            let properties = [("path", "/"), ("version", "1.0")];

            let service_info = ServiceInfo::new(
                service_type,
                instance_name,
                &hostname,
                "",
                port,
                &properties[..],
            )
            .expect("Failed to create service info")
            .enable_addr_auto();

            mdns.register(service_info)
                .expect("Failed to register service");

            println!("✅ Service erfolgreich registriert");
            println!("   Name: {}", instance_name);
            println!("   Hostname: {}", hostname.trim_end_matches('.'));
            println!("   Port: {}", port);
            println!("\n📡 Warte auf eingehende Verbindungen...");
            println!("   (Drücken Sie Ctrl+C zum Beenden)");

            // Keep the service alive
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
        "receive" => {
            println!("🔍 Suche nach verfügbaren Rustle-Servern...");

            let mdns = ServiceDaemon::new().expect("Failed to create daemon");
            let service_type = "_rustle._tcp.local.";
            let receiver = mdns.browse(service_type).expect("Failed to browse");

            let mut found_services = HashSet::new();
            let start_time = std::time::Instant::now();
            let timeout = Duration::from_secs(10);

            while start_time.elapsed() < timeout {
                match receiver.recv_timeout(Duration::from_millis(200)) {
                    Ok(event) => match event {
                        ServiceEvent::ServiceResolved(info) => {
                            let service_id = format!("{}:{}", info.get_hostname(), info.get_port());
                            
                            // Verhindere doppelte Ausgaben für den gleichen Service
                            if found_services.insert(service_id.clone()) {
                                println!("\n📍 Server gefunden:");
                                println!("   Name: {}", extract_instance_name(info.get_fullname()));
                                println!("   Hostname: {}", info.get_hostname().trim_end_matches('.'));
                                println!("   Port: {}", info.get_port());
                                
                                // Zeige nur die erste (wahrscheinlich lokale) IP-Adresse
                                if let Some(addr) = info.get_addresses().iter().next() {
                                    println!("   IP: {}", addr);
                                }
                                
                                // TXT-Records nur wenn interessant
                                let properties = info.get_properties();
                                if properties.len() > 0 {
                                    for property in properties.iter().take(3) { // Maximal 3 Properties
                                        let prop_str = property.to_string();
                                        if let Some((key, value)) = prop_str.split_once('=') {
                                            println!("   {}: {}", key, value);
                                        }
                                    }
                                }
                            }
                        }
                        ServiceEvent::ServiceRemoved(_, fullname) => {
                            let instance_name = extract_instance_name(&fullname);
                            println!("\n❌ Server nicht mehr verfügbar: {}", instance_name);
                        }
                        _ => {} // Ignoriere andere Events
                    },
                    Err(_) => continue,
                }
            }

            let found_count = found_services.len();
            println!("\n🏁 Suche abgeschlossen.");
            
            if found_count == 0 {
                println!("❌ Keine Rustle-Server gefunden.");
                println!("\n💡 Mögliche Lösungen:");
                println!("   • Stellen Sie sicher, dass ein Server läuft (cargo run send)");
                println!("   • Überprüfen Sie Ihre Netzwerkverbindung");
                println!("   • Deaktivieren Sie temporär die Firewall");
            } else {
                println!("✅ {} Server gefunden", found_count);
            }
        }
        "scan" => {
            println!("🔎 Scanne alle verfügbaren mDNS-Services...");

            let mdns = ServiceDaemon::new().expect("Failed to create daemon");
            let service_type = "_services._dns-sd._udp.local.";
            let receiver = mdns.browse(service_type).expect("Failed to browse");

            let start_time = std::time::Instant::now();
            let timeout = Duration::from_secs(8);
            let mut services = HashSet::new();

            while start_time.elapsed() < timeout {
                match receiver.recv_timeout(Duration::from_millis(200)) {
                    Ok(event) => match event {
                        ServiceEvent::ServiceResolved(info) => {
                            let service_name = info.get_fullname().to_string();
                            if services.insert(service_name.clone()) {
                                // Formatiere Service-Namen schöner
                                let clean_name = service_name
                                    .trim_end_matches(".local.")
                                    .trim_end_matches("._dns-sd._udp");
                                println!("   {}", clean_name);
                            }
                        }
                        _ => {}
                    },
                    Err(_) => continue,
                }
            }

            println!("\n🏁 Scan abgeschlossen. {} Service-Typen gefunden.", services.len());
        }
        _ => {
            println!("❌ Unbekannter Befehl: {}", args[1]);
            println!("\n📖 Verwendung:");
            println!("   {} send     - Startet einen File-Server", args[0]);
            println!("   {} receive  - Sucht nach verfügbaren Servern", args[0]);
            println!("   {} scan     - Zeigt alle mDNS-Services an", args[0]);
        }
    }
}

// Hilfsfunktion um den Instance-Namen aus dem Fullname zu extrahieren
fn extract_instance_name(fullname: &str) -> &str {
    fullname.split('.').next().unwrap_or(fullname)
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
