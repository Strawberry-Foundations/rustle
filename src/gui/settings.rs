use eframe::{Frame, NativeOptions};
use egui::Color32;
use crate::core::config::{ConfigManager};
use crate::core::I18N;
use crate::gui::{configure_fonts, configure_styles};

pub struct SettingsDialog {
    config_manager: ConfigManager,
    display_name: String,
    avatar_path: String,
    download_path: String,
    show_notifications: bool,
    language: String,
    status_msg: Option<String>,
}

impl SettingsDialog {
    pub fn run() {
        let options = NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([450.0, 500.0])
                .with_resizable(false),
            ..Default::default()
        };



        if let Err(e) = eframe::run_native(
            I18N.get("settings_title").as_str(),
            options,
            Box::new(|cc| {
                configure_fonts(&cc.egui_ctx);
                configure_styles(&cc.egui_ctx);
                
                Ok(Box::new(SettingsDialog::new()))
            }),
        ) {
            eprintln!("Error running settings GUI: {e}");
        }

        // Ensure Clean Exit
        std::process::exit(0);
    }

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
            ui.vertical_centered(|ui| {
                ui.heading(
                    egui::RichText::new(
                        I18N.get("settings_title")
                    )
                        .size(24.0)
                        .family(egui::FontFamily::Name("Heading".into()))
                        .color(Color32::WHITE)
                );
            });
            ui.add_space(20.0);

            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(I18N.get("settings_profile")).strong().size(16.0));
                    ui.add_space(5.0);
                    
                    egui::Grid::new("profile_grid")
                        .num_columns(2)
                        .spacing([15.0, 10.0])
                        .show(ui, |ui| {
                            ui.label(I18N.get("settings_display_name"));
                            ui.text_edit_singleline(&mut self.display_name);
                            ui.end_row();

                            ui.label(I18N.get("settings_avatar_path"));
                            ui.text_edit_singleline(&mut self.avatar_path);
                            ui.end_row();
                        });

                    ui.add_space(5.0);
                    ui.label(egui::RichText::new(I18N.get("settings_avatar_hint")).small().italics());
                });
            });

            ui.add_space(15.0);

            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(I18N.get("settings_general")).strong().size(16.0));
                    ui.add_space(5.0);

                    egui::Grid::new("general_grid")
                        .num_columns(2)
                        .spacing([15.0, 10.0])
                        .show(ui, |ui| {
                            ui.label(I18N.get("settings_download_path"));
                            ui.text_edit_singleline(&mut self.download_path);
                            ui.end_row();

                            ui.label(I18N.get("settings_language"));
                            let current = self.language.clone();
                            egui::ComboBox::from_id_salt("language_select")
                                .selected_text(&current)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.language, "en_US".to_string(), "English (en_US)");
                                    ui.selectable_value(&mut self.language, "de_DE".to_string(), "German (de_DE)");
                                });
                            ui.end_row();
                        });

                    ui.add_space(10.0);
                    ui.checkbox(&mut self.show_notifications, I18N.get("settings_notifications"));
                });
            });

            ui.add_space(25.0);

            ui.vertical_centered(|ui| {
                if ui.add(egui::Button::new(egui::RichText::new(I18N.get("settings_save_btn")).size(16.0)).min_size(egui::vec2(150.0, 40.0))).clicked() {
                    self.save();
                }

                if let Some(msg) = &self.status_msg {
                    ui.add_space(10.0);
                    
                    if msg.contains("Error") || msg.contains("Fehler") {
                         ui.label(egui::RichText::new(msg).color(Color32::from_rgb(200, 50, 50)).strong());
                    } else {
                         ui.label(egui::RichText::new(msg).color(Color32::from_rgb(50, 150, 50)).strong());
                    }
                }
            });
        });
    }
}