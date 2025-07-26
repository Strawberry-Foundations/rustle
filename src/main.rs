use futures_util::stream::StreamExt;
use mdns::{RecordKind, discover::all};
use std::{env, time::Duration};

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
            let responder = libmdns::Responder::new().unwrap();
            let _svc = responder.register(
                "_rustle._tcp.local".to_owned(),
                "Rustle File Server".to_owned(),  // Instance name, nicht der Service-Typ
                8080,
                &["path=/", "version=1.0"],
            );

            println!("Service registriert: _rustle._tcp.local auf Port 8080");
            println!("Warte auf Verbindungen...");

            loop {
                ::std::thread::sleep(::std::time::Duration::from_secs(10));
            }
        }
        "receive" => {
            println!("Receive mode selected.");
            let service_name = "_rustle._tcp.local";

            // Starte die Discovery für 10 Sekunden (länger als vorher)
            let listener = all(service_name, Duration::from_secs(10)).unwrap().listen();

            println!("Starte Discovery nach Services: {}", service_name);
            println!("Suche für 10 Sekunden...");

            let mut listener = Box::pin(listener);
            let mut found_services = false;

            while let Some(result) = listener.next().await {
                match result {
                    Ok(response) => {
                        found_services = true;
                        println!("Service gefunden! Details:");
                        for record in response.records() {
                            match &record.kind {
                                RecordKind::A(ip) => println!("  IPv4: {}", ip),
                                RecordKind::AAAA(ip) => println!("  IPv6: {}", ip),
                                RecordKind::SRV { port, target, .. } => {
                                    println!("  Service: {} auf Port {}", target, port);
                                },
                                RecordKind::TXT(txt) => {
                                    println!("  TXT-Records: {:?}", txt);
                                },
                                RecordKind::PTR(ptr) => {
                                    println!("  PTR: {}", ptr);
                                },
                                _ => {}
                            }
                        }
                        println!("---");
                    }
                    Err(e) => {
                        println!("Fehler beim Empfangen eines mDNS-Response: {:?}", e);
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
            let listener = all("_services._dns-sd._udp.local", Duration::from_secs(10)).unwrap().listen();
            let mut listener = Box::pin(listener);
            
            while let Some(result) = listener.next().await {
                match result {
                    Ok(response) => {
                        println!("Found service type:");
                        for record in response.records() {
                            match &record.kind {
                                RecordKind::PTR(ptr) => println!("  {}", ptr),
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        println!("Error: {:?}", e);
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
