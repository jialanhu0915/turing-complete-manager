use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_interval() -> u32 {
    30
}
fn default_keep() -> u32 {
    20
}

/// `game_dir` 的来源。Auto = 上次自动检测结果（可被重新检测覆盖）；
/// Manual = 用户手动指定（重新检测不会覆盖，需用户主动改）。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GameDirSource {
    #[default]
    Auto,
    Manual,
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
    /// 游戏安装目录缓存。首次启动后由 warm-up 写入；用户也可手动指定。
    #[serde(default)]
    pub game_dir: Option<String>,
    /// `game_dir` 是自动检测还是手动指定。Manual 时不被自动覆盖。
    #[serde(default)]
    pub game_dir_source: GameDirSource,
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
    let dir = config_dir().ok_or_else(|| "APPDATA_NOT_SET".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("CONFIG_DIR_FAILED|{e}"))?;
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
    std::fs::write(&path, bytes).map_err(|e| format!("CONFIG_WRITE_FAILED|{e}"))
}

pub fn default_save_dir() -> Option<PathBuf> {
    appdata_dir().map(|d| d.join("Turing Complete"))
}

/// 备份目录默认值：与软件本身解耦，落在 `%APPDATA%/turing-complete-manager/backups/`
/// 下，卸载/重装 app 时不丢备份。
pub fn default_backup_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("backups"))
}

pub fn detect_install_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from))
}

/// 纯读：cache 命中且 `<dir>/Turing Complete.exe` 存在即返回该路径，否则 None。
/// 不触发检测、不写盘。给翻译 / 角色等热路径用。
pub fn resolve_game_dir(cfg: &AppConfig) -> Option<PathBuf> {
    let cached = cfg.game_dir.as_deref()?;
    let p = PathBuf::from(cached);
    if p.join("Turing Complete.exe").exists() {
        Some(p)
    } else {
        None
    }
}

/// 重新跑 Steam 检测并写盘。结果写入 `cfg.game_dir`，source 设为 Auto。
/// 失败（找不到 Steam 或所有库都没装）返回 None，cfg 不变。
pub fn detect_and_persist_game_dir(cfg: &mut AppConfig) -> Option<PathBuf> {
    let detected = crate::translations::detect_game_dir()?;
    cfg.game_dir = Some(detected.to_string_lossy().into_owned());
    cfg.game_dir_source = GameDirSource::Auto;
    let _ = save(cfg);
    Some(detected)
}

/// 手动指定路径。无校验（用户可能在游戏装到目标盘前就先填好）。
pub fn set_game_dir_manual(cfg: &mut AppConfig, path: PathBuf) {
    cfg.game_dir = Some(path.to_string_lossy().into_owned());
    cfg.game_dir_source = GameDirSource::Manual;
    let _ = save(cfg);
}
