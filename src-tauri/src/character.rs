use base64::Engine;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

// 角色库管理。
//
// 存储布局（全部在 install_dir 下，避免写 C 盘）：
//   install_dir/default_characters/<name>/    只读预装（来自 MSI bundle）
//      manifest.json
//      neutral.png
//      smile.png
//   install_dir/characters/                   用户自建（可写）
//      index.json
//      <uuid>/neutral.png
//      <uuid>/smile.png
//   install_dir/originals/                    游戏原图快照（首次替换时建，永不覆盖）
//      mentor_neutral.png
//      mentor_smile.png
//
// 角色 id 格式：default:<dirname> 或 custom:<uuid>，前缀区分只读 / 可写。

// ===== 公共类型 =====

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CharacterKind {
    Default,
    Custom,
}

#[derive(Serialize, Clone, Debug)]
pub struct Character {
    pub id: String,
    pub name: String,
    pub kind: CharacterKind,
    pub has_neutral: bool,
    pub has_smile: bool,
    pub created_at: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct CharacterStatus {
    pub install_dir: String,
    pub install_dir_writable: bool,
    pub game_available: bool,
    pub game_dialogue_dir: Option<String>,
    pub snapshot_taken: bool,
    pub active_id: Option<String>,
}

// ===== 内部类型 =====

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct CustomIndex {
    #[serde(default)]
    active: Option<String>,
    #[serde(default)]
    characters: Vec<CustomEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct CustomEntry {
    id: String,
    name: String,
    has_neutral: bool,
    has_smile: bool,
    created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct DefaultManifest {
    id: String,
    #[serde(default)]
    name_zh: String,
    #[serde(default)]
    name_en: String,
    #[serde(default = "default_true")]
    has_neutral: bool,
    #[serde(default = "default_true")]
    has_smile: bool,
}
fn default_true() -> bool {
    true
}

// ===== 常量 =====

const NEUTRAL_FILE: &str = "neutral.png";
const SMILE_FILE: &str = "smile.png";
const MENTOR_NEUTRAL: &str = "mentor_neutral.png";
const MENTOR_SMILE: &str = "mentor_smile.png";

/// 测试钩子：设置后覆盖 install_dir，便于单测不污染真实安装目录。
#[cfg(test)]
const TEST_BASE_ENV: &str = "TC_CHARACTER_TEST_BASE";

/// 测试钩子：设置后覆盖游戏根目录，避免单测在真机上覆写 mentor PNG。
#[cfg(test)]
const TEST_GAME_DIR_ENV: &str = "TC_CHARACTER_TEST_GAME_DIR";

// ===== 校验 =====

/// 角色名格式：以英文字母开头，后跟字母/数字/下划线/连字符。
/// 游戏本身只支持字母命名，所以中文/空格等会拒。
fn validate_character_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("CHARACTER_NAME_EMPTY".to_string());
    }
    let bytes = name.as_bytes();
    if !bytes[0].is_ascii_alphabetic() {
        return Err("CHARACTER_NAME_INVALID".to_string());
    }
    for &b in &bytes[1..] {
        let ok = b.is_ascii_alphabetic() || b.is_ascii_digit() || b == b'_' || b == b'-';
        if !ok {
            return Err("CHARACTER_NAME_INVALID".to_string());
        }
    }
    Ok(())
}

// ===== 路径解析 =====

/// 预装角色根目录。dev 模式从源码目录读，release 从安装目录读。
fn default_chars_root() -> PathBuf {
    #[cfg(debug_assertions)]
    {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("default_characters")
    }
    #[cfg(not(debug_assertions))]
    {
        base_install_dir()
            .map(|d| d.join("default_characters"))
            .unwrap_or_default()
    }
}

fn custom_chars_root() -> Result<PathBuf, String> {
    Ok(base_install_dir()
        .ok_or("INSTALL_DIR_NOT_FOUND")?
        .join("characters"))
}

fn originals_root() -> Result<PathBuf, String> {
    Ok(base_install_dir()
        .ok_or("INSTALL_DIR_NOT_FOUND")?
        .join("originals"))
}

fn char_dir(uuid_str: &str) -> Result<PathBuf, String> {
    Ok(custom_chars_root()?.join(uuid_str))
}

fn base_install_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        if let Ok(p) = std::env::var(TEST_BASE_ENV) {
            return Some(PathBuf::from(p));
        }
    }
    crate::config::detect_install_dir()
}

