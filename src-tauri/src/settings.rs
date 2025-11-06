use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// true = enable notifications, false = disable notifications
    pub notifications_enabled: bool,
    /// true = hide window decorations, false = show window decorations
    pub hide_decorations: bool,
    /// true = show tray, false = hide tray
    pub show_tray: bool,
    /// true = close to tray, false = close to window
    pub close_to_tray: bool,
    /// true = white icon for dark themes, false = dark icon for light themes
    pub tray_icon_light: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            notifications_enabled: true,
            hide_decorations: false,
            show_tray: true,
            close_to_tray: false,
            tray_icon_light: false,
        }
    }
}

impl Settings {
    pub fn load<R: tauri::Runtime>(app: &AppHandle<R>) -> Self {
        let path = Self::get_settings_path(app);
        if let Ok(contents) = fs::read_to_string(&path) {
            serde_json::from_str(&contents).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save<R: tauri::Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        let path = Self::get_settings_path(app);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let contents = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, contents).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn get_settings_path<R: tauri::Runtime>(app: &AppHandle<R>) -> PathBuf {
        app.path()
            .app_config_dir()
            .expect("Failed to get config dir")
            .join("settings.json")
    }
}
