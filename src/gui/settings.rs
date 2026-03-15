use eframe::{Frame, NativeOptions};
use crate::core::config::ConfigManager;
use crate::core::I18N;

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