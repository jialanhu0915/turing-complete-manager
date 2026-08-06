use serde::Serialize;
use std::path::Path;

mod backup;
mod config;

#[derive(Serialize)]
struct DetectSaveDir {
    default_path: String,
    exists: bool,
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn detect_save_dir() -> DetectSaveDir {
    let dir = config::default_save_dir();
    let default_path = dir
        .as_deref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let exists = dir.is_some_and(|p| p.exists());
    DetectSaveDir { default_path, exists }
}

#[tauri::command]
fn detect_install_dir() -> String {
    config::detect_install_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[tauri::command]
fn get_config() -> Option<config::AppConfig> {
    config::load()
}

#[tauri::command]
fn set_config(cfg: config::AppConfig) -> Result<(), String> {
    config::save(&cfg)
}

#[tauri::command]
fn list_backups() -> Result<Vec<backup::BackupInfo>, String> {
    let cfg = config::load().ok_or("未配置。请先完成首次启动向导。")?;
    backup::list(Path::new(&cfg.backup_dir))
}

#[tauri::command]
fn create_backup() -> Result<backup::BackupInfo, String> {
    let cfg = config::load().ok_or("未配置。请先完成首次启动向导。")?;
    if is_game_running_inner() {
        return Err("检测到 Turing Complete 正在运行，请先关闭游戏再备份".to_string());
    }
    backup::create(Path::new(&cfg.save_dir), Path::new(&cfg.backup_dir))
}

#[tauri::command]
fn restore_backup(name: String) -> Result<String, String> {
    let cfg = config::load().ok_or("未配置。请先完成首次启动向导。")?;
    if is_game_running_inner() {
        return Err("检测到 Turing Complete 正在运行，请先关闭游戏再恢复".to_string());
    }
    backup::restore(Path::new(&cfg.save_dir), Path::new(&cfg.backup_dir), &name)
}

#[tauri::command]
fn delete_backup(name: String) -> Result<(), String> {
    let cfg = config::load().ok_or("未配置。请先完成首次启动向导。")?;
    backup::delete(Path::new(&cfg.backup_dir), &name)
}

#[tauri::command]
fn is_game_running() -> bool {
    is_game_running_inner()
}

fn is_game_running_inner() -> bool {
    #[cfg(windows)]
    {
        // 用 tasklist 检查候选 exe 名
        for name in ["Turing Complete.exe", "TuringComplete.exe"] {
            let output = std::process::Command::new("tasklist.exe")
                .args(["/FI", &format!("IMAGENAME eq {}", name), "/NH"])
                .output();
            if let Ok(o) = output {
                let s = String::from_utf8_lossy(&o.stdout).to_lowercase();
                if s.contains(&name.to_lowercase()) && !s.contains("info:") {
                    return true;
                }
            }
        }
        false
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            app_version,
            detect_save_dir,
            detect_install_dir,
            get_config,
            set_config,
            list_backups,
            create_backup,
            restore_backup,
            delete_backup,
            is_game_running,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}