use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use crate::core::constants::LOGGER;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub network: NetworkConfig,
    pub general: GeneralConfig,
    #[serde(default)]
    pub user: UserConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkConfig {
    pub port: u16,
    pub discovery_timeout_secs: u64,
    pub static_peers: Option<Vec<String>>,
    pub scan_subnets: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneralConfig {
    pub default_download_path: String,
    pub show_notifications: bool,
    pub language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserConfig {
    pub display_name: Option<String>,
    pub avatar_path: Option<String>,
}

impl Default for UserConfig {
    fn default() -> Self {
        UserConfig {
            display_name: Some(whoami::hostname().unwrap()),
            avatar_path: None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            network: NetworkConfig {
                port: 49242,
                discovery_timeout_secs: 5,
                static_peers: Some(vec![]),
                scan_subnets: Some(vec![]),
            },
            general: GeneralConfig {
                default_download_path: "~/Downloads".to_string(),
                show_notifications: true,
                language: None,
            },
            user: UserConfig::default(),
        }
    }
}

pub struct ConfigManager {
    pub config: Config,
    _config_path: PathBuf,
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigManager {
    pub fn new() -> Self {
        let config_path = Self::get_config_path().expect("Could not find a valid home directory.");
        let config = if config_path.exists() {
            Self::load_from_file(&config_path)
        } else {
            LOGGER.warning(format!("No configuration file found. Creating a new one at: {:?}", &config_path));
            Self::create_default_config(&config_path)
        };
        ConfigManager { config, _config_path: config_path }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let yaml = serde_yaml::to_string(&self.config).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        fs::write(&self._config_path, yaml)
    }

    fn load_from_file(path: &Path) -> Config {
        match fs::read_to_string(path) {
            Ok(content) => {
                match serde_yaml::from_str(&content) {
                    Ok(config) => {
                        LOGGER.info(format!("Configuration successfully loaded from {path:?}."));
                        config
                    },
                    Err(e) => {
                        LOGGER.error(format!("Error parsing configuration file: {e}. Loading defaults."));
                        Config::default()
                    }
                }
            },
            Err(e) => {
                LOGGER.error(format!("Error reading configuration file: {e}. Loading defaults."));
                Config::default()
            }
        }
    }

    fn create_default_config(path: &Path) -> Config {
        let config = Config::default();
        if let Some(parent) = path.parent()
            && let Err(e) = fs::create_dir_all(parent) {
                LOGGER.error(format!("Could not create configuration directory: {e}"));
                return config;
            }
        match serde_yaml::to_string(&config) {
            Ok(yaml) => {
                if let Err(e) = fs::write(path, yaml) {
                    LOGGER.error(format!("Error writing default configuration: {e}"));
                } else {
                    LOGGER.info(format!("Default configuration successfully saved to {path:?}."));
                }
            },
            Err(e) => {
                LOGGER.error(format!("Error serializing default configuration: {e}"));
            }
        }
        config
    }

    fn get_config_path() -> Option<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "Rustle", "Rustle") {
            let config_dir = proj_dirs.config_dir();
            Some(config_dir.join("config.yaml"))
        } else {
            None
        }
    }
}
