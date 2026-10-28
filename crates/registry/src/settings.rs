use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_REGISTRY: &str = "sonion://registry.sonion.redac.me:6767";
pub const DEFAULT_REGISTRY_KEY: &str = "sNX4JwTy9MXRc+N8n/JfH+QwFyQRHq35RgkHyU12bic=";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub registry: String,
    pub pin_key: Option<String>,
    pub insecure_dev: bool,
    pub homepage: String
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            registry: DEFAULT_REGISTRY.to_string(),
            pin_key: None, 
            insecure_dev: false,
            homepage: "sonion://home.son".to_string()
        }
    }
}

impl Settings {
    pub fn path() -> PathBuf {
        config_dir().join("config.json")
    }
    pub fn load() -> Result<Self, SettingsError> {
        let path = Self::path();
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(SettingsError::Io(e))
        };
        let s: Settings = serde_json::from_str(&raw).map_err(SettingsError::Json)?;
        Ok(s)
    }
    pub fn load_or_create() -> Result<Self, SettingsError> {
        let path = Self::path();
        if !path.exists() {
            let s = Settings::default();
            s.save()?;
            return Ok(s);
        }
        Self::load()
    }
    pub fn save(&self) -> Result<(), SettingsError> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(SettingsError::Io)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(SettingsError::Json)?;
        std::fs::write(&path, json).map_err(SettingsError::Io)
    }
}

fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("sonion")
}

pub fn default_pin_for(registry: &str) -> Option<String> {
    (registry.trim().trim_end_matches('/') == DEFAULT_REGISTRY).then(|| DEFAULT_REGISTRY_KEY.to_string())
}


#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid config json: {0}")]
    Json(#[from] serde_json::Error)
}