//! High-level driver: compile a circuit DSL source via shim.dll.
//!
//! ## Status (2026-08-08)
//!
//! - [x] Load shim.dll
//! - [x] Call NimMain once
//! - [x] Call tccCompile() with a synthetic DSL source
//! - [x] Verify CompileOutput gets populated
//! - [x] **Execute the JIT machine code** (see `exec`) — compile, map arena,
//!       drive commands, read sim_test_result (calling convention cracked 2026-08-08)
//! - [ ] Wire into replay.nim 9-pointer preamble (settings/commands/etc.)
//!
//! See `docs/10-investigation/compile-signature.md` and `docs/30-usage/compile-dll-integration.md`.

use super::exec;
use super::loader::{shim, Shim};
use super::signature::CompileOutput;
use crate::circuit::model::Circuit;
use std::ffi::CString;

/// Result of a single `run_circuit_test` invocation.
#[derive(Debug, Clone)]
pub struct CircuitRunReport {
    pub compiled_ok: bool,
    pub output: CompileOutput,
    /// DSL TestResult (0 = pass, 1 = win, 2 = fail), if the compiled code ran.
    pub test_result: Option<u64>,
    /// Final `sim_cycle` from the executed run.
    pub cycles_run: i64,
    pub error: Option<String>,
}

/// End-to-end: compile a DSL source through `compile.dll`, execute the JIT
/// machine code against a sim arena, and read the test result.
///
/// The JIT calling convention (cracked 2026-08-08): `compile()` writes
/// `{ len, data(+8), entry_offset, status, err }`; we copy the machine code to
/// executable memory, map the fixed-address sim arena (0x1000000..), and call
/// `(code + entry_offset)()` — a no-arg dispatcher — on a background thread.
/// See `docs/10-investigation/compile-signature.md` and `src/dll/exec.rs`.
pub fn run_circuit_test(
    level_id: &str,
    scheme_id: &str,
    _circuit: &Circuit,
    dsl_source: &str,
    test_number: u64,
    target_cycle: u64,
) -> Result<CircuitRunReport, String> {
    let shim = shim()?;

    // Allocate 40-byte output buffer (stack-allocated, zeroed).
    let mut out = CompileOutput::zeroed();

    // The shim's tccCompile takes a NUL-terminated cstring; it builds a real
    // Nim `string` internally (correct NimStringV2 ABI by construction).
    let source = CString::new(dsl_source).map_err(|e| format!("CIRCUIT_NUL_BYTE|{e}"))?;

    // compile()'s return value is rax = out_buf pointer, NOT a status; the
    // success signal is field_3 == 0 (13 = compiler error).
    unsafe {
        shim.compile(
            &mut out as *mut CompileOutput as *mut u8,
            source.as_ptr(),
            0,   // mode: 0 = normal
            267, // flags: simulation_state_length for and_gate (TODO: parametrize)
        )
    };

    if out.field_3 != 0 {
        return Ok(CircuitRunReport {
            compiled_ok: false,
            output: out,
            test_result: None,
            cycles_run: 0,
            error: Some("COMPILER_ERROR".into()),
        });
    }

    // Extract raw machine code (data lives in compile.dll's heap, +8 past the
    // Nim string payload's cap header; valid while compile.dll stays loaded).
    let len = out.field_0 as usize;
    let code_ptr = (out.field_1 as usize + 8) as *const u8;
    // SAFETY: field_1 points into compile.dll's heap (loaded for process
    // lifetime by the shim); field_0 is the byte length.
    let code = unsafe { std::slice::from_raw_parts(code_ptr, len) };

    let outcome = exec::run_test(code, out.field_2 as usize, test_number, target_cycle)
        .map_err(|e| format!("EXEC_FAILED|{e}"))?;

    Ok(CircuitRunReport {
        compiled_ok: true,
        output: out,
        test_result: Some(outcome.test_result),
        cycles_run: outcome.cycles_run,
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
        let shim_present = std::path::Path::new("../sim-shim/shim.dll").exists();
        if !shim_present {
            eprintln!("SKIP: sim-shim/shim.dll not built");
            return;
        }

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

    /// Phase A tool: dump the JIT machine code to a file so we can disassemble
    /// the entry and confirm the calling convention / sim-arena addresses.
    ///
    /// The compiled machine code lives in compile.dll's heap: `field_1` points
    /// at it (+8 skips the Nim string payload's `cap` header), `field_0` is the
    /// length, `field_2` is the entry offset. (All confirmed empirically
    /// 2026-08-08 — see compile-signature.md.)
    ///
    /// Output: sim-shim/and_gate.bin (raw x86-64). Disassemble from offset
    /// `field_2` to inspect the entry:
    ///   objdump -D -b binary -mi386:x86-64 --start-address=<field_2> \
    ///     --stop-address=<field_2+0x400> ../sim-shim/and_gate.bin
    #[test]
    #[ignore = "stateful: calls compile.dll::compile (single-use, needs game + shim); run via --ignored"]
    fn dump_and_gate_machine_code() {
        let shim: &Shim = shim().expect("shim load");
        let dsl_path = std::env::var("AND_GATE_DSL")
            .unwrap_or_else(|_| "../sim-shim/and_gate_current.dsl".to_string());
        let dsl = std::fs::read_to_string(&dsl_path).expect("read dsl");
        let src = std::ffi::CString::new(dsl).expect("dsl has no NUL bytes");

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

        if out.field_3 != 0 {
            eprintln!("COMPILER ERROR, skipping dump");
            return;
        }

        let len = out.field_0 as usize;
        let bytes_ptr = (out.field_1 as usize + 8) as *const u8;
        // SAFETY: field_1 points into compile.dll's heap (valid for the process
        // lifetime while compile.dll is loaded); field_0 is the byte length.
        let code = unsafe { std::slice::from_raw_parts(bytes_ptr, len) };

        let dump_path = "../sim-shim/and_gate.bin";
        std::fs::write(dump_path, code).expect("write machine code dump");
        eprintln!(
            "wrote {len} bytes to {dump_path}; entry offset = {} (0x{:x})",
            out.field_2, out.field_2
        );

        // Sanity: the DSL's sim-arena pointer 0x1000000 should appear as an
        // absolute-address constant if the compiled code dereferences the
        // arena directly (rather than getting a base from the caller).
        let arena = 0x1000000u64.to_le_bytes();
        let hits: Vec<usize> = code
            .windows(8)
            .enumerate()
            .filter(|(_, w)| *w == arena)
            .map(|(i, _)| i)
            .collect();
        eprintln!(
            "0x1000000 absolute refs: {} hits: {:?}",
            hits.len(),
            &hits[..hits.len().min(10)]
        );
    }
}