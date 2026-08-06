use chrono::Local;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::Path;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[derive(Serialize, Clone, Debug)]
pub struct BackupInfo {
    pub name: String,
    pub created_at: String, // 本地时间 ISO 8601
    pub size_bytes: u64,
}

fn now_filename() -> String {
    Local::now().format("backup_%Y-%m-%d_%H%M%S.zip").to_string()
}

fn auto_backup_name() -> String {
    Local::now()
        .format("auto_before_restore_%Y-%m-%d_%H%M%S.zip")
        .to_string()
}

fn fmt_iso(secs: u64) -> String {
    // 把 UNIX 秒数格式化为本地时间 ISO 字符串
    let secs_i64 = i64::try_from(secs).unwrap_or(0);
    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(secs_i64, 0)
        .unwrap_or_else(chrono::Utc::now);
    dt.with_timezone(&Local).format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn to_forward_slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn zip_dir(source_dir: &Path, zip_path: &Path) -> Result<(), String> {
    if !source_dir.exists() {
        return Err(format!("存档目录不存在: {}", source_dir.display()));
    }
    if !source_dir.is_dir() {
        return Err(format!("存档路径不是目录: {}", source_dir.display()));
    }

    let file = std::fs::File::create(zip_path).map_err(|e| format!("创建 zip 失败: {e}"))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let root_name = source_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "root".to_string());

    for entry in WalkDir::new(source_dir).follow_links(false) {
        let entry = entry.map_err(|e| format!("遍历存档目录失败: {e}"))?;
        let path = entry.path();
        let relative = path.strip_prefix(source_dir).unwrap();
        let rel_str = to_forward_slash(relative);
        let zip_path = if rel_str.is_empty() {
            root_name.clone()
        } else {
            format!("{}/{}", root_name, rel_str)
        };

        if path.is_file() {
            zip.start_file(&zip_path, options)
                .map_err(|e| format!("写入 zip 条目失败: {e}"))?;
            let mut f = std::fs::File::open(path).map_err(|e| format!("打开文件失败: {e}"))?;
            std::io::copy(&mut f, &mut zip).map_err(|e| format!("压缩失败: {e}"))?;
        } else if path != source_dir {
            zip.add_directory(&zip_path, options)
                .map_err(|e| format!("写入 zip 目录失败: {e}"))?;
        }
    }

    zip.finish().map_err(|e| format!("完成 zip 失败: {e}"))?;
    Ok(())
}

fn unzip_into(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("打开 zip 失败: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("读取 zip 失败: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("解析条目失败: {e}"))?;
        let name = entry.name().to_string();
        let relative = match name.split_once('/') {
            Some((_, rest)) => rest,
            None => continue,
        };
        if relative.is_empty() {
            continue;
        }
        let outpath = dest_dir.join(relative.replace('/', "\\"));

        if entry.is_dir() {
            std::fs::create_dir_all(&outpath).map_err(|e| format!("创建目录失败: {e}"))?;
        } else {
            if let Some(p) = outpath.parent() {
                std::fs::create_dir_all(p).map_err(|e| format!("创建父目录失败: {e}"))?;
            }
            let mut outfile =
                std::fs::File::create(&outpath).map_err(|e| format!("创建文件失败: {e}"))?;
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("读取 zip 条目失败: {e}"))?;
            outfile
                .write_all(&buf)
                .map_err(|e| format!("写入文件失败: {e}"))?;
        }
    }

    Ok(())
}

fn file_info(zip_path: &Path) -> BackupInfo {
    let meta = std::fs::metadata(zip_path).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let created_at = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
        .map(|d| fmt_iso(d.as_secs()))
        .unwrap_or_default();
    BackupInfo {
        name: zip_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        created_at,
        size_bytes: size,
    }
}

pub fn create(save_dir: &Path, backup_dir: &Path) -> Result<BackupInfo, String> {
    std::fs::create_dir_all(backup_dir).map_err(|e| format!("创建备份目录失败: {e}"))?;
    let name = now_filename();
    let zip_path = backup_dir.join(&name);
    zip_dir(save_dir, &zip_path)?;
    Ok(file_info(&zip_path))
}

pub fn list(backup_dir: &Path) -> Result<Vec<BackupInfo>, String> {
    if !backup_dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(backup_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map(|e| e == "zip").unwrap_or(false) {
            out.push(file_info(&path));
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at)); // 最新优先
    Ok(out)
}

pub fn delete(backup_dir: &Path, name: &str) -> Result<(), String> {
    let path = backup_dir.join(name);
    if !path.exists() {
        return Err(format!("备份不存在: {}", name));
    }
    if path.extension().map(|e| e != "zip").unwrap_or(true) {
        return Err("只允许删除 .zip 文件".to_string());
    }
    std::fs::remove_file(&path).map_err(|e| format!("删除失败: {e}"))
}

/// 恢复指定备份。返回自动保存的快照名（用于回退）。
pub fn restore(
    save_dir: &Path,
    backup_dir: &Path,
    name: &str,
) -> Result<String, String> {
    let zip_path = backup_dir.join(name);
    if !zip_path.exists() {
        return Err(format!("备份不存在: {}", name));
    }

    // 自动保存当前状态
    let auto_name = auto_backup_name();
    let auto_path = backup_dir.join(&auto_name);
    zip_dir(save_dir, &auto_path)?;

    // 恢复
    std::fs::create_dir_all(save_dir).map_err(|e| format!("创建存档目录失败: {e}"))?;
    unzip_into(&zip_path, save_dir)?;

    Ok(auto_name)
}