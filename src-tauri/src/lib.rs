use serde::Serialize;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

mod backup;
mod config;
mod levels;

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

#[tauri::command]
fn list_levels() -> Result<Vec<levels::LevelRow>, String> {
    let cfg = config::load().ok_or("未配置。请先完成首次启动向导。")?;
    levels::load_levels(Path::new(&cfg.save_dir))
}

#[tauri::command]
fn save_levels(updates: Vec<levels::LevelUpdate>) -> Result<String, String> {
    let cfg = config::load().ok_or("未配置。请先完成首次启动向导。")?;
    // 允许游戏中保存：levels.txt 支持热修改。
    // 注意：游戏中保存可能被游戏后续写入覆盖，操作需自行承担。
    levels::save_levels(Path::new(&cfg.save_dir), &updates)
}

fn is_game_running_inner() -> bool {
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW 避免 tasklist 弹出黑色命令行窗口
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const NAME: &str = "Turing Complete.exe";
        let output = std::process::Command::new("tasklist.exe")
            .args(["/FI", &format!("IMAGENAME eq {}", NAME), "/NH"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        if let Ok(o) = output {
            let s = String::from_utf8_lossy(&o.stdout);
            // 找到：包含进程名；未找到：包含 "INFO: No tasks..."
            return s.contains(NAME) && !s.contains("INFO:");
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
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                auto_backup_loop(handle);
            });
            Ok(())
        })
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
            list_levels,
            save_levels,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 后台循环：按配置中的间隔自动备份；启用变更后 60s 内生效。
fn auto_backup_loop(app: AppHandle) {
    loop {
        let cfg = match config::load() {
            Some(c) => c,
            None => {
                std::thread::sleep(Duration::from_secs(60));
                continue;
            }
        };
        if !cfg.auto_backup_enabled || cfg.auto_backup_interval_min == 0 {
            // 关闭或间隔为 0：每 60s 复查一次配置
            std::thread::sleep(Duration::from_secs(60));
            continue;
        }
        let secs = cfg.auto_backup_interval_min as u64 * 60;
        std::thread::sleep(Duration::from_secs(secs));
        match run_auto_backup(&cfg) {
            Ok(()) => {
                let _ = app.emit("auto-backup-done", ());
            }
            Err(e) => eprintln!("[auto-backup] failed: {e}"),
        }
    }
}

/// 执行一次自动备份。热备（不检查游戏运行），完成后按 keep 清理最旧。
fn run_auto_backup(cfg: &config::AppConfig) -> Result<(), String> {
    let save = Path::new(&cfg.save_dir);
    let backup = Path::new(&cfg.backup_dir);
    backup::create(save, backup)?;
    let list = backup::list(backup)?;
    let keep = cfg.auto_backup_keep as usize;
    if list.len() > keep {
        for old in &list[keep..] {
            // 清理失败不致命 —— 下次循环再试
            let _ = backup::delete(backup, &old.name);
        }
    }
    Ok(())
}