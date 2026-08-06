use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct AppConfig {
    save_dir: String,
    backup_dir: String,
    language: String,
}

#[derive(Serialize)]
struct DetectSaveDir {
    default_path: String,
    exists: bool,
}

fn appdata_dir() -> Option<PathBuf> {
    std::env::var("APPDATA").ok().map(PathBuf::from)
}

fn config_dir() -> Option<PathBuf> {
    appdata_dir().map(|d| d.join("turing-complete-manager"))
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.json"))
}

fn ensure_config_dir() -> Result<PathBuf, String> {
    let dir = config_dir().ok_or_else(|| "APPDATA 环境变量未设置".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
    Ok(dir)
}

fn load_config() -> Option<AppConfig> {
    let path = config_path()?;
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn default_save_dir() -> Option<PathBuf> {
    appdata_dir().map(|d| d.join("Turing Complete"))
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn detect_save_dir() -> DetectSaveDir {
    let dir = default_save_dir();
    let default_path = dir
        .as_deref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let exists = dir.is_some_and(|p| p.exists());
    DetectSaveDir { default_path, exists }
}

#[tauri::command]
fn get_config() -> Option<AppConfig> {
    load_config()
}

#[tauri::command]
fn set_config(cfg: AppConfig) -> Result<(), String> {
    let dir = ensure_config_dir()?;
    let path = dir.join("config.json");
    let bytes = serde_json::to_vec_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| format!("写入配置失败: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            app_version,
            detect_save_dir,
            get_config,
            set_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}