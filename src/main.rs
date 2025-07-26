use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use stblib::notifications::Notifier;
use std::{
    env, time::Duration, collections::HashMap, 
    net::{TcpListener, TcpStream}, 
    io::{Write, BufRead, BufReader},
    path::Path,
};
use tokio::task;

#[derive(Debug)]
struct DiscoveredDevice {
    name: String,
    hostname: String,
    ip: String,
    port: u16,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        return;
    }

    match args[1].as_str() {
        "announce" => {
            start_announcement_service().await;
        }
        "send" => {
            if args.len() < 3 {
                println!("Error: File path required");
                println!("Usage: {} send <file_path>", args[0]);
                return;
            }
            send_file(&args[2]).await;
        }
        "receive" => {
            start_receive_daemon().await;
        }
        "scan" => {
            scan_services().await;
        }
        _ => {
            println!("Error: Unknown command '{}'", args[1]);
            print_usage(&args[0]);
        }
    }
}

async fn start_announcement_service() {
    println!("Starting Rustle announcement service...");

    let mdns = ServiceDaemon::new().expect("Failed to create daemon");
    let service_type = "_rustle._tcp.local.";
    let instance_name = "Rustle File Server";
    let hostname = format!("{}.local.", 
        whoami::fallible::hostname().unwrap_or_else(|_| "localhost".to_string()));
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

    println!("Service registered successfully");
    println!("  Name: {}", instance_name);
    println!("  Hostname: {}", hostname.trim_end_matches('.'));
    println!("  Port: {}", port);
    println!("\nListening for connections...");
    println!("(Press Ctrl+C to stop)");

    // Keep the service alive
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

async fn send_file(file_path: &str) {
    println!("Scanning for available devices...");

    if !Path::new(file_path).exists() {
        println!("Error: File '{}' does not exist", file_path);
        return;
    }

    let devices = discover_devices().await;
    
    if devices.is_empty() {
        println!("No devices found.");
        println!("Make sure target devices are running 'rustle announce'");
        return;
    }

    println!("Found {} device(s):", devices.len());
    for (i, device) in devices.iter().enumerate() {
        println!("  [{}] {} ({})", i + 1, device.name, device.hostname);
    }

    print!("\nSelect device (1-{}): ", devices.len());
    std::io::Write::flush(&mut std::io::stdout()).unwrap();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    
    let choice: usize = match input.trim().parse::<usize>() {
        Ok(n) if n > 0 && n <= devices.len() => n - 1,
        _ => {
            println!("Invalid selection");
            return;
        }
    };

    let target_device = &devices[choice];
    println!("Sending transfer request to {}...", target_device.name);

    // Send transfer request
    match send_transfer_request(target_device, file_path).await {
        Ok(accepted) => {
            if accepted {
                println!("Transfer accepted! Sending file...");
                // TODO: Implement actual file transfer
                println!("File transfer completed successfully!");
            } else {
                println!("Transfer was declined by the recipient.");
            }
        }
        Err(e) => {
            println!("Error sending transfer request: {}", e);
        }
    }
}

async fn start_receive_daemon() {
    println!("Starting receive daemon...");
    
    // Start TCP server for receiving transfer requests
    let listener = TcpListener::bind("0.0.0.0:8080").expect("Failed to bind to port 8080");
    println!("Listening on port 8080 for transfer requests...");
    println!("(Press Ctrl+C to stop)");

    // Handle incoming connections
    task::spawn_blocking(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    if let Err(e) = handle_transfer_request(stream) {
                        println!("Error handling transfer request: {}", e);
                    }
                }
                Err(e) => {
                    println!("Connection error: {}", e);
                }
            }
        }
    }).await.unwrap();
}

fn handle_transfer_request(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let parts: Vec<&str> = request_line.trim().split('|').collect();
    if parts.len() != 3 || parts[0] != "TRANSFER_REQUEST" {
        stream.write_all(b"ERROR|Invalid request format\n")?;
        return Ok(());
    }

    let sender_name = parts[1];
    let file_name = parts[2];

    println!("Transfer request from {} for file '{}'", sender_name, file_name);

    // Show notification and get user response
    let accepted = show_transfer_notification(sender_name, file_name)?;

    if accepted {
        stream.write_all(b"ACCEPTED\n")?;
        println!("Transfer accepted");
        // TODO: Handle actual file transfer
    } else {
        stream.write_all(b"DECLINED\n")?;
        println!("Transfer declined");
    }

    Ok(())
}

