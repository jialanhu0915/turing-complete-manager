//! High-level driver: compile a circuit DSL source via shim.dll.
//!
//! ## Status (2026-08-08)
//!
//! - [x] Load shim.dll
//! - [x] Call NimMain once
//! - [x] Call tccCompile() with a synthetic DSL source
//! - [x] Verify CompileOutput gets populated
//! - [ ] **Execute the JIT function pointer** — pending calling-convention reverse engineering
//! - [ ] Wire into replay.nim 9-pointer preamble (settings/commands/etc.)
//!
//! See `docs/10-investigation/compile-signature.md` and `docs/30-usage/compile-dll-integration.md`.

use super::loader::{shim, Shim};
use super::signature::CompileOutput;
use crate::circuit::model::Circuit;
use std::ffi::CString;

/// Result of a single `run_circuit_test` invocation.
///
/// For now this only reports the compile step. Once Phase D execution lands,
/// `cycles_run` / `test_result` / `error` will be populated.
#[derive(Debug, Clone)]
pub struct CircuitRunReport {
    pub compiled_ok: bool,
    pub output: CompileOutput,
    pub cycles_run: i64,
    pub error: Option<String>,
}

/// Minimal end-to-end demo. Compiles a tiny DSL source through `compile.dll`
/// and verifies the output buffer is populated.
///
/// This does NOT yet invoke the JIT function pointer (call convention TBD).
/// See `docs/10-investigation/compile-signature.md` "已知未解之谜" section.
pub fn run_circuit_test(
    level_id: &str,
    scheme_id: &str,
    _circuit: &Circuit,
    dsl_source: &str,
) -> Result<CircuitRunReport, String> {
    let shim = shim()?;

    // Allocate 40-byte output buffer (stack-allocated, zeroed).
    let mut out = CompileOutput::zeroed();

    // The shim's tccCompile takes a NUL-terminated cstring; it builds a real
    // Nim `string` internally (correct NimStringV2 ABI by construction).
    let source = CString::new(dsl_source).map_err(|e| format!("CIRCUIT_NUL_BYTE|{e}"))?;

    let result = unsafe {
        shim.compile(
            &mut out as *mut CompileOutput as *mut u8,
            source.as_ptr(),
            0,   // mode: 0 = normal
            267, // flags: simulation_state_length for and_gate (TODO: parametrize)
        )
    };

    if result != 0 {
        return Err(format!("COMPILE_FAILED|status={result}"));
    }

    Ok(CircuitRunReport {
        compiled_ok: out.is_populated(),
        output: out,
        // TODO(phase-d-exec): call JIT function pointer, read sim_test_result
        cycles_run: 0,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: invoke the full compile pipeline with a trivial DSL body.
    /// Skips if shim.dll isn't built (CI without game dev env).
    #[test]
    fn end_to_end_compile_invocation() {
        let shim_path = crate::config::default_save_dir();
        let shim_present = std::path::Path::new("../sim-shim/shim.dll").exists();
        if !shim_present {
            eprintln!("SKIP: sim-shim/shim.dll not built");
            return;
        }
        // (Save dir check kept just to mirror the skip-if-no-game pattern;
        // we don't actually need a save dir for shim.)
        let _ = shim_path;

        let shim: &Shim = shim().expect("shim load");
        let mut out = CompileOutput::zeroed();

        // Trivial DSL: just a comment
        let src = std::ffi::CString::new("# smoke test\n").unwrap();

        let status = unsafe {
            shim.compile(
                &mut out as *mut CompileOutput as *mut u8,
                src.as_ptr(),
                0,
                267,
            )
        };

        // We don't assert status == 0 yet — empty source may legitimately fail.
        // What we DO assert: the call returned without segfault, AND the
        // output buffer got touched (some field is non-zero OR status != 0).
        eprintln!("compile() returned status={status}");
        eprintln!("output: {out:?}");
        // Either compile succeeded with populated output, or it failed with a status.
        // Both are acceptable for a smoke test.
        assert!(
            status != 0 || out.is_populated(),
            "compile() neither returned non-zero status nor populated output"
        );
    }
}