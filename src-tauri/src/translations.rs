use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 单个关卡的标题映射。当前只支持 en + zh-CN，因为游戏只分发这几种翻译文件。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LevelName {
    pub en: String,
    #[serde(rename = "zh-CN")]
    pub zh_cn: String,
}

pub type LevelNames = HashMap<String, LevelName>;

/// 试图定位 Steam 安装目录中的 Turing Complete。
/// 策略：读注册表 SteamPath → 读 libraryfolders.vdf 拿到所有库路径 → 在每个库下找 steamapps/common/Turing Complete/。
/// 任意一步失败即返回 None，不抛错（前端按 fallback 处理：只显示 ID）。
pub fn detect_game_dir() -> Option<PathBuf> {
    let steam_path = read_steam_path()?;
    let library_paths = read_library_paths(&steam_path).unwrap_or_default();
    // 把 SteamPath 自身也作为候选（部分用户把 Steam 装在默认盘就一个库）
    let mut candidates: Vec<PathBuf> = library_paths;
    if !candidates.iter().any(|p| p == &steam_path) {
        candidates.push(steam_path);
    }
    for lib in candidates {
        let candidate = lib.join("steamapps").join("common").join("Turing Complete");
        if candidate.join("Turing Complete.exe").exists() {
            return Some(candidate);
        }
    }
    None
}

/// 读 `HKCU\Software\Valve\Steam\SteamPath`。用 reg.exe 而非 winreg 避免新增依赖。
#[cfg(windows)]
fn read_steam_path() -> Option<PathBuf> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("reg.exe")
        .args([
            "query",
            r"HKCU\Software\Valve\Steam",
            "/v",
            "SteamPath",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    // 输出形如 "    SteamPath    REG_SZ    C:\\Program Files (x86)\\Steam"
    let last = s.lines().filter(|l| l.contains("SteamPath")).last()?;
    let value = last.split_whitespace().nth(2)?;
    Some(PathBuf::from(value))
}

#[cfg(not(windows))]
fn read_steam_path() -> Option<PathBuf> {
    None
}

/// 解析 Steam 的 libraryfolders.vdf，抽出所有 "path" 字段。
fn read_library_paths(steam_dir: &Path) -> Option<Vec<PathBuf>> {
    let vdf = steam_dir.join("config").join("libraryfolders.vdf");
    let text = std::fs::read_to_string(&vdf).ok()?;
    // VDF 格式里 "path" 后面紧跟一个带引号的路径。简单按行匹配即可，不依赖完整 VDF 解析器。
    let mut out = Vec::new();
    for line in text.lines() {
        // 例如:         "path"		"E:\\SteamLibrary"
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("\"path\"") else {
            continue;
        };
        // 紧跟一个引号包裹的字符串
        let value = rest.trim().trim_matches('"');
        if !value.is_empty() {
            // VDF 用双反斜杠转义，反斜杠字面值需要还原为单反斜杠
            let unescaped = value.replace("\\\\", "\\");
            out.push(PathBuf::from(unescaped));
        }
    }
    Some(out)
}

/// 缓存文件路径：与 config.json 同目录。
fn cache_path() -> Option<PathBuf> {
    crate::config::config_dir().map(|d| d.join("level_names.json"))
}

/// 加载关卡名映射：先读缓存；若缓存比游戏 translations 目录旧或缺 zh-CN，则重新解析。
/// 失败（找不到游戏 / 解析错误）一律返回空 map —— 前端按只显示 ID 处理。
pub fn load_level_names() -> LevelNames {
    let game_dir = match detect_game_dir() {
        Some(d) => d,
        None => return read_cache().unwrap_or_default(),
    };

    let translations_dir = game_dir.join("translations");
    if !translations_dir.exists() {
        return read_cache().unwrap_or_default();
    }

    // 缓存新鲜度：mtime(translations_dir) <= mtime(cache) 才视为新鲜
    let cache_f = cache_path();
    if let Some(ref cp) = cache_f {
        if let (Ok(cache_meta), Ok(src_meta)) = (
            std::fs::metadata(cp),
            std::fs::metadata(&translations_dir),
        ) {
            if let (Ok(cache_mtime), Ok(src_mtime)) = (
                cache_meta.modified(),
                src_meta.modified(),
            ) {
                if cache_mtime >= src_mtime {
                    if let Some(map) = read_cache() {
                        if map.values().any(|n| !n.zh_cn.is_empty()) {
                            return map;
                        }
                    }
                }
            }
        }
    }

    // 解析 + 写入缓存（写失败不致命 —— 下次启动还会重试）
    let map = parse_level_names(&game_dir).unwrap_or_default();
    if !map.is_empty() {
        if let Some(ref cp) = cache_f {
            let _ = write_cache(cp, &map);
        }
    }
    map
}

fn read_cache() -> Option<LevelNames> {
    let cp = cache_path()?;
    let bytes = std::fs::read(&cp).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_cache(path: &Path, map: &LevelNames) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(map).map_err(std::io::Error::other)?;
    std::fs::write(path, bytes)
}

/// 解析核心：扫描 campaign/*/meta.txt 与 Chinese (Simplified).txt。
/// 单独抽出以便测试。
fn parse_level_names(game_dir: &Path) -> Option<LevelNames> {
    let campaign_dir = game_dir.join("campaign");
    let translations_dir = game_dir.join("translations");

    let zh_map = parse_zh_translations(&translations_dir)?;
    let mut out = LevelNames::new();
    for entry in std::fs::read_dir(&campaign_dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = path.file_name()?.to_string_lossy().into_owned();
        let meta_path = path.join("meta.txt");
        let Some((hash, en)) = parse_title_from_meta(&meta_path) else {
            continue;
        };
        let zh_cn = zh_map.get(&hash).cloned().unwrap_or_default();
        out.insert(
            id,
            LevelName {
                en,
                zh_cn,
            },
        );
    }
    Some(out)
}

/// 从 `title = (31337_<hash>, `<English>`)` 行抽出 hash 与英文标题。
fn parse_title_from_meta(meta_path: &Path) -> Option<(String, String)> {
    let raw = std::fs::read_to_string(meta_path).ok()?;
    for line in raw.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("title = ") else {
            continue;
        };
        // 形如 (31337_<digits>, `<text>`) —— 标题里可嵌套括号如 "Arithmetic Logic Unit (ALU) 1"
        let rest = rest.trim().strip_prefix('(')?;
        let (hash_part, after_hash) = rest.split_once(',')?;
        let hash = hash_part
            .trim()
            .strip_prefix("31337_")?
            .trim()
            .to_string();
        // after_hash 形如 " `<text>`)" —— 先去前导反引号，再去尾 `)` 和反引号
        let after = after_hash.trim();
        let no_leading_tick = after.strip_prefix('`')?;
        let no_close_paren = no_leading_tick.strip_suffix(')')?;
        let en = no_close_paren.strip_suffix('`')?.to_string();
        return Some((hash, en));
    }
    None
}

