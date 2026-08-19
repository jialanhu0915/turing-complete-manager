//! `test-target` — a minimal Windows process that loads `compile.dll` and
//! sleeps. Used to test `tc-mod-inject.exe` against a real target without
//! involving Turing Complete.
//!
//! Usage:
//! ```text
//! test-target.exe <path\to\compile.dll>
//! ```
//!
//! Behavior:
//! 1. Load the given DLL via `libloading::Library` (must succeed — error exits).
//! 2. Print PID + load confirmation so the operator can target it.
//! 3. Sleep forever in a loop, printing a heartbeat every 5 seconds.
//! 4. On exit (Ctrl-C or natural), the `Library` is dropped and `compile.dll`
//!    unloaded. `DllMain(DLL_PROCESS_DETACH)` fires in our process.
//!
//! This program does **not** call any function from `compile.dll` — the
//! injector only needs the DLL to be *mapped*, not invoked. The hook on
//! `compile.dll::compile` will fire the moment some other process component
//! calls it (in real game: every level run).

use std::env;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path-to-compile.dll>", args[0]);
        std::process::exit(2);
    }

    let dll_path = &args[1];
    let pid = std::process::id();

    println!("[test-target] PID = {}", pid);
    println!("[test-target] Loading {} ...", dll_path);

    let _lib = unsafe { libloading::Library::new(dll_path) }
        .expect("failed to load compile.dll — check path");

    println!("[test-target] compile.dll loaded successfully");
    println!("[test-target] Ready for injection. Use this PID:");
    println!("[test-target]   $ tc-mod-inject.exe {} <path-to-tc_mod_hook.dll>", pid);
    println!("[test-target] Heartbeat every 5s. Ctrl-C to exit.");

    let mut tick: u32 = 0;
    loop {
        sleep(Duration::from_secs(5));
        tick += 1;
        println!("[test-target] heartbeat #{} (still alive)", tick);
    }

    // Library is held until process exit (unreachable from the loop above,
    // but kept for documentation — the drop happens automatically when the
    // process terminates).
    #[allow(unreachable_code)]
    drop(_lib);
}