/// 探测 install_dir 是否可写（往里写一个临时文件再删）。
pub fn check_install_writable() -> bool {
    let Some(install) = base_install_dir() else {
        return false;
    };
    let probe = install.join(".tc_write_test");
    if fs::write(&probe, b"test").is_err() {
        return false;
    }
    let _ = fs::remove_file(&probe);
    true
}

// ===== 索引管理 =====

fn load_custom_index() -> CustomIndex {
    let path = match custom_chars_root() {
        Ok(d) => d.join("index.json"),
        Err(_) => return CustomIndex::default(),
    };
    fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_custom_index(idx: &CustomIndex) -> Result<(), String> {
    let dir = custom_chars_root()?;
    fs::create_dir_all(&dir).map_err(|e| format!("MKDIR_FAILED|{e}"))?;
    let path = dir.join("index.json");
    let bytes = serde_json::to_vec_pretty(idx).map_err(|e| format!("INDEX_WRITE_FAILED|{e}"))?;
    fs::write(&path, bytes).map_err(|e| format!("INDEX_WRITE_FAILED|{e}"))
}

// ===== 预装角色 =====

fn load_default_characters() -> Vec<Character> {
    let root = default_chars_root();
    if !root.exists() {
        return Vec::new();
    }
    let entries = match fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("manifest.json");
        let m: DefaultManifest = match fs::read(&manifest_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
        {
            Some(m) => m,
            None => continue,
        };
        // 实际文件存在性为准，manifest 可能撒谎
        let has_neutral = m.has_neutral && path.join(NEUTRAL_FILE).exists();
        let has_smile = m.has_smile && path.join(SMILE_FILE).exists();
        let name = localize_name(&m);
        out.push(Character {
            id: format!("default:{}", m.id),
            name,
            kind: CharacterKind::Default,
            has_neutral,
            has_smile,
            created_at: None,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn localize_name(m: &DefaultManifest) -> String {
    // 优先按当前 locale 取；缺失回退到另一语言；都没填则用 id。
    let locale = current_locale();
    let (primary, fallback) = if locale == "en-US" {
        (&m.name_en, &m.name_zh)
    } else {
        (&m.name_zh, &m.name_en)
    };
    if !primary.is_empty() {
        return primary.clone();
    }
    if !fallback.is_empty() {
        return fallback.clone();
    }
    m.id.clone()
}

fn current_locale() -> String {
    crate::config::load()
        .map(|c| c.language)
        .unwrap_or_else(|| "zh-CN".to_string())
}

// ===== 角色查询 =====

pub fn list_characters_impl() -> Vec<Character> {
    let mut out = load_default_characters();
    let idx = load_custom_index();
    for c in idx.characters {
        out.push(Character {
            id: format!("custom:{}", c.id),
            name: c.name,
            kind: CharacterKind::Custom,
            has_neutral: c.has_neutral,
            has_smile: c.has_smile,
            created_at: Some(c.created_at),
        });
    }
    out
}

// ===== 游戏文件操作 =====

fn game_dialogue_dir() -> Result<PathBuf, String> {
    let game = detect_game_root().ok_or("GAME_NOT_AVAILABLE")?;
    let d = game.join("asset").join("dialogue");
    if !d.is_dir() {
        return Err(format!("DIALOGUE_DIR_MISSING|{}", d.display()));
    }
    Ok(d)
}

/// 游戏根目录解析：单测可通过 TC_CHARACTER_TEST_GAME_DIR 覆盖。
fn detect_game_root() -> Option<PathBuf> {
    #[cfg(test)]
    {
        if let Ok(p) = std::env::var(TEST_GAME_DIR_ENV) {
            return Some(PathBuf::from(p));
        }
    }
    crate::translations::detect_game_dir()
}

/// 首次替换前快照原图。后续调用跳过（永不覆盖）。
pub fn ensure_snapshot(game_dialogue: &Path) -> Result<(), String> {
    let originals = originals_root()?;
    fs::create_dir_all(&originals).map_err(|e| format!("MKDIR_FAILED|{e}"))?;
    for src in [MENTOR_NEUTRAL, MENTOR_SMILE] {
        let from = game_dialogue.join(src);
        let to = originals.join(src);
        if to.exists() {
            continue;
        }
        if !from.exists() {
            continue;
        }
        fs::copy(&from, &to).map_err(|e| format!("SNAPSHOT_FAILED|{e}"))?;
    }
    Ok(())
}

fn restore_originals_to_game() -> Result<(), String> {
    let game_dialogue = game_dialogue_dir()?;
    let originals = originals_root()?;
    if !originals.is_dir() {
        return Err("NO_SNAPSHOT".to_string());
    }
    for src in [MENTOR_NEUTRAL, MENTOR_SMILE] {
        let from = originals.join(src);
        let to = game_dialogue.join(src);
        if !from.exists() {
            continue;
        }
        fs::copy(&from, &to).map_err(|e| format!("RESTORE_WRITE_FAILED|{e}"))?;
    }
    Ok(())
}

fn copy_to_game(src: &Path, dst_name: &str) -> Result<(), String> {
    let game_dialogue = game_dialogue_dir()?;
    let dst = game_dialogue.join(dst_name);
    fs::copy(src, &dst)
        .map(|_| ())
        .map_err(|e| format!("APPLY_WRITE_FAILED|{e}"))
}

pub fn apply_character_impl(id: &str) -> Result<(), String> {
    let game_dialogue = game_dialogue_dir()?;
    ensure_snapshot(&game_dialogue)?;

    // 解析源目录
    let src_dir = resolve_source_dir(id)?;

    // 处理两个 slot：角色有 → 写用户图；无 → 从 originals 拷回
    let originals = originals_root()?;
    for (slot_file, mentor_name) in [(NEUTRAL_FILE, MENTOR_NEUTRAL), (SMILE_FILE, MENTOR_SMILE)] {
        let user_src = src_dir.join(slot_file);
        if user_src.exists() {
            copy_to_game(&user_src, mentor_name)?;
        } else {
            let orig = originals.join(mentor_name);
            if orig.exists() {
                copy_to_game(&orig, mentor_name)?;
            }
            // originals 都没有：首次替换且游戏目录里也没有，保持现状不动
        }
    }

    // 更新 active
    let mut idx = load_custom_index();
    idx.active = Some(id.to_string());
    save_custom_index(&idx)?;
    Ok(())
}

fn resolve_source_dir(id: &str) -> Result<PathBuf, String> {
    if let Some(name) = id.strip_prefix("default:") {
        let dir = default_chars_root().join(name);
        if !dir.is_dir() {
            return Err("CHARACTER_NOT_FOUND".to_string());
        }
        return Ok(dir);
    }
    if let Some(uuid_str) = id.strip_prefix("custom:") {
        let dir = char_dir(uuid_str)?;
        if !dir.is_dir() {
            return Err("CHARACTER_NOT_FOUND".to_string());
        }
        return Ok(dir);
    }
    Err("CHARACTER_NOT_FOUND".to_string())
}

pub fn restore_default_impl() -> Result<(), String> {
    restore_originals_to_game()?;
    let mut idx = load_custom_index();
    idx.active = None;
    save_custom_index(&idx)?;
    Ok(())
}

// ===== 角色增删改 =====

pub fn create_character_impl(name: &str) -> Result<Character, String> {
    let name = name.trim();
    validate_character_name(name)?;
    let mut idx = load_custom_index();
    if idx.characters.iter().any(|c| c.name == name) {
        return Err("CHARACTER_NAME_DUP".to_string());
    }
    let uuid = Uuid::new_v4().to_string();
    let dir = char_dir(&uuid)?;
    fs::create_dir_all(&dir).map_err(|e| format!("MKDIR_FAILED|{e}"))?;

    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    idx.characters.push(CustomEntry {
        id: uuid.clone(),
        name: name.to_string(),
        has_neutral: false,
        has_smile: false,
        created_at: now.clone(),
    });
    save_custom_index(&idx)?;

    Ok(Character {
        id: format!("custom:{uuid}"),
        name: name.to_string(),
        kind: CharacterKind::Custom,
        has_neutral: false,
        has_smile: false,
        created_at: Some(now),
    })
}

pub fn delete_character_impl(id: &str) -> Result<(), String> {
    if id.starts_with("default:") {
        return Err("CHARACTER_READONLY".to_string());
    }
    let uuid = id.strip_prefix("custom:").ok_or("CHARACTER_NOT_FOUND")?;
    let mut idx = load_custom_index();
    let pos = idx
        .characters
        .iter()
        .position(|c| c.id == uuid)
        .ok_or("CHARACTER_NOT_FOUND")?;
    idx.characters.remove(pos);
    if idx.active.as_deref() == Some(id) {
        idx.active = None;
    }
    save_custom_index(&idx)?;

    // best-effort 清理目录
    let dir = char_dir(uuid)?;
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    Ok(())
}

pub fn duplicate_character_impl(id: &str, new_name: &str) -> Result<Character, String> {
    let new_name = new_name.trim();
    validate_character_name(new_name)?;
    if !id.starts_with("default:") {
        return Err("CHARACTER_NOT_FOUND".to_string());
    }
    let src_name = id.strip_prefix("default:").unwrap();
    let src_dir = default_chars_root().join(src_name);
    if !src_dir.is_dir() {
        return Err("CHARACTER_NOT_FOUND".to_string());
    }

    let mut idx = load_custom_index();
    if idx.characters.iter().any(|c| c.name == new_name) {
        return Err("CHARACTER_NAME_DUP".to_string());
    }

    let uuid = Uuid::new_v4().to_string();
    let dst_dir = char_dir(&uuid)?;
    fs::create_dir_all(&dst_dir).map_err(|e| format!("MKDIR_FAILED|{e}"))?;

    let mut has_neutral = false;
    let mut has_smile = false;
    for (slot_file, flag_slot) in [(NEUTRAL_FILE, "neutral"), (SMILE_FILE, "smile")] {
        let from = src_dir.join(slot_file);
        if from.exists() {
            let to = dst_dir.join(slot_file);
            fs::copy(&from, &to).map_err(|e| format!("CHARACTER_WRITE_FAILED|{e}"))?;
            match flag_slot {
                "neutral" => has_neutral = true,
                "smile" => has_smile = true,
                _ => {}
            }
        }
    }

    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    idx.characters.push(CustomEntry {
        id: uuid.clone(),
        name: new_name.to_string(),
        has_neutral,
        has_smile,
        created_at: now.clone(),
    });
    save_custom_index(&idx)?;

    Ok(Character {
        id: format!("custom:{uuid}"),
        name: new_name.to_string(),
        kind: CharacterKind::Custom,
        has_neutral,
        has_smile,
        created_at: Some(now),
    })
}

pub fn save_character_image_impl(id: &str, slot: &str, png_base64: &str) -> Result<(), String> {
    if id.starts_with("default:") {
        return Err("CHARACTER_READONLY".to_string());
    }
    let uuid = id.strip_prefix("custom:").ok_or("CHARACTER_NOT_FOUND")?;

    let slot_file = match slot {
        "neutral" => NEUTRAL_FILE,
        "smile" => SMILE_FILE,
        _ => return Err("CHARACTER_INVALID_SLOT".to_string()),
    };

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(png_base64)
        .map_err(|e| format!("CHARACTER_WRITE_FAILED|{e}"))?;

    let dir = char_dir(uuid)?;
    fs::create_dir_all(&dir).map_err(|e| format!("MKDIR_FAILED|{e}"))?;
    let path = dir.join(slot_file);
    fs::write(&path, &bytes).map_err(|e| format!("CHARACTER_WRITE_FAILED|{e}"))?;

    let mut idx = load_custom_index();
    if let Some(c) = idx.characters.iter_mut().find(|c| c.id == uuid) {
        match slot {
            "neutral" => c.has_neutral = true,
            "smile" => c.has_smile = true,
            _ => {}
        }
    }
    save_custom_index(&idx)?;
    Ok(())
}

// ===== 状态 =====

pub fn status_impl() -> CharacterStatus {
    let install = base_install_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let install_writable = check_install_writable();
    let game_dialogue = detect_game_root().map(|d| d.join("asset").join("dialogue"));
    let game_available = game_dialogue.as_ref().is_some_and(|d| d.is_dir());
    let snapshot = originals_root()
        .map(|d| d.join(MENTOR_NEUTRAL).exists())
        .unwrap_or(false);
    let active = load_custom_index().active;
    CharacterStatus {
        install_dir: install,
        install_dir_writable: install_writable,
        game_available,
        game_dialogue_dir: game_dialogue.map(|p| p.to_string_lossy().into_owned()),
        snapshot_taken: snapshot,
        active_id: active,
    }
}

// ===== 单测 =====

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    static SEQ: AtomicUsize = AtomicUsize::new(0);

    // 串行化整个测试套件：env var 是全局的，并行跑会互相覆盖
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// 配一个临时安装目录 + mock 游戏目录 + mock 默认角色目录。返回 (install_dir, game_dir, default_dir)。
    /// 由调用方持有 ENV_LOCK（不内部 lock）。
    fn setup() -> (PathBuf, PathBuf, PathBuf) {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let base = std::env::temp_dir().join(format!("tc_char_test_{pid}_{n}"));
        let install = base.join("install");
        // game 根目录是 base/game（不带 asset/dialogue 那一层），character::detect_game_root 期望 game 根
        let game_root = base.join("game");
        let game = game_root.join("asset").join("dialogue");
        let defaults = base.join("default_characters");
        fs::create_dir_all(&install).unwrap();
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(&defaults).unwrap();
        // 写两个游戏原图
        fs::write(game.join(MENTOR_NEUTRAL), b"ORIG_NEUTRAL").unwrap();
        fs::write(game.join(MENTOR_SMILE), b"ORIG_SMILE").unwrap();
        // 设环境变量覆盖 install_dir 和 game_dir
        std::env::set_var(TEST_BASE_ENV, &install);
        std::env::set_var(TEST_GAME_DIR_ENV, &game_root);
        (install, game, defaults)
    }

    fn teardown(install: &Path) {
        std::env::remove_var(TEST_BASE_ENV);
        std::env::remove_var(TEST_GAME_DIR_ENV);
        let parent = install.parent().unwrap();
        let _ = fs::remove_dir_all(parent);
    }

    /// 把默认角色 mock 到 CARGO_MANIFEST_DIR/default_characters 之外，避免污染源码树。
    /// 因为 default_chars_root() 在 dev 下走源码目录，单测里没法重定向。
    /// 解决：单测里直接往源码的 default_characters/ 临时建一个角色目录，用完清掉。
    fn write_default_manifest(name: &str, m: &DefaultManifest) -> PathBuf {
        let dev_default = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("default_characters");
        let dir = dev_default.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("manifest.json"),
            serde_json::to_vec_pretty(m).unwrap(),
        )
        .unwrap();
        dir
    }

    fn cleanup_default(name: &str) {
        let dev_default = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("default_characters");
        let _ = fs::remove_dir_all(dev_default.join(name));
    }

    #[test]
    fn snapshot_only_once() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (install, game, _) = setup();
        ensure_snapshot(&game).unwrap();
        // 第二次手动改 originals 应当不被覆盖
        let originals = originals_root().unwrap();
        fs::write(originals.join(MENTOR_NEUTRAL), b"MUTATED").unwrap();
        ensure_snapshot(&game).unwrap();
        let s = fs::read(originals.join(MENTOR_NEUTRAL)).unwrap();
        assert_eq!(s, b"MUTATED", "snapshot 不应覆盖已有文件");
        teardown(&install);
    }

    #[test]
    fn apply_writes_only_owned_slots() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (install, game, _) = setup();
        // 先 snapshot
        ensure_snapshot(&game).unwrap();

        // 建一个只设了 neutral 的角色
        let ch = create_character_impl("test_partial").unwrap();
        let uuid = ch.id.strip_prefix("custom:").unwrap().to_string();
        let dir = char_dir(&uuid).unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(NEUTRAL_FILE), b"USER_NEUTRAL").unwrap();

        // 用一个最简的 base64 字符串
        // 实际场景是 PNG；这里我们就用任意字节测试
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"smile_bytes_test");
        save_character_image_impl(&ch.id, "smile", &b64).unwrap();

        // 现在 apply
        apply_character_impl(&ch.id).unwrap();

        // 游戏目录：neutral = 用户图；smile = 用户图（因为有）
        let n = fs::read(game.join(MENTOR_NEUTRAL)).unwrap();
        let s = fs::read(game.join(MENTOR_SMILE)).unwrap();
        assert_eq!(n, b"USER_NEUTRAL");
        assert_eq!(s, b"smile_bytes_test");

        // 删除 smile，apply 后游戏目录 smile 应回退到 original
        fs::remove_file(dir.join(SMILE_FILE)).unwrap();
        // index 里 has_smile 还是 true 因为之前的 save；通过重新 apply 来覆盖
        // 实际行为：apply 读 index 的 has_*？不，apply 直接看文件存在性。
        apply_character_impl(&ch.id).unwrap();
        let s2 = fs::read(game.join(MENTOR_SMILE)).unwrap();
        assert_eq!(s2, b"ORIG_SMILE", "角色无该 slot 时应回退到 original");

        teardown(&install);
    }

    #[test]
    fn restore_resets_to_original() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (install, game, _) = setup();
        ensure_snapshot(&game).unwrap();
        let ch = create_character_impl("test_restore").unwrap();
        let uuid = ch.id.strip_prefix("custom:").unwrap().to_string();
        let dir = char_dir(&uuid).unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(NEUTRAL_FILE), b"USER_N").unwrap();
        fs::write(dir.join(SMILE_FILE), b"USER_S").unwrap();
        apply_character_impl(&ch.id).unwrap();
        assert_eq!(fs::read(game.join(MENTOR_NEUTRAL)).unwrap(), b"USER_N");

        restore_default_impl().unwrap();
        assert_eq!(fs::read(game.join(MENTOR_NEUTRAL)).unwrap(), b"ORIG_NEUTRAL");
        assert_eq!(fs::read(game.join(MENTOR_SMILE)).unwrap(), b"ORIG_SMILE");
        assert!(load_custom_index().active.is_none());

        teardown(&install);
    }

    #[test]
    fn default_character_readonly() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (install, _, _) = setup();
        let m = DefaultManifest {
            id: "ro_test".to_string(),
            name_zh: "只读".to_string(),
            name_en: "RO".to_string(),
            has_neutral: true,
            has_smile: true,
        };
        let dir = write_default_manifest("ro_test", &m);
        fs::write(dir.join(NEUTRAL_FILE), b"N").unwrap();
        fs::write(dir.join(SMILE_FILE), b"S").unwrap();

        let res = delete_character_impl("default:ro_test");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "CHARACTER_READONLY");

        cleanup_default("ro_test");
        teardown(&install);
    }

    #[test]
    fn duplicate_default_copies_files() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (install, _, _) = setup();
        let m = DefaultManifest {
            id: "dup_test".to_string(),
            name_zh: "复制源".to_string(),
            name_en: "DupSrc".to_string(),
            has_neutral: true,
            has_smile: true,
        };
        let dir = write_default_manifest("dup_test", &m);
        fs::write(dir.join(NEUTRAL_FILE), b"DUP_N").unwrap();
        fs::write(dir.join(SMILE_FILE), b"DUP_S").unwrap();

        let ch = duplicate_character_impl("default:dup_test", "DupCopy").unwrap();
        let uuid = ch.id.strip_prefix("custom:").unwrap();
        let dst = char_dir(uuid).unwrap();
        assert_eq!(fs::read(dst.join(NEUTRAL_FILE)).unwrap(), b"DUP_N");
        assert_eq!(fs::read(dst.join(SMILE_FILE)).unwrap(), b"DUP_S");

        cleanup_default("dup_test");
        teardown(&install);
    }

    #[test]
    fn install_writable_check() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (install, _, _) = setup();
        // setup 后 install_dir 应该是可写的
        assert!(check_install_writable());

        // 模拟只读：chmod 不可移植，跳过；改用不存在的路径
        std::env::set_var(TEST_BASE_ENV, "/this/does/not/exist/at/all");
        assert!(!check_install_writable());
        std::env::set_var(TEST_BASE_ENV, &install);
        assert!(check_install_writable());

        teardown(&install);
    }

    #[test]
    fn character_name_validation() {
        // 合法
        assert!(validate_character_name("Steve").is_ok());
        assert!(validate_character_name("My_Cat-1").is_ok());
        assert!(validate_character_name("a").is_ok());
        assert!(validate_character_name("X9").is_ok());
        // 非法
        assert_eq!(validate_character_name("").unwrap_err(), "CHARACTER_NAME_EMPTY");
        assert_eq!(validate_character_name("我的猫").unwrap_err(), "CHARACTER_NAME_INVALID");
        assert_eq!(validate_character_name("My Cat").unwrap_err(), "CHARACTER_NAME_INVALID");
        assert_eq!(validate_character_name("1Cat").unwrap_err(), "CHARACTER_NAME_INVALID");
        assert_eq!(validate_character_name("-Cat").unwrap_err(), "CHARACTER_NAME_INVALID");
        assert_eq!(validate_character_name("Cat!").unwrap_err(), "CHARACTER_NAME_INVALID");
    }
}