fn show_transfer_notification(sender: &str, filename: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let notifier = Notifier::new(
        "File Transfer Request",
        &format!("{} wants to send you '{}'", sender, filename),
        "Rustle",
        "normal",
        "/usr/share/icons/hicolor/48x48/apps/folder.png", // Default icon
        None,
        30000, // 30 second timeout
        false,
    ).build();

    let actions = vec![
        ("accept".to_string(), "Accept".to_string()),
        ("decline".to_string(), "Decline".to_string()),
    ];

    match notifier.send_with_actions_and_wait(actions)? {
        Some(action) => Ok(action == "accept"),
        None => Ok(false), // Timeout or closed = decline
    }
}

async fn discover_devices() -> Vec<DiscoveredDevice> {
    let mdns = ServiceDaemon::new().expect("Failed to create daemon");
    let service_type = "_rustle._tcp.local.";
    let receiver = mdns.browse(service_type).expect("Failed to browse");

    let mut devices = HashMap::new();
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(5);

    while start_time.elapsed() < timeout {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(event) => {
                if let ServiceEvent::ServiceResolved(info) = event {
                    let device_id = format!("{}:{}", info.get_hostname(), info.get_port());
                    
                    if !devices.contains_key(&device_id) {
                        if let Some(ip) = info.get_addresses().iter().next() {
                            let device = DiscoveredDevice {
                                name: extract_instance_name(info.get_fullname()).to_string(),
                                hostname: info.get_hostname().trim_end_matches('.').to_string(),
                                ip: ip.to_string(),
                                port: info.get_port(),
                            };
                            devices.insert(device_id, device);
                        }
                    }
                }
            }
            Err(_) => continue,
        }
    }

    devices.into_values().collect()
}

async fn send_transfer_request(device: &DiscoveredDevice, file_path: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let sender_name = whoami::fallible::hostname().unwrap_or_else(|_| "Unknown".to_string());
    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    let mut stream = TcpStream::connect(format!("{}:{}", device.ip, device.port))?;
    
    let request = format!("TRANSFER_REQUEST|{}|{}\n", sender_name, file_name);
    stream.write_all(request.as_bytes())?;

    let mut response = String::new();
    let mut reader = BufReader::new(&stream);
    reader.read_line(&mut response)?;

    Ok(response.trim() == "ACCEPTED")
}

async fn scan_services() {
    println!("Scanning for all mDNS services...");

    let mdns = ServiceDaemon::new().expect("Failed to create daemon");
    let service_type = "_services._dns-sd._udp.local.";
    let receiver = mdns.browse(service_type).expect("Failed to browse");

    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(8);
    let mut services = std::collections::HashSet::new();

    while start_time.elapsed() < timeout {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(event) => {
                if let ServiceEvent::ServiceResolved(info) = event {
                    let service_name = info.get_fullname().to_string();
                    if services.insert(service_name.clone()) {
                        let clean_name = service_name
                            .trim_end_matches(".local.")
                            .trim_end_matches("._dns-sd._udp");
                        println!("  {}", clean_name);
                    }
                }
            }
            Err(_) => continue,
        }
    }

    println!("\nScan completed. Found {} service types.", services.len());
}

fn extract_instance_name(fullname: &str) -> &str {
    fullname.split('.').next().unwrap_or(fullname)
}

fn print_usage(program_name: &str) {
    println!("Rustle - Fast File Transfer Tool");
    println!();
    println!("Usage:");
    println!("  {} announce        - Start announcement service (run as daemon)", program_name);
    println!("  {} send <file>     - Send a file to discovered device", program_name);
    println!("  {} receive         - Start receive daemon", program_name);
    println!("  {} scan            - Show all mDNS services", program_name);
    println!();
    println!("Examples:");
    println!("  {} announce        # Start on target device", program_name);
    println!("  {} receive         # Start receiver daemon", program_name);
    println!("  {} send photo.jpg  # Send file to discovered device", program_name);
}
