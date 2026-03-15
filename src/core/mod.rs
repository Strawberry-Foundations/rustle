use std::sync::LazyLock;
use libstrawberry::localization::Localization;
use crate::core::config::ConfigManager;
use crate::core::constants::LOGGER;
use crate::core::file::get_language_strings;

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

    // Helper to try loading a file safely
    let try_load = |code: &str| -> Option<String> {
        std::panic::catch_unwind(|| get_language_strings(code)).ok()
    };

    // Try selected language
    let strings = try_load(&lang_code)
        // Fallback to "en_US" if selected fails
        .or_else(|| {
            LOGGER.error(format!("Could not load language file for '{}', falling back to en_US.yml", lang_code));
            try_load("en_US")
        })
        // Fallback to "de_DE" if en_US fails (just in case)
        .or_else(|| {
             try_load("de_DE")
        })
        .expect("Critical: No language files found in src/i18n/");
    
    Localization::new(
        &lang_code,
        &strings,
        true
    )
});