use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub visible: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self { x: 100, y: 100, visible: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub db_path: Option<String>,
    #[serde(default = "default_poll")]
    pub poll_interval_sec: u64,
    #[serde(default = "default_rate")]
    pub usd_to_cny: f64,
    #[serde(default)]
    pub overlay: WindowState,
    #[serde(default = "default_detail")]
    pub detail: WindowState,
}

fn default_poll() -> u64 { 3 }
fn default_rate() -> f64 { 7.2 }
fn default_detail() -> WindowState { WindowState { x: 800, y: 400, visible: false } }

impl Default for Config {
    fn default() -> Self {
        Config {
            db_path: None,
            poll_interval_sec: 3,
            usd_to_cny: 7.2,
            overlay: WindowState { x: 1600, y: 40, visible: true },
            detail: WindowState { x: 800, y: 400, visible: false },
        }
    }
}

impl Config {
    pub fn from_json(text: &str) -> Config {
        serde_json::from_str(text).unwrap_or_else(|_| Config::default())
    }
}

pub fn load(app: &AppHandle) -> Config {
    match app.path().app_config_dir() {
        Ok(dir) => {
            let p = dir.join("config.json");
            std::fs::read_to_string(&p).map(|t| Config::from_json(&t)).unwrap_or_default()
        }
        Err(_) => Config::default(),
    }
}

pub fn save(app: &AppHandle, cfg: &Config) {
    if let Ok(dir) = app.path().app_config_dir() {
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(text) = serde_json::to_string_pretty(cfg) {
            let _ = std::fs::write(dir.join("config.json"), text);
        }
    }
}

pub fn resolve_db_path(app: &AppHandle) -> Option<PathBuf> {
    let cfg = load(app);
    if let Some(p) = cfg.db_path {
        let pb = PathBuf::from(p);
        if pb.exists() { return Some(pb); }
    }
    let default = home_cc_switch_db()?;
    if default.exists() { Some(default) } else { None }
}

fn home_cc_switch_db() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")?;
    Some(PathBuf::from(home).join(".cc-switch").join("cc-switch.db"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_json_missing_file_returns_default() {
        let cfg = Config::from_json("");
        assert_eq!(cfg.poll_interval_sec, 3);
        assert!((cfg.usd_to_cny - 7.2).abs() < 1e-9);
        assert!(cfg.db_path.is_none());
    }

    #[test]
    fn from_json_roundtrip() {
        let cfg = Config { db_path: Some("C:/x.db".into()), poll_interval_sec: 5, usd_to_cny: 7.0,
            overlay: WindowState { x: 10, y: 20, visible: true },
            detail: WindowState { x: 30, y: 40, visible: false } };
        let text = serde_json::to_string(&cfg).unwrap();
        let back = Config::from_json(&text);
        assert_eq!(back.db_path.as_deref(), Some("C:/x.db"));
        assert_eq!(back.poll_interval_sec, 5);
        assert!((back.usd_to_cny - 7.0).abs() < 1e-9);
        assert_eq!(back.overlay.x, 10);
        assert_eq!(back.detail.visible, false);
    }

    #[test]
    fn from_json_partial_uses_defaults() {
        let text = r#"{"usd_to_cny":6.5}"#;
        let cfg = Config::from_json(text);
        assert!((cfg.usd_to_cny - 6.5).abs() < 1e-9);
        assert_eq!(cfg.poll_interval_sec, 3); // defaulted
    }

    #[test]
    fn from_json_corrupt_returns_default() {
        let cfg = Config::from_json("{not valid json");
        assert_eq!(cfg.poll_interval_sec, 3);
    }
}