//! `test` CLI: a process-isolated driver for the circuit test pipeline.
//!
//! ```text
//! test --game <game dir> --save <save dir> --level <level> --scheme <scheme>
//! ```
//!
//! Each invocation loads `compile.dll` + `sim-shim.dll` into a fresh process,
//! generates a circuit DSL from the saved circuit + the game's `test.si`,
//! compiles + executes it, and prints a JSON result to stdout. stdout
//! protocol keeps the CLI and its tauri command spectrally separate: stdout
//! is JSON only, all diagnostics go to stderr (or are suppressed). Exit code
//! is 0 on a successful run (regardless of pass/fail), non-zero on internal
//! failure.
//!
//! Why a separate process: `compile.dll` is single-shot per process. A
//! long-running tauri app that loaded it directly would only get one
//! test call before failing forever. Spawning a fresh `test` per
//! request side-steps this.

use std::path::PathBuf;
use std::process::ExitCode;

use tc_mod_sdk::circuit;
use tc_mod_sdk::dll::{gen, test_si};

#[derive(serde::Serialize)]
struct Output {
    ok: bool,
    test_result: u64,
    cycles_run: i64,
    error: Option<String>,
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("usage: test --game <dir> --save <dir> --level <id> --scheme <name>");
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    // Run the full pipeline in this process. compile.dll is loaded on first
    // call; the process exits after, so the single-shot constraint is moot.
    let result = run(&args);
    let out = match result {
        Ok(r) => Output {
            ok: true,
            test_result: r.test_result,
            cycles_run: r.cycles_run,
            error: None,
        },
        Err(e) => Output {
            ok: false,
            test_result: 0,
            cycles_run: 0,
            error: Some(e),
        },
    };
    match serde_json::to_string(&out) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("test: failed to serialize output: {e}");
            return ExitCode::from(1);
        }
    }
    if out.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

struct Args {
    game: PathBuf,
    save: PathBuf,
    level: String,
    scheme: String,
}

/// Save directory layout: `<save>/schematics/<level>/<scheme>/circuit.data`. The
/// `schematics/` segment is the convention used by both the GUI's
/// `list_schematics` / `read_circuit` and the live game.
const SCHEMATICS_SUBDIR: &str = "schematics";

fn parse_args() -> Result<Args, String> {
    let mut game: Option<PathBuf> = None;
    let mut save: Option<PathBuf> = None;
    let mut level: Option<String> = None;
    let mut scheme: Option<String> = None;
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let val = it
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--game" => game = Some(PathBuf::from(val)),
            "--save" => save = Some(PathBuf::from(val)),
            "--level" => level = Some(val),
            "--scheme" => scheme = Some(val),
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    Ok(Args {
        game: game.ok_or("--game is required")?,
        save: save.ok_or("--save is required")?,
        level: level.ok_or("--level is required")?,
        scheme: scheme.ok_or("--scheme is required")?,
    })
}

fn run(args: &Args) -> Result<RunSummary, String> {
    // 1. Read the saved circuit.
    let circuit_path = args
        .save
        .join(SCHEMATICS_SUBDIR)
        .join(&args.level)
        .join(&args.scheme)
        .join("circuit.data");
    let bytes = std::fs::read(&circuit_path)
        .map_err(|e| format!("CIRCUIT_READ_FAILED|{}|{e}", circuit_path.display()))?;
    let circuit = circuit::codec::decode_circuit(&bytes)
        .map_err(|e| format!("CIRCUIT_DECODE_FAILED|{e}"))?;

    // 2. Parse the test.si template.
    let tpl = test_si::parse_level(&args.game, &args.level)
        .map_err(|e| format!("TEST_SI_FAILED|{e}"))?;

    // 3. Generate the DSL.
    let dsl = gen::generate(&circuit, &tpl)
        .map_err(|e| format!("GEN_FAILED|{e}"))?;

    // 4. Compile + execute via the high-level runtime. target_cycle = 2050
    //    covers the PRNG/`cycle` truth tables every test.si uses.
    let report = tc_mod_sdk::dll::runtime::run_circuit_test(
        &args.level,
        &args.scheme,
        &circuit,
        &dsl,
        0,
        2050,
    )
    .map_err(|e| format!("EXEC_FAILED|{e}"))?;

    Ok(RunSummary {
        test_result: report.test_result.unwrap_or(0),
        cycles_run: report.cycles_run,
    })
}

struct RunSummary {
    test_result: u64,
    cycles_run: i64,
}
