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

use crate::core::config::ConfigManager;
use crate::core::device::DiscoveredDevice;
use crate::core::I18N;

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
    language: String, // "en_US" or "de_DE" etc.
    status_msg: Option<String>,
}

impl SettingsDialog {
    fn new() -> Self {
        let config_manager = ConfigManager::new();
        let config = &config_manager.config;
        
        let language = config.general.language.clone().unwrap_or_else(|| {
             std::env::var("LANG")
                .unwrap_or_else(|_| "en_US.UTF-8".to_string())
                .split('.')
                .next()
                .unwrap_or("en_US")
                .to_string()
        });

        Self {
            display_name: config.user.display_name.clone().unwrap_or_default(),
            avatar_path: config.user.avatar_path.clone().unwrap_or_default(),
            download_path: config.general.default_download_path.clone(),
            show_notifications: config.general.show_notifications,
            language,
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
        self.config_manager.config.general.language = Some(self.language.clone());

        match self.config_manager.save() {
            Ok(_) => self.status_msg = Some(I18N.get("settings_saved_msg")),
            Err(e) => self.status_msg = Some(I18N.get_with_params("settings_error_msg", &[&e.to_string()])),
        }
    }
}

impl eframe::App for SettingsDialog {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(I18N.get("settings_title"));
            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label(I18N.get("settings_profile"));
                ui.horizontal(|ui| {
                    ui.label(I18N.get("settings_display_name"));
                    ui.text_edit_singleline(&mut self.display_name);
                });
                ui.horizontal(|ui| {
                    ui.label(I18N.get("settings_avatar_path"));
                    ui.text_edit_singleline(&mut self.avatar_path);
                });
                ui.small(I18N.get("settings_avatar_hint"));
            });

            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label(I18N.get("settings_general"));
                ui.horizontal(|ui| {
                    ui.label(I18N.get("settings_download_path"));
                    ui.text_edit_singleline(&mut self.download_path);
                });
                ui.checkbox(&mut self.show_notifications, I18N.get("settings_notifications"));
                
                ui.horizontal(|ui| {
                     ui.label(I18N.get("settings_language"));
                     let current = self.language.clone();
                     egui::ComboBox::from_id_salt("language_select")
                         .selected_text(&current)
                         .show_ui(ui, |ui| {
                             ui.selectable_value(&mut self.language, "en_US".to_string(), "English (en_US)");
                             ui.selectable_value(&mut self.language, "de_DE".to_string(), "German (de_DE)");
                         });
                });
            });

            ui.add_space(20.0);

            if ui.button(I18N.get("settings_save_btn")).clicked() {
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
    status_msg: Arc<Mutex<String>>,
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
            status_msg: Arc::new(Mutex::new(String::new())),
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
                ui.heading(I18N.get("send_file_title"));
                ui.label(I18N.get_with_params("send_file_name", &[&self.file_name]));
            });
            ui.add_space(20.0);

            let devices_guard = self.devices.lock().unwrap();
            if devices_guard.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.spinner();
                    ui.add_space(10.0);
                    ui.label(I18N.get("send_searching"));
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
                                
                                // Draw Avatar
                                if let Some(texture) = self.texture_cache.get(&device_id) {
                                    // Use Image widget for easier handling of aspect ratio and rounding
                                    let image = egui::Image::new(texture)
                                        .fit_to_exact_size(rect.size())
                                        .corner_radius(radius as u8);
                                    
                                    // Place the image in the rect
                                    ui.put(rect, image);
                                    
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

            // Drop the lock explicitly to avoid deadlock when clicking the button below
            drop(devices_guard);

            ui.add_space(20.0);
            if let Ok(msg) = self.status_msg.lock() {
                if !msg.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new(msg.clone()).strong().size(14.0));
                    });
                    ui.add_space(10.0);
                }
            }

            ui.vertical_centered(|ui| {
                ui.add_enabled_ui(self.selected.is_some(), |ui| {
                    if ui.button(I18N.get("send_btn")).clicked() {
                        if let Some(idx) = self.selected {
                            let devices = self.devices.lock().unwrap();
                            if let Some(device) = devices.get(idx) {
                                let device_clone = device.clone();
                                let path_clone = self.file_path.clone(); // Clone PathBuf? String.
                                let status = self.status_msg.clone();
                                
                                // Reset status
                                if let Ok(mut msg) = status.lock() {
                                    *msg = I18N.get("send_status_start");
                                }
                                
                                // Spawn send task
                                std::thread::spawn(move || {
                                    // Use a catch_unwind to prevent the thread from crashing silently
                                    let result = std::panic::catch_unwind(|| {
                                        let res = send_file_sync(&device_clone, &path_clone, &status);
                                        match res {
                                            Ok(_) => I18N.get("send_status_complete"),
                                            Err(e) => I18N.get_with_params("send_status_error", &[&e.to_string()]),
                                        }
                                    });

                                    // Update status based on result
                                    if let Ok(mut msg) = status.lock() {
                                        *msg = match result {
                                            Ok(success_msg) => success_msg,
                                            Err(_) => I18N.get("send_status_panic"),
                                        };
                                    }
                                });
                            }
                        }
                    }
                });
            });
        });
    }
}



fn send_file_sync(device: &DiscoveredDevice, file_path_str: &str, status: &Arc<Mutex<String>>) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = Path::new(file_path_str);
    if !file_path.exists() {
        return Err("File does not exist".into());
    }

    let file_metadata = fs::metadata(file_path)?;
    let file_size = file_metadata.len();
    let file_name = file_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid filename")?;

    let update_status = |msg: &str| {
        if let Ok(mut s) = status.lock() {
            *s = msg.to_string();
        }
    };

    update_status(&I18N.get_with_params("send_status_connect", &[&device.name, &device.ip, &device.port.to_string()]));

    // Connect with timeout
    // Handle IPv6 properly by wrapping in brackets if needed
    let addr_str = if device.ip.contains(':') {
        format!("[{}]:{}", device.ip, device.port)
    } else {
        format!("{}:{}", device.ip, device.port)
    };
    
    let addr: std::net::SocketAddr = addr_str.parse()?;
    let mut stream = TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5))?;
    
    stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;

    let sender_name = whoami::hostname().unwrap_or_else(|_| "Unknown".to_string());
    
    update_status(&I18N.get("send_status_req"));
    let request = format!("TRANSFER_REQUEST|{sender_name}|{file_name}|{file_size}\n");
    stream.write_all(request.as_bytes())?;

    update_status(&I18N.get("send_status_wait_acc"));
    
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut response = String::new();
    reader.read_line(&mut response)?;

    if response.trim() != "ACCEPTED" {
         return Err(I18N.get("send_status_declined").into());
    }

    update_status(&I18N.get("send_status_data"));
    let mut file = fs::File::open(file_path)?;
    
    std::io::copy(&mut file, &mut stream)?;
    
    // Flush
    stream.flush()?;

    update_status(&I18N.get("send_status_wait_conf"));
    let mut ack = String::new();
    reader.read_line(&mut ack)?;
    
    if ack.trim() == "TRANSFER_COMPLETE" {
        Ok(())
    } else {
        Err(I18N.get_with_params("send_status_error", &[&ack.trim()]).into())
    }
}
