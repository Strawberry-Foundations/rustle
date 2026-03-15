use eframe::{Frame, NativeOptions, egui};
use egui::{Color32, Sense, Stroke, vec2};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
};

use crate::commands::send::stream_file_data;
use crate::core::constants::LOGGER;
use crate::core::device::DiscoveredDevice;

pub fn show_send_dialog(devices: Arc<Mutex<Vec<DiscoveredDevice>>>, file_path: String) {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 380.0])
            .with_resizable(false),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "Send with Rustle",
        options,
        Box::new(|_cc| Ok(Box::new(SendDialog::new(devices, file_path)))),
    ) {
        eprintln!("Error running native GUI: {e}");
    }
}

struct SendDialog {
    devices: Arc<Mutex<Vec<DiscoveredDevice>>>,
    selected: Option<usize>,
    file_path: String,
    file_name: String,
}

impl SendDialog {
    fn new(devices: Arc<Mutex<Vec<DiscoveredDevice>>>, file_path: String) -> Self {
        let file_name = Path::new(&file_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();

        Self {
            devices,
            selected: None,
            file_path,
            file_name,
        }
    }
}

impl eframe::App for SendDialog {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // Auto-refresh the GUI to show new devices
        ctx.request_repaint();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.heading("Send with Rustle");
                ui.add_space(4.0);
                ui.label(format!("Sending: {}", self.file_name));
                ui.add_space(4.0);
                ui.label("Select a device to send to:");
            });
            ui.add_space(20.0);

            // --- Device Grid ---
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Lock the devices vector for reading
                let devices = self.devices.lock().unwrap();

                if devices.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label("🔍 Searching for devices...");
                        ui.spinner();
                    });
                } else {
                    // Center the grid horizontally
                    let grid_width = 3.0 * 80.0 + 2.0 * ui.style().spacing.item_spacing.x;
                    ui.add_space((ui.available_width() - grid_width) / 2.0);

                    egui::Grid::new("device_grid")
                        .spacing(ui.style().spacing.item_spacing)
                        .show(ui, |ui| {
                            for (i, dev_name) in devices.iter().enumerate() {
                                ui.vertical_centered(|ui| {
                                    let is_selected = self.selected == Some(i);
                                    let (response, painter) =
                                        ui.allocate_painter(vec2(80.0, 80.0), Sense::click());

                                    let rect = response.rect;
                                    let center = rect.center();
                                    let radius = rect.width() / 2.0 - 5.0;

                                    // Draw circle
                                    let bg_color = if is_selected {
                                        Color32::from_rgb(0, 120, 255)
                                    } else {
                                        ctx.style().visuals.widgets.inactive.bg_fill
                                    };
                                    painter.circle_filled(center, radius, bg_color);

                                    // Draw initials
                                    let initials = dev_name
                                        .hostname
                                        .split_whitespace()
                                        .filter_map(|s| s.chars().next())
                                        .take(2)
                                        .collect::<String>()
                                        .to_uppercase();
                                    painter.text(
                                        center,
                                        egui::Align2::CENTER_CENTER,
                                        initials,
                                        egui::FontId::proportional(32.0),
                                        Color32::WHITE,
                                    );

                                    // Draw selection ring
                                    if is_selected {
                                        painter.circle_stroke(
                                            center,
                                            radius + 2.0,
                                            Stroke::new(2.0, Color32::from_rgb(0, 120, 255)),
                                        );
                                    }

                                    if response.clicked() {
                                        self.selected = Some(i);
                                    }

                                    ui.label(&dev_name.hostname);
                                });

                                if (i + 1) % 3 == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                }
            });

            // --- Send Button ---
            // Use a separator and push the button to the bottom
            ui.add_space(ui.available_height() - 60.0);
            ui.separator();
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                let send_button =
                    egui::Button::new("Send").min_size(vec2(ui.available_width() - 32.0, 40.0));

                let is_enabled = self.selected.is_some();
                if ui.add_enabled(is_enabled, send_button).clicked()
                    && let Some(idx) = self.selected {
                        let devices = self.devices.lock().unwrap();
                        if let Some(device) = devices.get(idx) {
                            let device_clone = device.clone();
                            let file_path_clone = self.file_path.clone();

                            // Spawn a background task to send the file
                            tokio::spawn(async move {
                                send_file_to_device(&device_clone, &file_path_clone).await;
                            });

                            LOGGER.info(format!(
                                "Sending {} to {}...",
                                self.file_name, device.hostname
                            ));
                        }
                    }
            });
        });
    }
}

async fn send_file_to_device(device: &DiscoveredDevice, file_path: &str) {
    // Check if file exists
    let file_path_obj = Path::new(file_path);
    if !file_path_obj.exists() {
        LOGGER.error(format!("Error: File '{file_path}' does not exist"));
        return;
    }

    let file_size = match fs::metadata(file_path) {
        Ok(meta) => meta.len(),
        Err(e) => {
            LOGGER.error(format!("Could not read file metadata: {e}"));
            return;
        }
    };

    LOGGER.info(format!(
        "Sending transfer request to {} ({})...",
        device.name, device.ip
    ));

    // Connect to the device
    let mut stream = match TcpStream::connect(format!("{}:{}", device.ip, device.port)) {
        Ok(s) => s,
        Err(e) => {
            LOGGER.error(format!("Failed to connect to {}: {}", device.hostname, e));
            return;
        }
    };

    match send_transfer_request(&mut stream, file_path, file_size).await {
        Ok(accepted) => {
            if accepted {
                LOGGER.info("Transfer accepted! Sending file...");
                match stream_file_data(&mut stream, file_path).await {
                    Ok(_) => LOGGER.info("File transfer completed successfully!"),
                    Err(e) => LOGGER.error(format!("File transfer failed: {e}")),
                }
            } else {
                LOGGER.warning("Transfer was declined by the recipient.");
            }
        }
        Err(e) => {
            LOGGER.error(format!("Error sending transfer request: {e}"));
        }
    }
}

async fn send_transfer_request(
    stream: &mut TcpStream,
    file_path: &str,
    file_size: u64,
) -> Result<bool, String> {
    let sender_name = whoami::hostname().unwrap_or_else(|_| "Unknown".to_string());
    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    let request = format!(
        "TRANSFER_REQUEST|{sender_name}|{file_name}|{file_size}\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;

    let mut response = String::new();
    let mut reader = BufReader::new(stream);
    reader.read_line(&mut response).map_err(|e| e.to_string())?;

    Ok(response.trim() == "ACCEPTED")
}
