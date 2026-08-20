//! Example mod: `LoggerMod` — logs every compile() invocation to a per-PID
//! file in `%TEMP%\tc-mod-hook-mods\logger\<pid>-compile.log`.
//!
//! Demonstrates the mod callback API. Real mod authors would create their
//! own mod files in this directory and add a similar `register()` call.
//!
//! Each line: `<ts> <seq> <src_len> <mc_len> <status> <entry_off> <mod_name>`

#![cfg(windows)]

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::mod_api::{register_mod, CompileCtx, Mod, ModAction};

/// The mod instance. Single global, registered in `register()`.
struct LoggerMod;

impl Mod for LoggerMod {
    fn name(&self) -> &str {
        "logger"
    }

    fn post_compile(&self, ctx: &CompileCtx) -> ModAction {
        // Use ctx.seq + ctx.status + ctx.mc_len as a quick "did anything
        // change?" sanity check.
        let _ = writeln_to_log(
            ctx.pid,
            format_args!(
                "<ts={}> seq={} src_len={} mc_len={} status={} entry_off={} mod={}",
                unix_now(),
                ctx.seq,
                ctx.src_str().map(|s| s.len()).unwrap_or(0),
                ctx.mc_len,
                ctx.status,
                ctx.entry_off,
                self.name(),
            ),
        );
        ModAction::Continue
    }
}

static LOGGER_MOD: LoggerMod = LoggerMod;

/// Called from `DllMain` (or wherever) to register this mod. After this
/// returns, the logger is part of the active mod list and will see every
/// compile() call.
pub fn register() {
    register_mod(&LOGGER_MOD);
}

// ---- helpers ---------------------------------------------------------------

fn writeln_to_log(pid: u32, args: std::fmt::Arguments) -> std::io::Result<()> {
    let path = log_path(pid);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{}", args)
}

fn log_path(pid: u32) -> PathBuf {
    std::env::var_os("TEMP")
        .or_else(|| std::env::var_os("TMP"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows\\Temp"))
        .join("tc-mod-hook-mods")
        .join("logger")
        .join(format!("{}-compile.log", pid))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}