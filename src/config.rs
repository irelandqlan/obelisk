use crate::backend::auth::microsoft::Account;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PreferredViewType {
    #[default]
    Grid,
    List,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    #[default]
    Alphabetical,
    LastPlayed,
    Playtime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    pub instances_path: Option<PathBuf>,
    #[serde(alias = "fallback_data_path")]
    pub shared_data_path: Option<PathBuf>,
    pub minecraft_data_path: PathBuf,
    pub java_path: Option<PathBuf>,
    pub max_memory: u32,
    pub min_memory: u32,
    pub microsoft_client_id: Option<String>,
    #[serde(default)]
    pub accounts: Vec<Account>,
    pub active_account_uuid: Option<String>,
    /// Path to the global default instance icon image.
    #[serde(default)]
    pub default_instance_icon: Option<PathBuf>,
    /// Total playtime across all instances (persistent even if instances are deleted).
    #[serde(default)]
    pub total_playtime: u64,
    #[serde(default)]
    pub preferred_view_type: PreferredViewType,
    #[serde(default)]
    pub sort_by: SortBy,
    #[serde(skip)]
    pub is_demo: bool,
}

impl Default for Config {
    fn default() -> Self {
        let mc_data = Self::get_data_dir();

        Self {
            instances_path: None,
            shared_data_path: None,
            minecraft_data_path: mc_data,
            java_path: Some(PathBuf::from("java")),
            max_memory: 4096,
            min_memory: 512,
            microsoft_client_id: None,
            accounts: Vec::new(),
            active_account_uuid: None,
            default_instance_icon: None,
            total_playtime: 0,
            preferred_view_type: PreferredViewType::default(),
            sort_by: SortBy::default(),
            is_demo: false,
        }
    }
}

impl Config {
    pub fn get_data_dir() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            let host_obelisk = PathBuf::from(&home).join(".local/share/obelisk");
            if host_obelisk.exists() {
                return host_obelisk;
            }
            let host_legacy = PathBuf::from(&home).join(".local/share/obelisk-launcher");
            if host_legacy.exists() {
                return host_legacy;
            }
        }

        let new_dir = if let Some(base) = directories::BaseDirs::new() {
            base.data_local_dir().join("obelisk")
        } else {
            let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
            home.join(".local/share/obelisk")
        };
        if !new_dir.exists() {
            let legacy_dir = if let Some(base) = directories::BaseDirs::new() {
                base.data_local_dir().join("obelisk-launcher")
            } else {
                let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
                home.join(".local/share/obelisk-launcher")
            };
            if legacy_dir.exists() {
                return legacy_dir;
            }
        }
        new_dir
    }

    pub fn get_cache_dir() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            let host_obelisk = PathBuf::from(&home).join(".cache/obelisk");
            if host_obelisk.exists() {
                return host_obelisk;
            }
            let host_legacy = PathBuf::from(&home).join(".cache/obelisk-launcher");
            if host_legacy.exists() {
                return host_legacy;
            }
        }

        let new_dir = if let Some(base) = directories::BaseDirs::new() {
            base.cache_dir().join("obelisk")
        } else {
            let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
            home.join(".cache/obelisk")
        };
        if !new_dir.exists() {
            let legacy_dir = if let Some(base) = directories::BaseDirs::new() {
                base.cache_dir().join("obelisk-launcher")
            } else {
                let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
                home.join(".cache/obelisk-launcher")
            };
            if legacy_dir.exists() {
                return legacy_dir;
            }
        }
        new_dir
    }

    fn config_path() -> PathBuf {
        // First check directly under host $HOME/.config so Flatpak (running with --filesystem=host)
        // shares and detects the config from the native `cargo run` version rather than isolating it.
        if let Ok(home) = std::env::var("HOME") {
            let host_obelisk = PathBuf::from(&home).join(".config/obelisk/config.json");
            if host_obelisk.exists() {
                return host_obelisk;
            }
            let host_legacy = PathBuf::from(&home).join(".config/obelisk-launcher/config.json");
            if host_legacy.exists() {
                return host_legacy;
            }
        }

        let new_path = if let Some(base) = directories::BaseDirs::new() {
            base.config_dir().join("obelisk").join("config.json")
        } else {
            let mut path = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
            path.push(".config");
            path.push("obelisk");
            path.push("config.json");
            path
        };
        if !new_path.exists() {
            let legacy_path = if let Some(base) = directories::BaseDirs::new() {
                base.config_dir().join("obelisk-launcher").join("config.json")
            } else {
                let mut path = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
                path.push(".config");
                path.push("obelisk-launcher");
                path.push("config.json");
                path
            };
            if legacy_path.exists() {
                return legacy_path;
            }
        }
        new_path
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(config) = serde_json::from_str(&content) {
                    return config;
                }
            }
        }
        Config::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_demo {
            return Ok(());
        }
        let path = Self::config_path();
        let content = serde_json::to_string_pretty(self)?;
        crate::backend::core::fs_utils::atomic_write(&path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert_eq!(config.max_memory, 4096);
        assert_eq!(config.min_memory, 512);
        assert_eq!(config.preferred_view_type, PreferredViewType::Grid);
        assert_eq!(config.sort_by, SortBy::Alphabetical);
        assert!(config.accounts.is_empty());
        assert_eq!(config.total_playtime, 0);
    }

    #[test]
    fn test_config_deserialize_partial() {
        let json = r#"{"minecraft_data_path": "/tmp/mc"}"#;
        let config: Config = serde_json::from_str(json).expect("Failed to deserialize partial config");
        assert_eq!(config.minecraft_data_path, PathBuf::from("/tmp/mc"));
        assert_eq!(config.max_memory, 4096);
        assert_eq!(config.preferred_view_type, PreferredViewType::Grid);
        assert_eq!(config.sort_by, SortBy::Alphabetical);
    }

    #[test]
    fn test_config_legacy_alias() {
        let json = r#"{"minecraft_data_path": "/tmp/mc", "fallback_data_path": "/tmp/shared"}"#;
        let config: Config = serde_json::from_str(json).expect("Failed to deserialize legacy alias");
        assert_eq!(config.shared_data_path, Some(PathBuf::from("/tmp/shared")));
    }

    #[test]
    fn test_config_roundtrip() {
        let mut config = Config::default();
        config.preferred_view_type = PreferredViewType::List;
        config.sort_by = SortBy::LastPlayed;
        config.max_memory = 8192;

        let json = serde_json::to_string(&config).expect("Serialize failed");
        let parsed: Config = serde_json::from_str(&json).expect("Deserialize failed");

        assert_eq!(parsed.preferred_view_type, PreferredViewType::List);
        assert_eq!(parsed.sort_by, SortBy::LastPlayed);
        assert_eq!(parsed.max_memory, 8192);
    }
}
