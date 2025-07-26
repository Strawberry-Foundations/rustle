use std::{io::{BufRead, BufReader, Write}, net::TcpStream, path::Path};

use crate::core::{
    constants::LOGGER, device::{discover_devices, DiscoveredDevice}, net::get_valid_ips
};

pub async fn send_file(file_path: &str) {
    LOGGER.info("Scanning for available devices...");

    if !Path::new(file_path).exists() {
        LOGGER.error(&format!("Error: File '{file_path}' does not exist"));
        return;
    }

    let devices = discover_devices().await;
    if devices.is_empty() {
        LOGGER.error("No devices found.");
        LOGGER.warning("Make sure target devices are running 'rustle daemon'");
        return;
    }

    LOGGER.info(&format!("Found {} device(s):", devices.len()));
    for (i, device) in devices.iter().enumerate() {
        LOGGER.info(&format!("  [{}] {} ({})", i + 1, device.name, device.hostname));
        let ips = get_valid_ips(&device.hostname);
        for (j, ip) in ips.iter().enumerate() {
            LOGGER.info(&format!("      [{}] IP: {}", j + 1, ip));
        }
    }

    LOGGER.info(&format!("Select device (1-{}): ", devices.len()));
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    let choice: usize = match input.trim().parse::<usize>() {
        Ok(n) if n > 0 && n <= devices.len() => n - 1,
        _ => {
            LOGGER.warning("Invalid selection");
            return;
        }
    };

    let target_device = &devices[choice];
    let ips = get_valid_ips(&target_device.hostname);
    if ips.is_empty() {
        LOGGER.warning("No valid IP found for device.");
        return;
    }

    LOGGER.info(&format!("Select IP for connection (1-{}): ", ips.len()));
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    
    let mut ip_input = String::new();
    std::io::stdin().read_line(&mut ip_input).unwrap();
    
    let ip_choice: usize = match ip_input.trim().parse::<usize>() {
        Ok(n) if n > 0 && n <= ips.len() => n - 1,
        _ => {
            LOGGER.warning("Invalid IP selection");
            return;
        }
    };

    let selected_ip = &ips[ip_choice];
    LOGGER.info(&format!(
        "Sending transfer request to {} ({})...",
        target_device.name, selected_ip
    ));

    let device_for_transfer = DiscoveredDevice {
        name: target_device.name.clone(),
        hostname: target_device.hostname.clone(),
        ip: selected_ip.clone(),
        port: target_device.port,
    };
    match send_transfer_request(&device_for_transfer, file_path).await {
        Ok(accepted) => {
            if accepted {
                LOGGER.info("Transfer accepted! Sending file...");
                LOGGER.info("File transfer completed successfully!");
            } else {
                LOGGER.warning("Transfer was declined by the recipient.");
            }
        }
        Err(e) => {
            LOGGER.error(&format!("Error sending transfer request: {e}"));
        }
    }
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
