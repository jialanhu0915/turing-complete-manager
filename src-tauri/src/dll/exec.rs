//! Executor for JIT-compiled circuit machine code.
//!
//! Calling convention (reverse-engineered from the game's `jit`/`jit_function`,
//! 2026-08-08 — see `docs/10-investigation/compile-signature.md`):
//!
//! ```text
//! compile() → CompileOutput { len=field_0, data=field_1(+8), entry_off=field_2,
//!                            status=field_3, err=field_4 }
//!   exec  = VirtualAlloc(len, EXECUTE_READWRITE) + copy(data+8, len)
//!   arena = VirtualAlloc(0x1000000, ..., READWRITE)   // DSL preamble addresses
//!   call (exec + entry_off)()                         // NO arguments
//! ```
//!
//! The compiled code is a LONG-RUNNING dispatcher (the DSL's `run_sim` loop),
//! not a one-shot call: it resets the sim state, then blocks polling
//! `commands[ctl_command_id]` and dispatches on `commands[ctl_command]`.
//! So the entry runs on a background thread while this module drives it:
//!
//! ```text
//! 1. pre-write: commands[ctl_command]=run, commands[ctl_command_id]=1,
//!               commands[ctl_test]=<test>, commands[ctl_cycle_speed_ms]=1e13,
//!               settings[sim_target_cycle]=<target>
//! 2. spawn thread → entry()   → reset_sim, mode_refresh, mode_run(target)
//!                               → writes settings[sim_test_result]
//! 3. poll settings[sim_cycle]==target (pass) or sim_test_result==fail
//! 4. write commands[ctl_command]=quit_simulation, ctl_command_id=2 → thread exits
//! ```

use std::time::{Duration, Instant};

// ─── Win32 kernel32 FFI (declared locally; no external dependency needed) ────
unsafe extern "system" {
    fn VirtualAlloc(
        lp_address: *mut u8,
        dw_size: usize,
        fl_allocation_type: u32,
        fl_protect: u32,
    ) -> *mut u8;
    fn VirtualFree(lp_address: *mut u8, dw_size: usize, dw_free_type: u32) -> i32;
    fn GetLastError() -> u32;
}

const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const MEM_RELEASE: u32 = 0x8000;
const PAGE_READWRITE: u32 = 0x04;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;

/// Absolute sim-arena base that the DSL preamble's pointer vars target.
const ARENA_BASE: usize = 0x1_000_000;
/// One contiguous region covering all nine preamble regions. The keyboard
/// buffers are the largest: 0x1000080 + 8192 = 0x1002080. 1 MB gives generous
/// slack (the entry self-zeroes these on every reset).
const ARENA_SIZE: usize = 0x100000;

/// `commands[]` indices (matches the DSL's `CommandIndex`).
mod command {
    pub const CMD: usize = 0;
    pub const CMD_ID: usize = 1;
    pub const CYCLE_SPEED_MS: usize = 2;
    pub const TEST: usize = 6;
}

/// `settings[]` indices (matches the DSL's `StateIndex`).
mod state {
    pub const CYCLE: usize = 1;
    pub const TARGET_CYCLE: usize = 2;
    pub const TEST_RESULT: usize = 3;
    pub const LAST_COMMAND_ID: usize = 4;
    pub const RUNNING: usize = 15;
}

/// DSL `SimCommand` enum values.
const CMD_RUN: u64 = 0;
const CMD_QUIT: u64 = 3;

/// DSL `TestResult` enum values.
pub const TR_PASS: u64 = 0;
pub const TR_WIN: u64 = 1;
pub const TR_FAIL: u64 = 2;

/// ctl_cycle_speed_ms at the pacing cap → mode_run effectively free-runs.
const MAX_CYCLE_SPEED: u64 = 10000000000000;

/// Result of executing a compiled circuit.
#[derive(Debug, Clone, Copy)]
pub struct RunOutcome {
    /// DSL TestResult: 0 = pass, 1 = win, 2 = fail.
    pub test_result: u64,
    /// Final `sim_cycle` (number of simulated cycles processed by mode_run).
    pub cycles_run: i64,
}

