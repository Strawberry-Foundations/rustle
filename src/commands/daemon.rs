use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
};
use tokio::task;

use crate::core::{constants::LOGGER, notifier::show_transfer_notification};

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
            LOGGER.panic_crash(format!("Failed to bind to port {}: {e}", self.port));
        });

        let mdns = ServiceDaemon::new().unwrap_or_else(|e| {
            LOGGER.panic_crash(format!("Failed to create daemon: {e}"));
        });

        let service_type = "_rustle._tcp.local.";
        let instance_name = "Rustle";
        let hostname = format!(
            "{}.local.",
            whoami::fallible::hostname().unwrap_or_else(|_| "localhost".to_string())
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
            LOGGER.panic_crash(format!("Failed to create service info: {e}"));
        })
        .enable_addr_auto();

        mdns.register(service_info).unwrap_or_else(|e| {
            LOGGER.panic_crash(format!("Failed to register service: {e}"));
        });

        LOGGER.default("Daemon started successfully");

        LOGGER.info("Service discovery started");
        LOGGER.info(format!("Hostname: {}", hostname.trim_end_matches('.')));
        LOGGER.info("Listening for incoming connections on port 49242");

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
    if parts.len() != 3 || parts[0] != "TRANSFER_REQUEST" {
        stream.write_all(b"ERROR|Invalid request format\n")?;
        return Ok(());
    }

    let sender_name = parts[1];
    let file_name = parts[2];

    LOGGER.info(format!(
        "Transfer request from {sender_name} for file '{file_name}'"
    ));

    let accepted = show_transfer_notification(sender_name, file_name)?;

    if accepted {
        stream.write_all(b"ACCEPTED\n")?;
        LOGGER.info("Transfer accepted");
    } else {
        stream.write_all(b"DECLINED\n")?;
        LOGGER.info("Transfer declined");
    }

    Ok(())
}
