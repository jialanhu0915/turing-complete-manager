use serde::Serialize;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

mod backup;
mod circuit;
mod config;
mod dll;
mod game;
mod levels;
mod translations;

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
    let cfg = config::load().ok_or("NOT_CONFIGURED")?;
    backup::list(Path::new(&cfg.backup_dir))
}

#[tauri::command]
fn create_backup() -> Result<backup::BackupInfo, String> {
    let cfg = config::load().ok_or("NOT_CONFIGURED")?;
    if is_game_running_inner() {
        return Err("GAME_RUNNING_BACKUP".to_string());
    }
    backup::create(Path::new(&cfg.save_dir), Path::new(&cfg.backup_dir))
}

#[tauri::command]
fn restore_backup(name: String) -> Result<String, String> {
    let cfg = config::load().ok_or("NOT_CONFIGURED")?;
    if is_game_running_inner() {
        return Err("GAME_RUNNING_RESTORE".to_string());
    }
    backup::restore(Path::new(&cfg.save_dir), Path::new(&cfg.backup_dir), &name)
}

#[tauri::command]
fn delete_backup(name: String) -> Result<(), String> {
    let cfg = config::load().ok_or("NOT_CONFIGURED")?;
    backup::delete(Path::new(&cfg.backup_dir), &name)
}

#[tauri::command]
fn is_game_running() -> bool {
    is_game_running_inner()
}

/// `true` iff Turing Complete is installed and complete (has `compile.dll` +
/// `campaign/`). Gates the validation UI.
#[tauri::command]
fn is_game_available() -> bool {
    game::is_available()
}

#[tauri::command]
fn list_levels() -> Result<Vec<levels::LevelRow>, String> {
    let cfg = config::load().ok_or("NOT_CONFIGURED")?;
    levels::load_levels(Path::new(&cfg.save_dir))
}

#[tauri::command]
fn save_levels(updates: Vec<levels::LevelUpdate>) -> Result<String, String> {
    let cfg = config::load().ok_or("NOT_CONFIGURED")?;
    // 允许游戏中保存：levels.txt 支持热修改。
    // 注意：游戏中保存可能被游戏后续写入覆盖，操作需自行承担。
    levels::save_levels(Path::new(&cfg.save_dir), &updates)
}

#[tauri::command]
fn list_level_names() -> translations::LevelNames {
    translations::load_level_names()
}

#[tauri::command]
fn list_schematics(level_id: String) -> Result<Vec<String>, String> {
    let cfg = config::load().ok_or("NOT_CONFIGURED")?;
    let dir = Path::new(&cfg.save_dir).join(&level_id);
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("CIRCUIT_LIST_FAILED|{e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("CIRCUIT_LIST_FAILED|{e}"))?;
        let name = entry.file_name();
        let s = name.to_string_lossy();
        if let Some(stem) = s.strip_suffix(".circuit") {
            // Scheme subfolders are <scheme>/circuit.data — but the GUI only
            // needs the scheme name. Future: also handle nested scheme dirs.
            out.push(stem.to_string());
        }
    }
    out.sort();
    Ok(out)
}

#[tauri::command]
fn read_circuit(
    level_id: String,
    scheme_id: String,
) -> Result<circuit::model::Circuit, String> {
    let cfg = config::load().ok_or("NOT_CONFIGURED")?;
    let path = Path::new(&cfg.save_dir)
        .join(&level_id)
        .join(&scheme_id)
        .join("circuit.data");
    let bytes = std::fs::read(&path).map_err(|e| format!("CIRCUIT_READ_FAILED|{e}"))?;
    circuit::codec::decode_circuit(&bytes)
}

#[tauri::command]
fn write_circuit(
    level_id: String,
    scheme_id: String,
    payload: circuit::model::Circuit,
) -> Result<(), String> {
    let cfg = config::load().ok_or("NOT_CONFIGURED")?;
    let bytes = circuit::codec::encode_v15(&payload)
        .map_err(|e| format!("CIRCUIT_ENCODE_FAILED|{e}"))?;
    let dir = Path::new(&cfg.save_dir)
        .join(&level_id)
        .join(&scheme_id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("CIRCUIT_DIR_FAILED|{e}"))?;
    std::fs::write(dir.join("circuit.data"), bytes)
        .map_err(|e| format!("CIRCUIT_WRITE_FAILED|{e}"))
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
    is_game_available,
            list_levels,
            save_levels,
            list_level_names,
            list_schematics,
            read_circuit,
            write_circuit,
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