/// The sim arena handed to the compiled circuit. Its layout is RELATIVE to
/// `base` (commands at base+0, settings at base+0x10, ...) — the DSL preamble's
/// nine pointer vars are injected with `base + <offset>`, mirroring how the
/// game's `generate_source` inlines `$simulation_commands` etc.
pub struct Arena {
    base: *mut u8,
}

impl Arena {
    /// Map the arena at exactly `ARENA_BASE` (the fixed 0x1000000 convention
    /// used by the extracted DSL's preamble).
    fn alloc() -> Result<Self, String> {
        Self::alloc_at(ARENA_BASE)
    }

    /// Map the arena at a caller-chosen fixed address.
    fn alloc_at(addr: usize) -> Result<Self, String> {
        let p = unsafe {
            VirtualAlloc(
                addr as *mut u8,
                ARENA_SIZE,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };
        if p.is_null() {
            return Err(format!(
                "VirtualAlloc arena@0x{addr:x} failed (err={})",
                unsafe { GetLastError() }
            ));
        }
        if (p as usize) != addr {
            unsafe { VirtualFree(p, 0, MEM_RELEASE) };
            return Err(format!(
                "arena mapped at 0x{:x}, expected 0x{addr:x}",
                p as usize
            ));
        }
        Ok(Arena { base: p })
    }

    /// Map the arena at ANY address. The base must then be injected into the
    /// DSL preamble before compiling (see `inject_preamble_addresses`) — this
    /// avoids the fragile fixed 0x1000000 base and matches the game's own
    /// runtime-pointer inlining.
    pub fn alloc_any() -> Result<Self, String> {
        let p = unsafe {
            VirtualAlloc(
                std::ptr::null_mut(),
                ARENA_SIZE,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };
        if p.is_null() {
            return Err(format!(
                "VirtualAlloc arena (any) failed (err={})",
                unsafe { GetLastError() }
            ));
        }
        Ok(Arena { base: p })
    }

    /// Absolute base address of the arena.
    pub fn base(&self) -> usize {
        self.base as usize
    }

    #[inline]
    fn commands(&self) -> *mut u64 {
        self.base as *mut u64
    }

    #[inline]
    fn settings(&self) -> *mut u64 {
        // DSL preamble: `var settings = Ptr <base + 0x10>`.
        unsafe { self.base.add(0x10) as *mut u64 }
    }

    #[inline]
    fn setting(&self, idx: usize) -> u64 {
        unsafe { *self.settings().add(idx) }
    }

    #[inline]
    fn set_setting(&self, idx: usize, value: u64) {
        unsafe { *self.settings().add(idx) = value };
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        unsafe { VirtualFree(self.base as *mut u8, 0, MEM_RELEASE) };
    }
}

/// Rewrite the DSL preamble's nine fixed arena addresses (0x1000000..0x1000080)
/// to be relative to `base`, mirroring how the game's `generate_source` inlines
/// the runtime `$simulation_commands` etc. pointers. The layout offsets are the
/// fixed DSL preamble spacing (0x10 apart), so `base + offset` reproduces it.
///
/// ```text
/// var commands        = Ptr 0x1000000  →  var commands        = Ptr <base>
/// var settings        = Ptr 0x1000010  →  var settings        = Ptr <base+0x10>
/// ...                                  →  ...
/// ```
pub fn inject_preamble_addresses(dsl: &str, base: usize) -> String {
    let offsets = [0x00, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
    let mut out = dsl.to_string();
    for off in offsets {
        let fixed = format!("0x{:x}", ARENA_BASE + off);
        let injected = format!("0x{:x}", base + off);
        out = out.replace(&fixed, &injected);
    }
    out
}

/// Executable copy of the compiled machine code.
struct ExecCode {
    base: *mut u8,
}

// SAFETY: ExecCode is moved into the entry thread and only touched there; the
// main thread drives the sim arena, never the code buffer.
unsafe impl Send for ExecCode {}

impl ExecCode {
    fn alloc(code: &[u8]) -> Result<Self, String> {
        let p = unsafe {
            VirtualAlloc(
                std::ptr::null_mut(),
                code.len(),
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        };
        if p.is_null() {
            return Err(format!(
                "VirtualAlloc exec failed (err={})",
                unsafe { GetLastError() }
            ));
        }
        // SAFETY: freshly allocated writable region of `code.len()` bytes.
        unsafe { std::ptr::copy_nonoverlapping(code.as_ptr(), p, code.len()) };
        Ok(ExecCode { base: p })
    }

    /// Call the JIT entry (no arguments, Windows x64 ABI). Blocks until the
    /// dispatcher is told to `quit_simulation`.
    ///
    /// # Safety
    ///
    /// The compiled code dereferences the fixed-address arena and absolute
    /// addresses into compile.dll (loaded for the process lifetime by the
    /// shim). Both must be live.
    unsafe fn call_entry(&self, entry_offset: usize) {
        let entry: extern "C" fn() = std::mem::transmute(self.base.add(entry_offset));
        entry();
    }
}

impl Drop for ExecCode {
    fn drop(&mut self) {
        unsafe { VirtualFree(self.base as *mut u8, 0, MEM_RELEASE) };
    }
}

/// Run the compiled circuit for one test in the fixed 0x1000000 arena
/// (the extracted DSL's convention).
///
/// `code` = raw machine-code bytes (CompileOutput field_1 + 8, len = field_0);
/// `entry_offset` = CompileOutput field_2.
pub fn run_test(
    code: &[u8],
    entry_offset: usize,
    test_number: u64,
    target_cycle: u64,
) -> Result<RunOutcome, String> {
    let arena = Arena::alloc()?;
    run_in_arena(&arena, code, entry_offset, test_number, target_cycle)
}

/// Run the compiled circuit for one test inside a caller-provided arena
/// (`inject_preamble_addresses` must have rewritten the DSL to match its base).
pub fn run_in_arena(
    arena: &Arena,
    code: &[u8],
    entry_offset: usize,
    test_number: u64,
    target_cycle: u64,
) -> Result<RunOutcome, String> {
    let exec = ExecCode::alloc(code)?;

    let cmds = arena.commands();
    let sett = arena.settings();

    // Drive protocol (see module docs): write everything before the entry
    // thread starts so the dispatcher's first iteration picks it up.
    unsafe {
        *cmds.add(command::CMD) = CMD_RUN;
        *cmds.add(command::CMD_ID) = 1;
        *cmds.add(command::CYCLE_SPEED_MS) = MAX_CYCLE_SPEED;
        *cmds.add(command::TEST) = test_number;
        *sett.add(state::TARGET_CYCLE) = target_cycle;
    }

    // Run the dispatcher on a background thread (it blocks between commands).
    let handle = std::thread::spawn(move || unsafe { exec.call_entry(entry_offset) });

    // Wait for mode_run to finish. Primary signal: run_sim sets sim_running=1
    // before mode_run and 0 after — track the 1→0 transition. Fallbacks (the
    // poll may miss a sub-ms transition): sim_cycle reaching target (pass) or
    // sim_test_result becoming 2 (fail). NOTE: input_replay aliases
    // settings[2]/[3], so after a PASS settings[3] holds the last input value,
    // not pass(0); only the FAIL path writes a recognizable 2.
    let mut saw_running = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    let result = loop {
        let cycle = arena.setting(state::CYCLE);
        let test_result = arena.setting(state::TEST_RESULT);
        let running = arena.setting(state::RUNNING);
        if running == 1 {
            saw_running = true;
        }
        if (saw_running && running == 0) || cycle == target_cycle || test_result == TR_FAIL {
            break test_result;
        }
        if Instant::now() > deadline {
            let cmd_state: Vec<u64> =
                (0..7).map(|i| unsafe { *cmds.add(i) }).collect();
            eprintln!(
                "run_test: TIMEOUT cycle={cycle:#x} test_result={test_result} running={running} target={target_cycle} commands={cmd_state:?}"
            );
            // Hex dump the whole arena so we can see exactly what the machine
            // code wrote where (commands/settings overlap in the DSL layout).
            eprintln!("--- arena 0x1000000..0x10000a0 ---");
            for row in 0..10 {
                let base = row * 16;
                let bytes: Vec<u64> = (0..2)
                    .map(|i| unsafe { *(cmds as *const u64).add(base / 8 + i) })
                    .collect();
                eprintln!(
                    "  {:04x}: {:016x} {:016x}",
                    base, bytes[0], bytes[1]
                );
            }
            unsafe {
                *cmds.add(command::CMD) = CMD_QUIT;
                *cmds.add(command::CMD_ID) = 2;
            }
            // The compiled code quits via kernel32.ExitThread, which terminates
            // the thread without running Rust's TLS teardown → join() panics
            // with "threads should not terminate unexpectedly". Swallow it.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle.join()));
            return Err(format!(
                "run_test: timeout (cycle={cycle:#x}, test_result={test_result}, target={target_cycle})"
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    };

    // Signal quit_simulation so the dispatcher loop exits and the thread ends.
    unsafe {
        *cmds.add(command::CMD) = CMD_QUIT;
        *cmds.add(command::CMD_ID) = 2;
    }
    // Same ExitThread caveat as the timeout path above.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle.join())) {
        Ok(Ok(())) => eprintln!("exec thread exited cleanly"),
        _ => eprintln!("exec thread terminated abnormally (expected: kernel32.ExitThread on quit)"),
    }

    // Interpret the result: on a FAIL the DSL writes 2 to settings[sim_test_result]
    // (handle_test_result). On a PASS it stays 0 but input_replay[1] aliases
    // settings[3] and holds the LAST input value — so only fail==2 is a real
    // test_result; anything else after a completed run means pass.
    let interpreted = if result == TR_FAIL { TR_FAIL } else { TR_PASS };
    Ok(RunOutcome {
        test_result: interpreted,
        cycles_run: arena.setting(state::CYCLE) as i64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dll::loader::{shim, Shim};
    use crate::dll::signature::CompileOutput;

    /// Extract the raw machine-code bytes from a successful compile output.
    /// The bytes live in compile.dll's heap (field_1, +8 past the Nim string
    /// payload's cap header); field_0 is the length.
    fn machine_code(out: &CompileOutput) -> &'static [u8] {
        assert_eq!(out.field_3, 0, "compile must succeed (field_3=0), got {out:?}");
        let len = out.field_0 as usize;
        let ptr = (out.field_1 as usize + 8) as *const u8;
        // SAFETY: field_1 points into compile.dll's heap, which stays loaded
        // for the process lifetime (the shim holds it).
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    /// Phase A milestone: compile the real and_gate DSL, then EXECUTE the
    /// compiled machine code against a sim arena and read the test result.
    ///
    /// The converted circuit hard-codes `result = 0` (a partial extraction),
    /// so with random inputs the comparison level's `expected` is true ~50% of
    /// the time → the run must end in FAIL deterministically. What we verify
    /// here is the MACHINERY: the code runs, the arena + drive protocol work,
    /// and mode_run advances the sim to a deterministic result.
    #[test]
    #[ignore = "stateful: calls compile.dll::compile + executes JIT code (single-use, needs game + shim); run via --ignored"]
    fn run_and_gate_machine_code() {
        let shim: &Shim = shim().expect("shim load");
        let dsl = std::fs::read_to_string("../sim-shim/and_gate_current.dsl")
            .expect("read and_gate_current.dsl");
        let src = std::ffi::CString::new(dsl).expect("dsl has no NUL bytes");

        let mut out = CompileOutput::zeroed();
        // compile()'s return value is rax = out_buf pointer, NOT a status; the
        // success signal is field_3 == 0 (13 = compiler error).
        let _status = unsafe {
            shim.compile(
                &mut out as *mut CompileOutput as *mut u8,
                src.as_ptr(),
                0,
                267,
            )
        };
        assert_eq!(out.field_3, 0, "compile must succeed, got {out:?}");

        let code = machine_code(&out);
        let entry_offset = out.field_2 as usize;
        eprintln!("code={} bytes, entry_offset={entry_offset} (0x{entry_offset:x})", code.len());

        // Run one test: 3 PRNG-generated inputs, free-running pace.
        let test_number = 0;
        let target_cycle = 3;
        let outcome = run_test(code, entry_offset, test_number, target_cycle)
            .expect("run_test must complete");
        eprintln!(
            "outcome: test_result={} (pass=0/win=1/fail=2) cycles_run={}",
            outcome.test_result, outcome.cycles_run
        );

        // The partial circuit outputs 0 always; a comparison level with
        // ~50% expected-true inputs must FAIL (mode_run halts early).
        assert_eq!(
            outcome.test_result, TR_FAIL,
            "hardcoded-0 circuit must fail random comparison inputs"
        );
        assert!(
            outcome.cycles_run < target_cycle as i64,
            "a failed run must halt before target (cycles_run={})",
            outcome.cycles_run
        );
    }

    /// Verify option B: arena at ANY address, with its base injected into the
    /// DSL preamble before compiling (mirrors the game's `$simulation_*`
    /// inlining). Must behave identically to the fixed 0x1000000 run.
    #[test]
    #[ignore = "stateful: calls compile.dll::compile + executes JIT code (single-use, needs game + shim); run via --ignored"]
    fn run_with_injected_arena_addresses() {
        let shim: &Shim = shim().expect("shim load");
        let dsl_orig = std::fs::read_to_string("../sim-shim/and_gate_current.dsl")
            .expect("read and_gate_current.dsl");

        // Allocate the arena at ANY address, then rewrite the DSL preamble to
        // point at it (like the game inlines $simulation_commands).
        let arena = Arena::alloc_any().expect("arena alloc_any");
        eprintln!("arena base = 0x{:x}", arena.base());
        let dsl = inject_preamble_addresses(&dsl_orig, arena.base());
        let src = std::ffi::CString::new(dsl).expect("dsl has no NUL bytes");

        let mut out = CompileOutput::zeroed();
        let _status = unsafe {
            shim.compile(
                &mut out as *mut CompileOutput as *mut u8,
                src.as_ptr(),
                0,
                267,
            )
        };
        assert_eq!(
            out.field_3, 0,
            "compile must succeed with injected addresses, got {out:?}"
        );

        let code = machine_code(&out);
        let entry_offset = out.field_2 as usize;
        eprintln!("code={} bytes, entry_offset={entry_offset}", code.len());

        let outcome = run_in_arena(&arena, code, entry_offset, 0, 3)
            .expect("run_in_arena must complete");
        eprintln!(
            "outcome: test_result={} (pass=0/win=1/fail=2) cycles_run={}",
            outcome.test_result, outcome.cycles_run
        );

        // Same result as the fixed-address run: the hardcoded-0 circuit fails.
        assert_eq!(
            outcome.test_result, TR_FAIL,
            "injected-address run must fail like the fixed-address run"
        );
    }

    /// Phase 1 spike: the hand-written and_gate DSL (`and_gate_gen.dsl`,
    /// generated from the player's circuit.data) must compile + run to PASS.
    ///
    /// The circuit is `Input{a,b} -> nand -> not -> Output`; `get_input` cycles
    /// the full 2-bit truth table (a=tick&1, b=(tick>>1)&1), so `target_cycle=4`
    /// covers 00, 10, 01, 11. This proves the template+emission approach works
    /// before building the Rust generator.
    #[test]
    #[ignore = "stateful: calls compile.dll::compile + executes JIT code (single-use, needs game + shim); run via --ignored"]
    fn run_hand_generated_and_gate_dsl() {
        let shim: &Shim = shim().expect("shim load");
        let dsl_orig = std::fs::read_to_string("../sim-shim/and_gate_gen.dsl")
            .expect("read and_gate_gen.dsl");
        assert!(
            !dsl_orig.contains("\n\n"),
            "generated DSL must not contain blank lines (dialect restriction)"
        );

        let arena = Arena::alloc_any().expect("arena alloc_any");
        eprintln!("arena base = 0x{:x}", arena.base());
        let dsl = inject_preamble_addresses(&dsl_orig, arena.base());
        let src = std::ffi::CString::new(dsl).expect("dsl has no NUL bytes");

        let mut out = CompileOutput::zeroed();
        let _status = unsafe {
            shim.compile(
                &mut out as *mut CompileOutput as *mut u8,
                src.as_ptr(),
                0,
                267,
            )
        };
        assert_eq!(out.field_3, 0, "compile must succeed, got {out:?}");

        let code = machine_code(&out);
        let entry_offset = out.field_2 as usize;
        eprintln!("code={} bytes, entry_offset={entry_offset}", code.len());

        // Full 2-bit truth table: ticks 0..3 → (0,0),(1,0),(0,1),(1,1).
        let outcome = run_in_arena(&arena, code, entry_offset, 0, 4)
            .expect("run_in_arena must complete");
        eprintln!(
            "outcome: test_result={} (pass=0/win=1/fail=2) cycles_run={}",
            outcome.test_result, outcome.cycles_run
        );
        assert_eq!(
            outcome.test_result, TR_PASS,
            "correct nand+not circuit must pass all 4 truth-table rows"
        );
        assert_eq!(
            outcome.cycles_run, 4,
            "a passing run must complete all target cycles (cycles_run={})",
            outcome.cycles_run
        );
    }

    /// Negative counterpart: dropping the trailing NOT turns the circuit into
    /// NAND(A,B) — wrong for the and_gate level — so the truth table must FAIL.
    /// Proves the harness can tell a correct circuit from a broken one.
    #[test]
    #[ignore = "stateful: calls compile.dll::compile + executes JIT code (single-use, needs game + shim); run via --ignored"]
    fn run_broken_and_gate_dsl_fails() {
        let shim: &Shim = shim().expect("shim load");
        let dsl_orig = std::fs::read_to_string("../sim-shim/and_gate_gen.dsl")
            .expect("read and_gate_gen.dsl");
        // Break the circuit: NOT dropped → output = NAND(A,B), not AND.
        let broken = dsl_orig.replace(
            "var vid_y = U1 (U1 ~(U1 vid_n))",
            "var vid_y = U1 vid_n",
        );
        assert!(
            broken.contains("var vid_y = U1 vid_n"),
            "replacement must land"
        );

        let arena = Arena::alloc_any().expect("arena alloc_any");
        let dsl = inject_preamble_addresses(&broken, arena.base());
        let src = std::ffi::CString::new(dsl).expect("dsl has no NUL bytes");

        let mut out = CompileOutput::zeroed();
        let _status = unsafe {
            shim.compile(
                &mut out as *mut CompileOutput as *mut u8,
                src.as_ptr(),
                0,
                267,
            )
        };
        assert_eq!(out.field_3, 0, "compile must succeed, got {out:?}");

        let code = machine_code(&out);
        let entry_offset = out.field_2 as usize;
        let outcome = run_in_arena(&arena, code, entry_offset, 0, 4)
            .expect("run_in_arena must complete");
        eprintln!(
            "broken outcome: test_result={} cycles_run={}",
            outcome.test_result, outcome.cycles_run
        );
        assert_eq!(
            outcome.test_result, TR_FAIL,
            "NAND-only circuit must fail the and_gate truth table"
        );
    }
}