/// 从 translations/Chinese (Simplified).txt 抽 `$<hash>* <text>` 映射。
/// 只保留单行短标题：长描述通常会跨多行、不以 `$` 开头，靠行级解析自然跳过。
fn parse_zh_translations(translations_dir: &Path) -> Option<HashMap<String, String>> {
    let path = translations_dir.join("Chinese (Simplified).txt");
    let raw = std::fs::read_to_string(&path).ok()?;
    let mut out = HashMap::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        let Some(after_dollar) = trimmed.strip_prefix('$') else {
            continue;
        };
        // 形如 $50956872142719* 同或门（XNOR）
        let Some((hash, rest)) = after_dollar.split_once('*') else {
            continue;
        };
        let hash = hash.trim();
        if !hash.chars().all(|c| c.is_ascii_digit()) || hash.is_empty() {
            continue;
        }
        let text = rest.trim();
        if text.is_empty() {
            continue;
        }
        out.entry(hash.to_string()).or_insert_with(|| text.to_string());
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_title_basic() {
        let dir = std::env::temp_dir().join("tcm_meta_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("meta.txt");
        std::fs::write(
            &path,
            "kind = combinational\ntitle = (31337_50956872142719, `XNOR Gate`)\n",
        )
        .unwrap();
        let (hash, en) = parse_title_from_meta(&path).unwrap();
        assert_eq!(hash, "50956872142719");
        assert_eq!(en, "XNOR Gate");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parse_title_handles_inner_parens() {
        let dir = std::env::temp_dir().join("tcm_meta_test_parens");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("meta.txt");
        std::fs::write(
            &path,
            "title = (31337_16562043259074, `Arithmetic Logic Unit (ALU) 1`)\n",
        )
        .unwrap();
        let (hash, en) = parse_title_from_meta(&path).unwrap();
        assert_eq!(hash, "16562043259074");
        assert_eq!(en, "Arithmetic Logic Unit (ALU) 1");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parse_title_skips_unrelated_lines() {
        let dir = std::env::temp_dir().join("tcm_meta_test2");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("meta.txt");
        std::fs::write(
            &path,
            "kind = combinational\nhint_kind = combinational\ntitle = (31337_123, `Test`)\n",
        )
        .unwrap();
        let (hash, en) = parse_title_from_meta(&path).unwrap();
        assert_eq!(hash, "123");
        assert_eq!(en, "Test");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_library_paths_skips_non_path_lines() {
        // 回归测试：循环里用 `?` 会让首行失败时整个函数返回 None。
        // VDF 第一行是 "libraryfolders"，不是 "path"，必须正确跳过。
        let dir = std::env::temp_dir().join("tcm_vdf_test");
        std::fs::create_dir_all(dir.join("config")).unwrap();
        let vdf = dir.join("config").join("libraryfolders.vdf");
        std::fs::write(
            &vdf,
            "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Steam\"\n\t}\n}\n",
        )
        .unwrap();
        let paths = read_library_paths(&dir).expect("must not return None");
        assert_eq!(paths, vec![PathBuf::from("C:\\Steam")]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parse_zh_basic() {
        let dir = std::env::temp_dir().join("tcm_zh_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Chinese (Simplified).txt");
        std::fs::write(
            &path,
            "=== campaign/xnor/meta.txt ===\n\n$50956872142719* 同或门（XNOR）\n$16562043259074* 算术逻辑单元（ALU）1\n",
        )
        .unwrap();
        let map = parse_zh_translations(&dir).unwrap();
        assert_eq!(map.get("50956872142719").map(|s| s.as_str()), Some("同或门（XNOR）"));
        assert_eq!(map.get("16562043259074").map(|s| s.as_str()), Some("算术逻辑单元（ALU）1"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parse_level_names_end_to_end() {
        // 用真实游戏目录作为 fixture（依赖用户机器安装位置）
        let Some(game_dir) = detect_game_dir() else {
            // 没装游戏的环境直接跳过
            return;
        };
        let map = parse_level_names(&game_dir).expect("parse ok");
        assert!(!map.is_empty());
        // 抽样验证：xnor 的英文标题已知
        assert_eq!(
            map.get("xnor").map(|n| n.en.as_str()),
            Some("XNOR Gate")
        );
        // 中文翻译存在（hash 50956872142719 → 同或门）
        assert_eq!(
            map.get("xnor").map(|n| n.zh_cn.as_str()),
            Some("同或门（XNOR）")
        );
    }
}