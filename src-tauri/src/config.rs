use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_interval() -> u32 {
    30
}
fn default_keep() -> u32 {
    20
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub save_dir: String,
    pub backup_dir: String,
    pub language: String,
    #[serde(default)]
    pub auto_backup_enabled: bool,
    #[serde(default = "default_interval")]
    pub auto_backup_interval_min: u32,
    #[serde(default = "default_keep")]
    pub auto_backup_keep: u32,
}

pub fn appdata_dir() -> Option<PathBuf> {
    std::env::var("APPDATA").ok().map(PathBuf::from)
}

pub fn config_dir() -> Option<PathBuf> {
    appdata_dir().map(|d| d.join("turing-complete-manager"))
}

pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.json"))
}

pub fn ensure_config_dir() -> Result<PathBuf, String> {
    let dir = config_dir().ok_or_else(|| "APPDATA 环境变量未设置".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
    Ok(dir)
}

pub fn load() -> Option<AppConfig> {
    let path = config_path()?;
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save(cfg: &AppConfig) -> Result<(), String> {
    let dir = ensure_config_dir()?;
    let path = dir.join("config.json");
    let bytes = serde_json::to_vec_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| format!("写入配置失败: {e}"))
}

pub fn default_save_dir() -> Option<PathBuf> {
    appdata_dir().map(|d| d.join("Turing Complete"))
}

pub fn detect_install_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from))
}