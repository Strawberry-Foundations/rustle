use std::env;

use crate::commands::{daemon::start_daemon, scan::scan_services, send::send_file};

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
