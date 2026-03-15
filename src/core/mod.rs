use std::sync::LazyLock;
use libstrawberry::localization::Localization;
use crate::core::config::ConfigManager;
use crate::core::constants::LOGGER;

pub mod constants;
pub mod notifier;
pub mod device;
pub mod net;
pub mod config;
pub mod file;

pub static I18N: LazyLock<Localization> = LazyLock::new(|| {
    let config_manager = ConfigManager::new();
    let config_lang = config_manager.config.general.language.clone();

    // Determine language to use
    let lang_code = if let Some(lang) = config_lang {
        lang
    } else {
        std::env::var("LANG")
            .unwrap_or_else(|_| "en_US".to_string())
            .split('.')
            .next()
            .unwrap_or("en_US")
            .to_string()
    };

    // Helper to load bundled language strings
    let load_embedded = |code: &str| -> Option<String> {
        match code {
            "de_DE" => Some(include_str!("../i18n/de_DE.yml").to_string()),
            "en_US" => Some(include_str!("../i18n/en_US.yml").to_string()),
            _ => None
        }
    };

    // Try selected language
    let strings = load_embedded(&lang_code)
        // Fallback to "en_US" if selected fails
        .or_else(|| {
            LOGGER.error(format!("Language '{}' not available, falling back to en_US.yml", lang_code));
            load_embedded("en_US")
        })
        // Fallback to "de_DE" if en_US fails (just in case)
        .or_else(|| {
             load_embedded("de_DE")
        })
        .expect("Critical: No embedded language files found!");
    
    Localization::new(
        &lang_code,
        &strings,
        true
    )
});