//! Game install detection: returns the path to `Turing Complete.exe` when a
//! Steam library contains it, plus `compile.dll` and `campaign/`. The testing
//! feature (`test_circuit` tauri command) is gated on this — without the
//! game's `compile.dll` + `campaign/<level>/test.si` we can't test circuits.
//!
//! Reuses `translations::detect_game_dir` (Steam registry + libraryfolders.vdf
//! scan, already wired for the level-name translations).

use std::path::{Path, PathBuf};

/// Locate the game install directory. Returns `None` if no Steam library
/// contains `Turing Complete.exe` + `compile.dll` + `campaign/`.
pub fn detect() -> Option<PathBuf> {
    let dir = crate::translations::detect_game_dir()?;
    if !is_complete_install(&dir) {
        return None;
    }
    Some(dir)
}

/// `true` iff the game directory has everything testing needs.
pub fn is_available() -> bool {
    detect().is_some()
}

/// `true` iff `dir` has `compile.dll` and a `campaign/` subdirectory.
fn is_complete_install(dir: &Path) -> bool {
    dir.join("compile.dll").is_file() && dir.join("campaign").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Env var `TC_GAME_DIR` can override detection (tests + CI).
    fn detect_or_override() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("TC_GAME_DIR") {
            let dir = PathBuf::from(p);
            return is_complete_install(&dir).then_some(dir);
        }
        detect()
    }

    #[test]
    #[ignore = "requires a Steam library with Turing Complete installed; run via --ignored"]
    fn detects_a_real_install() {
        let dir = detect_or_override().expect("game not found");
        assert!(dir.join("compile.dll").is_file(), "compile.dll missing");
        assert!(dir.join("campaign").is_dir(), "campaign/ missing");
        assert!(dir.join("Turing Complete.exe").is_file());
    }

    #[test]
    fn rejects_incomplete_install() {
        let tmp = std::env::temp_dir().join("tc-fake-game");
        let _ = std::fs::create_dir_all(&tmp);
        assert!(!is_complete_install(&tmp));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Stub: an `is_complete_install` invariant table for kind-of-game dirs.
    #[test]
    fn complete_install_predicate() {
        let cases = HashMap::from([
            ("has-compile-and-campaign", true),
            ("has-compile-only", false),
            ("has-campaign-only", false),
            ("empty", false),
        ]);
        let tmp = std::env::temp_dir().join("tc-test-install");
        for (label, expected) in cases {
            let dir = tmp.join(label);
            std::fs::create_dir_all(&dir).unwrap();
            if label == "has-compile-and-campaign" || label == "has-compile-only" {
                std::fs::write(dir.join("compile.dll"), []).unwrap();
            }
            if label == "has-compile-and-campaign" || label == "has-campaign-only" {
                std::fs::create_dir_all(dir.join("campaign")).unwrap();
            }
            assert_eq!(
                is_complete_install(&dir),
                expected,
                "label={label}"
            );
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
