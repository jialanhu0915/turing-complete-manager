use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LevelRow {
    /// 关卡 ID（第一列），如 "introduction"
    pub id: String,
    /// 方案名（第三列）
    pub solution: String,
    /// 通关标志（第二列）
    pub completed: bool,
    /// 第四列按 '|' 切分的记录条数
    pub records: u32,
    /// 原始文件中的行号（0-based），用于写回定位
    pub line_index: usize,
}

/// 前端提交的部分修改：按行号切换通关标志
#[derive(Deserialize, Debug)]
pub struct LevelUpdate {
    pub line_index: usize,
    pub completed: bool,
}

fn parse_bool(s: &str) -> Option<bool> {
    let t = s.trim();
    if t.eq_ignore_ascii_case("true") {
        Some(true)
    } else if t.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

fn unquote(s: &str) -> &str {
    let t = s.trim();
    if t.starts_with('"') && t.ends_with('"') && t.len() >= 2 {
        &t[1..t.len() - 1]
    } else {
        t
    }
}

/// 读取 levels.txt，解析每一行为 LevelRow。
/// 格式：`"id",true|false,"solution","score1&...|score2&..."`(可选第四列)
///
/// 依赖格式约定：ID / 方案 / 第四列均不含未转义逗号。
/// 游戏关卡 ID 由开发者定义、方案名 UI 不允许逗号，所以安全。
pub fn load_levels(save_dir: &Path) -> Result<Vec<LevelRow>, String> {
    let path = save_dir.join("levels.txt");
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("LEVELS_READ_FAILED|{e}"))?;

    let mut rows = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 {
            return Err(format!("LEVELS_ROW_FORMAT|{}", i + 1));
        }
        let id = unquote(parts[0]).to_string();
        let completed = parse_bool(parts[1])
            .ok_or_else(|| format!("LEVELS_BAD_BOOL|{}", i + 1))?;
        let solution = unquote(parts[2]).to_string();
        let records = if parts.len() >= 4 {
            unquote(parts[3]).split('|').filter(|s| !s.is_empty()).count() as u32
        } else {
            0
        };

        rows.push(LevelRow {
            id,
            solution,
            completed,
            records,
            line_index: i,
        });
    }
    Ok(rows)
}

/// 把内存中整篇文本按行号切出来，仅替换指定行的第二列，其他原样保留。
fn apply_updates_to_text(raw: &str, updates: &[(usize, bool)]) -> String {
    let new_val = |completed: bool| if completed { "true" } else { "false" };
    let lines: Vec<&str> = raw.lines().collect();
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

    for (line_index, completed) in updates {
        if let Some(line) = out.get_mut(*line_index) {
            // 仅替换第一个逗号前后的第二列
            if let Some(first_comma) = line.find(',') {
                let rest = &line[first_comma + 1..];
                if let Some(second_comma_rel) = rest.find(',') {
                    let second_comma = first_comma + 1 + second_comma_rel;
                    let head = &line[..first_comma + 1];
                    let tail = &line[second_comma..];
                    *line = format!("{}{}{}", head, new_val(*completed), tail);
                }
            }
        }
    }
    out.join("\n")
}

/// 保存修改：先备份原文件到 levels_backups/，再写回。
/// 返回备份文件名。
pub fn save_levels(save_dir: &Path, updates: &[LevelUpdate]) -> Result<String, String> {
    if updates.is_empty() {
        return Err("NO_LEVEL_CHANGES".to_string());
    }

    let path = save_dir.join("levels.txt");
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("LEVELS_READ_FAILED|{e}"))?;

    // 自动备份
    let backup_dir = save_dir.join("levels_backups");
    std::fs::create_dir_all(&backup_dir).map_err(|e| format!("LEVELS_BACKUP_DIR_FAILED|{e}"))?;
    let backup_name = Local::now()
        .format("levels_%Y-%m-%d_%H%M%S.txt")
        .to_string();
    std::fs::write(backup_dir.join(&backup_name), &raw)
        .map_err(|e| format!("LEVELS_BACKUP_WRITE_FAILED|{e}"))?;

    // 应用修改（保证 line_index 与 updates 都合法）
    let pairs: Vec<(usize, bool)> = updates.iter().map(|u| (u.line_index, u.completed)).collect();
    let new_text = apply_updates_to_text(&raw, &pairs);

    std::fs::write(&path, new_text).map_err(|e| format!("LEVELS_WRITE_FAILED|{e}"))?;
    Ok(backup_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bool_basic() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("false"), Some(false));
        assert_eq!(parse_bool("True"), Some(true));
        assert_eq!(parse_bool("FALSE"), Some(false));
        assert_eq!(parse_bool("  true  "), Some(true));
        assert_eq!(parse_bool("yes"), None);
        assert_eq!(parse_bool(""), None);
    }

    #[test]
    fn unquote_basic() {
        assert_eq!(unquote("\"foo\""), "foo");
        assert_eq!(unquote("foo"), "foo");
        assert_eq!(unquote("\"foo"), "\"foo");
        assert_eq!(unquote("foo\""), "foo\"");
        assert_eq!(unquote("\"\""), "");
    }

    #[test]
    fn records_count_logic() {
        // 复刻 load_levels 里的 records 计算逻辑
        fn count(s: &str) -> u32 {
            s.split('|').filter(|s| !s.is_empty()).count() as u32
        }
        assert_eq!(count("1&2&1|2&2&1"), 2);
        assert_eq!(count("1&2&1|"), 1);
        assert_eq!(count(""), 0);
        assert_eq!(count("|"), 0);
    }

    #[test]
    fn load_levels_parses_real_format() {
        let dir = std::env::temp_dir().join("tcm_levels_test_load");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("levels.txt");
        let raw = "\"intro\",true,\"Default\"\n\
                   \"byte_adder\",false,\"LCA\",\"56&32&1|86&17&1\"\n\
                   \"empty\",false,\"\",\"\"\n";
        std::fs::write(&path, raw).unwrap();

        let rows = load_levels(&dir).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].id, "intro");
        assert!(rows[0].completed);
        assert_eq!(rows[0].solution, "Default");
        assert_eq!(rows[0].records, 0);

        assert_eq!(rows[1].id, "byte_adder");
        assert!(!rows[1].completed);
        assert_eq!(rows[1].records, 2);

        assert_eq!(rows[2].id, "empty");
        assert_eq!(rows[2].records, 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_replaces_only_second_column() {
        let raw = "\"introduction\",true,\"Default\",\n\"byte_adder\",false,\"LCA\",\"56&32&1\"\n";
        let updates = vec![(1usize, true)];
        let out = apply_updates_to_text(raw, &updates);
        let lines: Vec<&str> = out.split('\n').collect();
        assert_eq!(lines[0], "\"introduction\",true,\"Default\",", "第一行未修改");
        assert_eq!(lines[1], "\"byte_adder\",true,\"LCA\",\"56&32&1\"", "第二行第二列被替换，其他原样");
    }
}