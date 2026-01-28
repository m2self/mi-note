use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub account_cookie: Option<String>,
    pub user_agent: Option<String>,
    pub hotkey: String,       // e.g. "Alt-L"
    pub destination: String,  // "Clipboard" or "PreviousProgram"
    pub theme: String,        // "Desktop" or "Mobile"
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            account_cookie: None,
            user_agent: None,
            hotkey: "Alt-L".to_string(),
            destination: "Clipboard".to_string(),
            theme: "Mobile".to_string(),
        }
    }
}

impl AppConfig {
    fn get_config_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "minote", "MiNoteWebView")
            .map(|proj_dirs| proj_dirs.config_dir().join("config.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::get_config_path() {
            if let Ok(content) = fs::read_to_string(path) {
                return serde_json::from_str(&content).unwrap_or_default();
            }
        }
        Self::default()
    }

    pub fn save(&self) -> crate::api::MiResult<()> {
        if let Some(path) = Self::get_config_path() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = serde_json::to_string_pretty(self)?;
            fs::write(path, content)?;
        }
        Ok(())
    }
}
