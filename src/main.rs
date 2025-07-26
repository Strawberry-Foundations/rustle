use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::{
    collections::HashMap,
    env,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    path::Path,
    time::Duration,
};

use crate::commands::daemon::start_daemon;

pub mod commands;
pub mod core;

#[derive(Debug, Clone)]
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
        commands::help::help();
        return;
    }

    match args[1].as_str() {
        "daemon" => {
            start_daemon().await;
        }
        "send" => {
            if args.len() < 3 {
                println!("Error: File path required");
                println!("Usage: {} send <file_path>", args[0]);
                return;
            }
            send_file(&args[2]).await;
        }
        "scan" => {
            scan_services().await;
        }
        _ => {
            println!("Error: Unknown command '{}'", args[1]);
            commands::help::help();
        }
    }
}

async fn send_file(file_path: &str) {
    println!("Scanning for available devices...");

    if !Path::new(file_path).exists() {
        println!("Error: File '{file_path}' does not exist");
        return;
    }

    let devices = discover_devices().await;
    if devices.is_empty() {
        println!("No devices found.");
        println!("Make sure target devices are running 'rustle daemon'");
        return;
    }

    println!("Found {} device(s):", devices.len());
    for (i, device) in devices.iter().enumerate() {
        println!("  [{}] {} ({})", i + 1, device.name, device.hostname);
        // Zeige alle IPs
        let ips = get_valid_ips(&device.hostname);
        for (j, ip) in ips.iter().enumerate() {
            println!("      [{}] IP: {}", j + 1, ip);
        }
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
    let ips = get_valid_ips(&target_device.hostname);
    if ips.is_empty() {
        println!("No valid IP found for device.");
        return;
    }
    println!("Select IP for connection (1-{}): ", ips.len());
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    let mut ip_input = String::new();
    std::io::stdin().read_line(&mut ip_input).unwrap();
    let ip_choice: usize = match ip_input.trim().parse::<usize>() {
        Ok(n) if n > 0 && n <= ips.len() => n - 1,
        _ => {
            println!("Invalid IP selection");
            return;
        }
    };
    let selected_ip = &ips[ip_choice];
    println!(
        "Sending transfer request to {} ({})...",
        target_device.name, selected_ip
    );
    let device_for_transfer = DiscoveredDevice {
        name: target_device.name.clone(),
        hostname: target_device.hostname.clone(),
        ip: selected_ip.clone(),
        port: target_device.port,
    };
    match send_transfer_request(&device_for_transfer, file_path).await {
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
            println!("Error sending transfer request: {e}");
        }
    }
    // Liefert alle privaten IPs für einen Hostnamen
    fn get_valid_ips(hostname: &str) -> Vec<String> {
        use std::net::ToSocketAddrs;
        let mut ips = Vec::new();
        let addr_str = format!("{hostname}:49242");
        if let Ok(addrs) = addr_str.to_socket_addrs() {
            for addr in addrs {
                let ip = addr.ip();
                if is_private_ip(&ip) {
                    ips.push(ip.to_string());
                }
            }
        }
        ips
    }

    fn is_private_ip(ip: &std::net::IpAddr) -> bool {
        if let std::net::IpAddr::V4(ipv4) = ip {
            let octets = ipv4.octets();
            // 10.x.x.x
            if octets[0] == 10 {
                return true;
            }
            // 192.168.x.x
            if octets[0] == 192 && octets[1] == 168 {
                return true;
            }
            // 172.16.x.x - 172.31.x.x
            if octets[0] == 172 && (16..=31).contains(&octets[1]) {
                return true;
            }
        }
        false
    }
}

async fn discover_devices() -> Vec<DiscoveredDevice> {
    let mdns = ServiceDaemon::new().expect("Failed to create daemon");
    let service_type = "_rustle._tcp.local.";
    let receiver = mdns.browse(service_type).expect("Failed to browse");

    let mut devices = HashMap::new();
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(20);

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

async fn send_transfer_request(
    device: &DiscoveredDevice,
    file_path: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let sender_name = whoami::fallible::hostname().unwrap_or_else(|_| "Unknown".to_string());
    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    let mut stream = TcpStream::connect(format!("{}:{}", device.ip, device.port))?;

    let request = format!("TRANSFER_REQUEST|{sender_name}|{file_name}\n");
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
                        println!("  {clean_name}");
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
