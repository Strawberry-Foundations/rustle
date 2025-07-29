use std::{env, sync::{Arc, Mutex}};

use crate::{
    commands::{daemon::Daemon, gui::show_send_dialog, scan::scan_services, send::send_file},
    core::{constants::CFG, device::{discover_devices_continuously, DiscoveredDevice}},
};

pub mod commands;
pub mod core;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        commands::help::help();
        return;
    }

    match args[1].as_str() {
        "daemon" => {
            let daemon = Daemon::new(CFG.config.network.port, "localhost".to_string());
            daemon.start().await;
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
        "gui" => {
            if args.len() < 3 {
                println!("Error: File path required for GUI");
                println!("Usage: {} gui <file_path>", args[0]);
                return;
            }
            
            let file_path = args[2].clone();
            
            // Create shared device list
            let devices = Arc::new(Mutex::new(Vec::<DiscoveredDevice>::new()));
            let devices_clone = Arc::clone(&devices);
            
            // Start continuous discovery in background
            tokio::spawn(async move {
                discover_devices_continuously(devices_clone).await;
            });
            
            // Show GUI immediately (will be empty initially)
            show_send_dialog(devices, file_path);
        }
        _ => {
            println!("Error: Unknown command '{}'", args[1]);
            commands::help::help();
        }
    }
}
