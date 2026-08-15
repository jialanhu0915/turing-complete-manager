# tc-mod-sdk

Rust SDK for reading/writing [Turing Complete](https://store.steampowered.com/app/1444480/Turing_Complete/)
circuits and driving the game's own `compile.dll`.

> **Wrapper, not a reimplementation.** This crate wraps the game's data formats and its
> compile pipeline. It never redistributes game binaries — it loads them from the user's
> own installation at runtime.

## Three-layer API

| Layer | What it does | Module |
|---|---|---|
| ① Data format | Decode/encode `circuit.data` (v15 read/write; v7/v13/v14 read-only) | `tc_mod_sdk::circuit` |
| ② Compile | Parse `test.si` → generate DSL → `compile.dll` ABI | `tc_mod_sdk::dll::{test_si, gen, loader, signature}` |
| ③ Sim runtime | JIT-execute the compiled code and read the test result | `tc_mod_sdk::dll::{runtime, exec}` |

## Quick start

```toml
[dependencies]
tc-mod-sdk = "0.1"
```

```rust
use tc_mod_sdk::circuit::codec;
use tc_mod_sdk::dll::{gen, runtime, test_si};

// ① read a circuit.data (v15)
let data = std::fs::read("and_gate.data")?;
let circuit = codec::decode_circuit(&data)?;

// ② parse the level's test.si and generate DSL
let tpl = test_si::parse(&std::fs::read_to_string("and_gate.si")?, "and_gate")?;
let dsl = gen::generate(&circuit, &tpl)?;

// ③ compile + execute, read the test result (0=pass, 1=win, 2=fail)
let report = runtime::run_circuit_test("and_gate", "default", &circuit, &dsl, 0, 2050)?;
println!("test_result = {:?}", report.test_result);
```

A full walkthrough (self-contained, ships with fixtures) is at `examples/and_gate.rs`:

```
cargo run --example and_gate
```

## Prerequisites

- **Windows x64** — the game and `compile.dll` are Windows-only.
- **A Turing Complete install** — needed for layers ②③ (the game's `compile.dll`).
- **`sim-shim/shim.dll`** — built from `sim-shim/shim.nim` (see `sim-shim/build.bat`);
  it wraps `compile.dll::compile` behind a stable C ABI.

Layer ① (the codec) has no game dependency and works standalone on any platform.

## License boundary

- The format codec is reimplemented here under **MIT**, derived from the CC0
  [`save_monger`](https://github.com/Stuffe/save_monger) and MIT `isa_spec` references.
- This crate **does not** bundle `compile.dll`, `replay.nim`, or `game_engine.dll` — those
  are commercial game assets, loaded from the user's installation at runtime.
- It does not modify the game executable or bypass Steam's checks.

## Known limitations

- **Complex circuits** — `gen::generate` handles word I/O and the common gate set, and
  simple-to-moderate levels (`and_gate`, `or_gate`, `not_gate`, `xor_gate`, `full_adder`,
  `bit_adder`, …) resolve correctly, but bit-level carry-lookahead structures (e.g. the
  game's `byte_adder` level) are not yet resolved. Tracked in the repo's `todo/planning/`.
- **Simulation preamble** — layer ③'s `replay.nim` preamble wiring is partially hardcoded
  (`flags = 267`, the `simulation_state_length` for `and_gate`).
- **Thread exit** — a finished run ends the JIT thread via `kernel32.ExitThread`, so
  `join()` reports `threads should not terminate unexpectedly`. Benign: the result is read
  before this fires.

## License

MIT
