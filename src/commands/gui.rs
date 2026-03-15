use eframe::{Frame, NativeOptions, egui};
use egui::{Color32, Sense, Stroke, vec2, TextureHandle, TextureOptions};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
};

use crate::commands::send::stream_file_data;
use crate::core::config::ConfigManager;
use crate::core::constants::LOGGER;
use crate::core::device::DiscoveredDevice;

pub fn show_settings_dialog() {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 500.0])
            .with_resizable(false),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "Rustle Settings",
        options,
        Box::new(|_cc| Ok(Box::new(SettingsDialog::new()))),
    ) {
        eprintln!("Error running settings GUI: {e}");
    }
}

pub fn show_send_dialog(devices: Arc<Mutex<Vec<DiscoveredDevice>>>, file_path: String) {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 420.0]) // Slightly larger for better grid
            .with_resizable(true),
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

struct SettingsDialog {
    config_manager: ConfigManager,
    display_name: String,
    avatar_path: String,
    download_path: String,
    show_notifications: bool,
    status_msg: Option<String>,
}

impl SettingsDialog {
    fn new() -> Self {
        let config_manager = ConfigManager::new();
        let config = &config_manager.config;
        
        Self {
            display_name: config.user.display_name.clone().unwrap_or_default(),
            avatar_path: config.user.avatar_path.clone().unwrap_or_default(),
            download_path: config.general.default_download_path.clone(),
            show_notifications: config.general.show_notifications,
            config_manager,
            status_msg: None,
        }
    }

    fn save(&mut self) {
        self.config_manager.config.user.display_name = if self.display_name.trim().is_empty() {
            None
        } else {
            Some(self.display_name.clone())
        };

        self.config_manager.config.user.avatar_path = if self.avatar_path.trim().is_empty() {
            None
        } else {
            Some(self.avatar_path.clone())
        };

        self.config_manager.config.general.default_download_path = self.download_path.clone();
        self.config_manager.config.general.show_notifications = self.show_notifications;

        match self.config_manager.save() {
            Ok(_) => self.status_msg = Some("Settings saved successfully!".to_string()),
            Err(e) => self.status_msg = Some(format!("Error saving settings: {}", e)),
        }
    }
}

impl eframe::App for SettingsDialog {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Rustle Settings");
            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label("User Profile");
                ui.horizontal(|ui| {
                    ui.label("Display Name:");
                    ui.text_edit_singleline(&mut self.display_name);
                });
                ui.horizontal(|ui| {
                    ui.label("Avatar Path:");
                    ui.text_edit_singleline(&mut self.avatar_path);
                });
                ui.small("Use a local file path (e.g., /home/user/pic.png).");
            });

            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label("General");
                ui.horizontal(|ui| {
                    ui.label("Download Path:");
                    ui.text_edit_singleline(&mut self.download_path);
                });
                ui.checkbox(&mut self.show_notifications, "Show Notifications");
            });

            ui.add_space(20.0);

            if ui.button("Save Settings").clicked() {
                self.save();
            }

            if let Some(msg) = &self.status_msg {
                ui.add_space(10.0);
                ui.label(msg);
            }
        });
    }
}

struct SendDialog {
    devices: Arc<Mutex<Vec<DiscoveredDevice>>>,
    selected: Option<usize>,
    file_path: String,
    file_name: String,
    texture_cache: HashMap<String, TextureHandle>,
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
            texture_cache: HashMap::new(),
        }
    }

    fn load_texture(&mut self, ctx: &egui::Context, device_id: &str, data: &[u8]) {
        if self.texture_cache.contains_key(device_id) {
            return;
        }

        if let Ok(image) = image::load_from_memory(data) {
            let size = [image.width() as usize, image.height() as usize];
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                size,
                pixels.as_slice(),
            );

            let texture = ctx.load_texture(
                device_id,
                color_image,
                TextureOptions::default()
            );
            self.texture_cache.insert(device_id.to_string(), texture);
        }
    }
}

