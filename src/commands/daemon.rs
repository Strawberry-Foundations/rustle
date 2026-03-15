use crate::core::{constants::{CFG, LOGGER}, notifier::{show_transfer_notification, show_simple}, I18N};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
};
use tokio::task;
use crate::core::config::ConfigManager;

pub struct Daemon {
    pub port: u16,
    pub hostname: String,
}

impl Daemon {
    pub fn new(port: u16, hostname: String) -> Self {
        Self { port, hostname }
    }

    pub async fn start(&self) {
        LOGGER.info("Starting Rustle daemon");

        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).unwrap_or_else(|e| {
            LOGGER.panic(format!("Failed to bind to port {}: {e}", self.port));
        });

        let mdns: ServiceDaemon = ServiceDaemon::new().unwrap_or_else(|e| {
            LOGGER.panic(format!("Failed to create daemon: {e}"));
        });

        let service_type = "_rustle._tcp.local.";
        
        let display_name = CFG.config.user.display_name.clone().unwrap_or_else(|| {
            whoami::hostname().unwrap().to_string()
        });
        
        let instance_name = &display_name;
        
        let hostname = format!(
            "{}.local.",
            whoami::hostname().unwrap_or_else(|_| "localhost".to_string())
        );

        let mut properties = vec![("path", "/".to_string()), ("version", "1.0".to_string())];
        
        if let Some(avatar) = &CFG.config.user.avatar_path {
            properties.push(("avatar", avatar.clone()));
        }

        // Convert properties to required format for mdns-sd
        let properties_refs: Vec<(&str, &str)> = properties.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let service_info = ServiceInfo::new(
            service_type,
            instance_name,
            &hostname,
            "",
            self.port,
            &properties_refs[..],
        )
        .unwrap_or_else(|e| {
            LOGGER.panic(format!("Failed to create service info: {e}"));
        })
        .enable_addr_auto();

        mdns.register(service_info).unwrap_or_else(|e| {
            LOGGER.panic(format!("Failed to register service: {e}"));
        });

        LOGGER.ok("Daemon started successfully");

        LOGGER.info("Service discovery started");
        LOGGER.info(format!("Hostname: {}", hostname.trim_end_matches('.')));
        LOGGER.info(format!(
            "Listening for incoming connections on port {}",
            self.port
        ));

        // Handle incoming connections
        task::spawn_blocking(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        if let Err(e) = handle_transfer_request(stream) {
                            LOGGER.error(format!("Error handling transfer request: {e}"));
                        }
                    }
                    Err(e) => {
                        LOGGER.error(format!("Connection error: {e}"));
                    }
                }
            }
        })
        .await
        .unwrap();
    }
}

pub fn handle_transfer_request(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();

    reader.read_line(&mut request_line)?;

    let parts: Vec<&str> = request_line.trim().split('|').collect();

    if !parts.is_empty() && parts[0] == "INFO" {
        let hostname = whoami::hostname().unwrap_or_else(|_| "localhost".to_string());
        let response = format!("INFO|{}|Rustle|1.0\n", hostname);
        stream.write_all(response.as_bytes())?;
        return Ok(());
    }

    if !parts.is_empty() && parts[0] == "GET_AVATAR" {
        if let Some(avatar_path) = &CFG.config.user.avatar_path {
            if let Ok(img) = image::open(avatar_path) {
                let resized = img.resize(128, 128, image::imageops::FilterType::Lanczos3);
                let mut buffer = std::io::Cursor::new(Vec::new());
                if resized.write_to(&mut buffer, image::ImageFormat::Png).is_ok() {
                    let data = buffer.into_inner();
                    let response = format!("AVATAR|{}\n", data.len());
                    stream.write_all(response.as_bytes())?;
                    stream.write_all(&data)?;
                    return Ok(());
                }
            } else {
                LOGGER.warning(format!("Failed to load avatar: {avatar_path}"));
            }
        }
        stream.write_all(b"AVATAR|0\n")?;
        return Ok(());
    }

    if parts.len() != 4 || parts[0] != "TRANSFER_REQUEST" {
        stream.write_all(b"ERROR|Invalid request format\n")?;
        return Ok(());
    }

    let sender_name = parts[1];
    let file_name = parts[2];
    let file_size: u64 = parts[3].parse()?;

    LOGGER.info(format!(
        "Transfer request from {sender_name} for file '{file_name}' ({file_size} bytes)"
    ));

    let accepted = show_transfer_notification(sender_name, file_name)?;

    if accepted {
        stream.write_all(b"ACCEPTED\n")?;
        show_simple(&I18N.get("notifier_info_title"), &I18N.get("notifier_transfer_started"), false);

        LOGGER.info("Transfer accepted. Receiving file...");

        let config_manager = ConfigManager::new();
        let mut download_path = PathBuf::from(
            shellexpand::tilde(&config_manager.config.general.default_download_path).to_string(),
        );
        download_path.push(file_name);

        match receive_file_data(&mut stream, &download_path, file_size) {
            Ok(_) => {
                LOGGER.info(format!("File successfully saved to {download_path:?}"));
                stream.write_all(b"TRANSFER_COMPLETE\n")?;
                show_simple(&I18N.get("notifier_success_title"), &I18N.get_with_params("notifier_transfer_success", &[&file_name]), false);
            }
            Err(e) => {
                LOGGER.error(format!("Failed to receive file: {e}"));
                stream.write_all(format!("ERROR|{e}\n").as_bytes())?;
                show_simple(&I18N.get("notifier_error_title"), &I18N.get_with_params("notifier_transfer_error", &[&file_name, &e.to_string()]), true);
            }
        }
    } else {
        stream.write_all(b"DECLINED\n")?;
        LOGGER.info("Transfer declined");
    }

    Ok(())
}

fn receive_file_data(
    stream: &mut TcpStream,
    path: &PathBuf,
    file_size: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = fs::File::create(path)?;
    let mut reader = BufReader::new(stream);
    let mut received_bytes: u64 = 0;
    let mut buffer = [0; 8192]; // 8KB buffer

    while received_bytes < file_size {
        let bytes_to_read = std::cmp::min(buffer.len() as u64, file_size - received_bytes) as usize;
        let bytes_read = reader.read(&mut buffer[..bytes_to_read])?;

        if bytes_read == 0 {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Connection closed prematurely",
            )));
        }

        file.write_all(&buffer[..bytes_read])?;
        received_bytes += bytes_read as u64;
    }

    Ok(())
}
