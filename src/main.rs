use futures_util::stream::StreamExt;
use mdns::{RecordKind, Response, discover::all};
use std::{env, time::Duration};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: {} [send|receive]", args[0]);
        return;
    }

    match args[1].as_str() {
        "send" => {
            println!("Send mode selected.");
            let responder = libmdns::Responder::new().unwrap();
            let _svc = responder.register(
                "_http._tcp".to_owned(),
                "libmdns Web Server".to_owned(),
                8080,
                &["path=/"],
            );

            loop {
                ::std::thread::sleep(::std::time::Duration::from_secs(10));
            }
        }
        "receive" => {
            println!("Receive mode selected.");
            let service_name = "_rustle._tcp.local";

            // Starte die Discovery für 5 Sekunden
            let listener = all(service_name, Duration::from_secs(5)).unwrap().listen();

            println!("Starte Discovery nach Services: {}", service_name);

            let mut listener = Box::pin(listener);

            while let Some(result) = listener.next().await {
                match result {
                    Ok(response) => {
                        for record in response.records() {
                            match &record.kind {
                                RecordKind::A(ip) => println!("IPv4 gefunden: {:?}", ip),
                                RecordKind::AAAA(ip) => println!("IPv6 gefunden: {:?}", ip),
                                RecordKind::SRV { port, .. } => println!("Port gefunden: {:?}", port),
                                RecordKind::TXT(txt) => println!("TXT-Records: {:?}", txt),
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        println!("Fehler beim Empfangen eines mDNS-Response: {:?}", e);
                    }
                }
            }
        }
        _ => {
            println!("Unknown command: {}", args[1]);
            println!("Usage: {} [send|receive]", args[0]);
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