impl eframe::App for SendDialog {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // Auto-refresh to find devices
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        // 1. Pre-load textures for any devices that have avatar_data but no texture yet.
        //    We must do this outside the closure to avoid double-borrowing `self`.
        let mut to_load = Vec::new();
        {
            let devices = self.devices.lock().unwrap();
            for dev in devices.iter() {
                let id = format!("{}:{}", dev.hostname, dev.ip);
                if dev.avatar_data.is_some() && !self.texture_cache.contains_key(&id) {
                    to_load.push((id, dev.avatar_data.clone().unwrap()));
                }
            }
        }
        for (id, data) in to_load {
            self.load_texture(ctx, &id, &data);
        }

        // 2. Render UI
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                ui.heading("Send File");
                ui.label(format!("File: {}", self.file_name));
            });
            ui.add_space(20.0);

            let devices_guard = self.devices.lock().unwrap();
            if devices_guard.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.spinner();
                    ui.add_space(10.0);
                    ui.label("Searching for devices...");
                });
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(20.0, 20.0);
                        
                        for (idx, device) in devices_guard.iter().enumerate() {
                            let device_id = format!("{}:{}", device.hostname, device.ip);
                            let is_selected = self.selected == Some(idx);
                            
                            // Each device is a vertical group
                            ui.vertical(|ui| {
                                ui.set_min_width(100.0);
                                ui.set_max_width(100.0);
                                
                                let (response, painter) = ui.allocate_painter(vec2(80.0, 80.0), Sense::click());
                                
                                if response.clicked() {
                                    self.selected = Some(idx);
                                }

                                let rect = response.rect;
                                let center = rect.center();
                                let radius = rect.width() / 2.0;

                                // Background for avatar
                                let bg_color = if is_selected {
                                    Color32::from_rgb(0, 120, 215) // Highlight
                                } else {
                                    Color32::from_gray(220) // Light gray
                                };
                                
                                // Draw Avatar or Fallback
                                if let Some(texture) = self.texture_cache.get(&device_id) {
                                    // Draw texture with rounding (circular)
                                    // egui::Painter doesn't have a direct "image_with_rounding" easily accessible 
                                    // without a specific shader or mesh. 
                                    // Just drawing a square image inside the circle for now 
                                    // or using an Image widget would have been easier if we weren't doing manual layout.
                                    // Let's use a square image clipped by a circle? 
                                    // Actually, egui Images support rounding.
                                    // But we allocated a painter. 
                                    // Let's just draw the image rect.
                                    painter.image(
                                        texture.id(),
                                        rect,
                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                        Color32::WHITE
                                    );
                                    
                                    // Draw selection ring
                                    if is_selected {
                                         painter.circle_stroke(center, radius + 4.0, Stroke::new(3.0, bg_color));
                                    }
                                } else {
                                    // Fallback: Circle with initials
                                    painter.circle_filled(center, radius, bg_color);
                                    if is_selected {
                                        painter.circle_stroke(center, radius + 2.0, Stroke::new(2.0, Color32::WHITE));
                                    }
                                    
                                    // Initial
                                    let initial = device.name.chars().next()
                                        .unwrap_or('?')
                                        .to_string()
                                        .to_uppercase();
                                    
                                    painter.text(
                                        center,
                                        egui::Align2::CENTER_CENTER,
                                        initial,
                                        egui::FontId::proportional(32.0),
                                        Color32::WHITE,
                                    );
                                }

                                ui.add_space(5.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new(
                                        &device.name
                                    ).strong().size(14.0));
                                    
                                    // Show IP as subtext
                                    ui.label(egui::RichText::new(&device.ip).size(10.0).color(Color32::GRAY));
                                });
                            });
                        }
                    });
                });
            }

            // Send Button
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.add_enabled_ui(self.selected.is_some(), |ui| {
                    if ui.button("Send File").clicked() {
                        if let Some(idx) = self.selected {
                            let devices = self.devices.lock().unwrap();
                            if let Some(device) = devices.get(idx) {
                                let device_clone = device.clone();
                                let path_clone = self.file_path.clone();
                                
                                // Spawn send task
                                std::thread::spawn(move || {
                                    let rt = tokio::runtime::Runtime::new().unwrap();
                                    rt.block_on(async {
                                        send_file_to_device(&device_clone, &path_clone).await;
                                    });
                                });
                                
                                // Close window or show success state? 
                                // For now, just close or stay open. 
                                // Let's keep it open to show log?
                                // Actually we don't have log UI here.
                                // Just print to terminal for now.
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    }
                });
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
