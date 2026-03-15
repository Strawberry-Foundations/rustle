use crate::core::{config::ConfigManager, constants::LOGGER, notifier::show_transfer_notification};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
};
use tokio::task;

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
        let instance_name = "Rustle";
        let hostname = format!(
            "{}.local.",
            whoami::hostname().unwrap_or_else(|_| "localhost".to_string())
        );

        let properties = [("path", "/"), ("version", "1.0")];

        let service_info = ServiceInfo::new(
            service_type,
            instance_name,
            &hostname,
            "",
            self.port,
            &properties[..],
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
            }
            Err(e) => {
                LOGGER.error(format!("Failed to receive file: {e}"));
                stream.write_all(format!("ERROR|{e}\n").as_bytes())?;
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
