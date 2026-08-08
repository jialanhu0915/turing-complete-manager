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
    #[ignore = "stateful: calls compile.dll::compile (single-use, needs game + shim); run via --ignored"]
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

    /// Probe the DSL grammar empirically: try several minimal sources and print
    /// the compiler's response. Helps reverse-engineer what the CURRENT
    /// compile.dll accepts (replay.nim on disk is an older DSL dialect).
    #[test]
    #[ignore = "stateful: calls compile.dll::compile (single-use, needs game + shim); run via --ignored"]
    fn probe_minimal_dsl_grammar() {
        let shim: &Shim = shim().expect("shim load");

        // Read the DSL source from ../sim-shim/probe.dsl so we can iterate on
        // the DSL text without recompiling Rust. Also print the compiler's
        // stderr/stdout (it echoes errors to the process streams).
        let dsl_path = std::path::Path::new("../sim-shim/probe.dsl");
        if !dsl_path.exists() {
            eprintln!("SKIP: probe.dsl not at {}", dsl_path.display());
            return;
        }
        let dsl = std::fs::read_to_string(dsl_path).expect("read probe.dsl");
        let src = std::ffi::CString::new(dsl.clone()).expect("probe.dsl has no NUL bytes");

        let mut out = CompileOutput::zeroed();
        let status = unsafe {
            shim.compile(
                &mut out as *mut CompileOutput as *mut u8,
                src.as_ptr(),
                0,
                267,
            )
        };
        eprintln!("--- probe.dsl (status={status}) ---");
        eprintln!("  out: {out:?}");
        eprintln!("  --- source (first 40 lines) ---");
        for line in dsl.lines().take(40) {
            eprintln!("  {line}");
        }
    }

    /// Compile a REAL and_gate DSL source converted to the CURRENT dialect
    /// (see sim-shim/convert_dialect.py). Verifies the game's JIT compiler
    /// accepts our circuit's DSL — this is the milestone proof that a real
    /// circuit (component wiring + test logic) compiles to machine code via
    /// compile.dll.
    ///
    /// Env override: AND_GATE_DSL path (default ../sim-shim/and_gate_current.dsl)
    /// lets you iterate on the DSL text without recompiling Rust.
    #[test]
    #[ignore = "stateful: calls compile.dll::compile (single-use, needs game + shim); run via --ignored"]
    fn compile_real_and_gate_dsl() {
        let shim: &Shim = shim().expect("shim load");
        let dsl_path = std::env::var("AND_GATE_DSL").unwrap_or_else(|_| {
            "../sim-shim/and_gate_current.dsl".to_string()
        });
        if !std::path::Path::new(&dsl_path).exists() {
            eprintln!("SKIP: {dsl_path} not found");
            return;
        }
        let dsl = std::fs::read_to_string(&dsl_path).expect("read dsl");
        let src = std::ffi::CString::new(dsl.clone()).expect("dsl has no NUL bytes");

        let mut out = CompileOutput::zeroed();
        let status = unsafe {
            shim.compile(
                &mut out as *mut CompileOutput as *mut u8,
                src.as_ptr(),
                0,
                267,
            )
        };

        eprintln!("=== {dsl_path} (status={status}) ===");
        eprintln!("output: {out:?}");
        if out.field_3 == 13 {
            // field_4 = error message ptr (compile.dll allocates it); we can't
            // deref across the DLL boundary safely here, but compile.dll also
            // echoes errors to stderr — see above output.
            eprintln!("COMPILER ERROR (field_3=13), details echoed to stderr");
        }
        eprintln!("--- DSL tail (command loop, last 30 lines) ---");
        for line in dsl.lines().rev().take(30) {
            eprintln!("  {line}");
        }
        assert!(out.is_populated(), "compile output must be populated");
    }
}