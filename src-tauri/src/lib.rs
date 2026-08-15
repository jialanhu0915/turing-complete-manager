use serde::Serialize;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

mod backup;
mod character;
mod config;
mod levels;
mod translations;

mod game;

use tc_mod_sdk::circuit;

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
fn detect_backup_dir() -> String {
    config::default_backup_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[tauri::command]
fn detect_game_dir() -> Option<String> {
    let mut cfg = config::load()?;
    config::detect_and_persist_game_dir(&mut cfg)
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
fn set_game_dir(path: String) -> Result<(), String> {
    let mut cfg = config::load().ok_or("NOT_CONFIGURED")?;
    config::set_game_dir_manual(&mut cfg, PathBuf::from(path));
    Ok(())
}

#[derive(Serialize)]
struct GameDirStatus {
    path: Option<String>,
    source: String,
    /// `<path>/Turing Complete.exe` 是否存在
    exists: bool,
}

#[tauri::command]
fn get_game_dir_status() -> GameDirStatus {
    let cfg = config::load();
    let (path, source) = match cfg.as_ref() {
        Some(c) => (c.game_dir.clone(), c.game_dir_source.clone()),
        None => (None, config::GameDirSource::Auto),
    };
    let exists = path
        .as_deref()
        .map(PathBuf::from)
        .map(|p| p.join("Turing Complete.exe").exists())
        .unwrap_or(false);
    let source_str = match source {
        config::GameDirSource::Auto => "auto",
        config::GameDirSource::Manual => "manual",
    };
    GameDirStatus {
        path,
        source: source_str.to_string(),
        exists,
    }
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

// ===== 电路验证（circuit.rs / dll.rs / game.rs，来自 test/verify-cli） =====

/// `true` iff Turing Complete is installed and complete (has `compile.dll` +
/// `campaign/`). Gates the validation UI.
#[tauri::command]
fn is_game_available() -> bool {
    game::is_available()
}

/// Result of a verify_circuit invocation (parsed from the verify CLI's JSON).
#[derive(serde::Deserialize, serde::Serialize)]
struct VerifyResult {
    ok: bool,
    test_result: u64,
    cycles_run: i64,
    error: Option<String>,
}

#[tauri::command]
fn list_schematics(level_id: String) -> Result<Vec<String>, String> {
    let cfg = config::load().ok_or("NOT_CONFIGURED")?;
    let dir = Path::new(&cfg.save_dir).join("schematics").join(&level_id);
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("CIRCUIT_LIST_FAILED|{e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("CIRCUIT_LIST_FAILED|{e}"))?;
        let name = entry.file_name();
        let s = name.to_string_lossy();
        if entry.path().is_dir() && entry.path().join("circuit.data").is_file() {
            out.push(s.into_owned());
        } else if let Some(stem) = s.strip_suffix(".circuit") {
            out.push(stem.to_string());
        }
    }
    out.sort();
    Ok(out)
}

#[tauri::command]
fn read_circuit(level_id: String, scheme_id: String) -> Result<circuit::model::Circuit, String> {
    let cfg = config::load().ok_or("NOT_CONFIGURED")?;
    let path = Path::new(&cfg.save_dir)
        .join("schematics")
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
        .join("schematics")
        .join(&level_id)
        .join(&scheme_id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("CIRCUIT_DIR_FAILED|{e}"))?;
    std::fs::write(dir.join("circuit.data"), bytes)
        .map_err(|e| format!("CIRCUIT_WRITE_FAILED|{e}"))
}

#[tauri::command]
fn verify_circuit(level_id: String, scheme_id: String) -> Result<VerifyResult, String> {
    let cfg = config::load().ok_or("NOT_CONFIGURED")?;
    if !game::is_available() {
        return Err("GAME_NOT_DETECTED".into());
    }
    let game_dir = game::detect().ok_or("GAME_NOT_DETECTED")?;

    let exe = std::env::current_exe().map_err(|e| format!("VERIFY_LOCATE|{e}"))?;
    let verify = exe.with_file_name("verify.exe");
    if !verify.is_file() {
        return Err(format!("VERIFY_NOT_FOUND|{}", verify.display()));
    }

    let output = std::process::Command::new(&verify)
        .arg("--game")
        .arg(&game_dir)
        .arg("--save")
        .arg(&cfg.save_dir)
        .arg("--level")
        .arg(&level_id)
        .arg("--scheme")
        .arg(&scheme_id)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("VERIFY_SPAWN_FAILED|{}|{e}", verify.display()))?
        .wait_with_output()
        .map_err(|e| format!("VERIFY_WAIT_FAILED|{e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    serde_json::from_str(&stdout).map_err(|e| format!("VERIFY_PARSE_FAILED|{e}|{stdout}"))
}

// ===== 角色替换（character.rs） =====

#[tauri::command]
fn character_status() -> character::CharacterStatus {
    character::status_impl()
}

#[tauri::command]
fn list_characters() -> Vec<character::Character> {
    character::list_characters_impl()
}

#[tauri::command]
fn create_character(name: String) -> Result<character::Character, String> {
    character::create_character_impl(&name)
}

#[tauri::command]
fn delete_character(id: String) -> Result<(), String> {
    character::delete_character_impl(&id)
}

#[tauri::command]
fn duplicate_character(id: String, new_name: String) -> Result<character::Character, String> {
    character::duplicate_character_impl(&id, &new_name)
}

#[tauri::command]
fn save_character_image(id: String, slot: String, png_base64: String) -> Result<(), String> {
    character::save_character_image_impl(&id, &slot, &png_base64)
}

#[tauri::command]
fn apply_character(id: String) -> Result<(), String> {
    character::apply_character_impl(&id)
}

#[tauri::command]
fn restore_default_character() -> Result<(), String> {
    character::restore_default_impl()
}

/// 用系统默认浏览器打开外部链接。
///
/// Tauri 2 的 WebView2 默认拦截 `target="_blank"`，所以 QQ 群这种外链必须主动调起来。
/// Windows 下用 `cmd /c start "" <url>`：空字符串是 start 的窗口标题参数，避免被当成 title。
#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("cmd")
            .args(["/c", "start", "", &url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("OPEN_URL_FAILED|{e}"))?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("OPEN_URL_UNSUPPORTED".to_string())
    }
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
            warm_game_dir_cache();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                auto_backup_loop(handle);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            detect_save_dir,
            detect_backup_dir,
            detect_game_dir,
            set_game_dir,
            get_game_dir_status,
            get_config,
            set_config,
            list_backups,
            create_backup,
            restore_backup,
            delete_backup,
            is_game_running,
            list_levels,
            save_levels,
            list_level_names,
            is_game_available,
            list_schematics,
            read_circuit,
            write_circuit,
            verify_circuit,
            character_status,
            list_characters,
            create_character,
            delete_character,
            duplicate_character,
            save_character_image,
            apply_character,
            restore_default_character,
            open_external_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 启动时跑一次：补全 cfg.game_dir（用户没手动指定的前提下）。
/// 失败就静默 —— UI 会显示"未检测到"，用户可走"浏览..."手动覆盖。
fn warm_game_dir_cache() {
    let Some(mut cfg) = config::load() else { return };
    // 用户手动指定过：不覆盖
    if cfg.game_dir_source == config::GameDirSource::Manual {
        return;
    }
    // 缓存路径仍有效：跳过
    if config::resolve_game_dir(&cfg).is_some() {
        return;
    }
    // 跑一次检测，结果写入 cfg + 落盘
    let _ = config::detect_and_persist_game_dir(&mut cfg);
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