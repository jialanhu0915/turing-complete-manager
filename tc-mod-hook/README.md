# tc-mod-hook

> Mod SDK for **Turing Complete** (M8-5 of the broader M8 design).
> Hooks `compile.dll::compile` in a running game process so mods can
> register compile-time callbacks. Mods see the exact DSL source the
> game compiles, plus the JIT output (machine code, status, entry offset).
>
> Mods run **inside the game process** as separate `cdylib`s, loaded at
> startup by the host hook DLL.

## Status

**End-to-end working, 2026-08-19.** Real-game PoC verified (Turing Complete
v2.1.305) — mod callbacks fire on every real `compile()` call, content
is consistent with what the game compiled.

| Capability | Status |
|---|---|
| x64 inline trampoline primitive | ✅ |
| `compile.dll::compile` 31-byte hook install | ✅ |
| Trampoline-back (game continues running) | ✅ |
| **Mod callback API** (`Mod` trait + `CompileCtx` + registry) | ✅ |
| Built-in `logger_mod` (writes per-PID log) | ✅ |
| **Dynamic external mod loading** (`%TEMP%\tc-mod-hook-mods\*.dll`) | ✅ |
| Cross-DLL mod registration via `GetProcAddress` | ✅ |
| `external_mod` example with `GetProcAddress` pattern | ✅ |
| **JIT machine code dump to disk** (`%TEMP%\tc-mod-hook-mc-dump\`) | ✅ |
| Multi-level MC fingerprint comparison (`compare-mc.ps1`) | ✅ |
| ModAction::ReplaceDsl (let mods mutate the source) | ❌ |
| AV / Steam-integrity check risk assessment | ❌ |
| Automatic prologue-size detection (replaces hard-coded 31) | ❌ |

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│  Turing Complete.exe (game process)                                  │
│                                                                      │
│  compile.dll:compile()                                               │
│       │                                                              │
│       │  JMP-to-hook (31 bytes patched at function entry)            │
│       ▼                                                              │
│  tc_mod_hook.dll                                                     │
│       │                                                              │
│       │  trampoline-back: original 31 bytes + JMP to compile+31     │
│       │   (game calls this to actually run compile())                │
│       │                                                              │
│  compile_hook (Rust fn in tc_mod_hook.dll)                          │
│       │                                                              │
│       │  pre_compile mods → call trampoline-back → post_compile mods  │
│       │                                                              │
│       │  MOD_REGISTRY: Vec<&'static dyn Mod>                          │
│       │  (one entry per registered mod)                                │
│       │                                                              │
│  ┌────┴────────────┐    ┌─────────────────┐    ┌───────────────┐     │
│  │ logger_mod      │    │ external_mod.dll │    │ (your mod.dll)│    │
│  │ (built-in)      │    │ (example)        │    │               │    │
│  └─────────────────┘    └─────────────────┘    └───────────────┘     │
│   writes per-PID log  writes per-PID log      your custom logic    │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
        ▲
        │  inject.exe creates remote thread in game process
        │  LoadLibraryW(tc_mod_hook.dll) → DllMain → hook install
        │
  tc-mod-inject.exe
        (or any host that calls CreateRemoteThread + LoadLibraryW)
```

## How it works

1. **Inject** loads `tc_mod_hook.dll` into the game via
   `CreateRemoteThread` + `LoadLibraryW` (`tc-mod-hook/src/bin/inject.rs`).
2. **`tc_mod_hook.dll`'s `DllMain`** (PROCESS_ATTACH):
   - Finds `compile.dll` via `GetModuleHandleW` + `GetProcAddress`
   - Installs a 31-byte inline trampoline on `compile()` (covering the
     full prologue: 8 pushes + `sub $0x118,%rsp` + `movaps %xmm6` + `pxor %xmm0`)
   - Allocates a RWX trampoline-back buffer: original 31 bytes +
     `JMP [RIP+0] + <absolute addr of compile+31>`
   - Stores the trampoline-back address in a static `AtomicUsize`
   - Registers the built-in `logger_mod`
   - Scans `%TEMP%\tc-mod-hook-mods\*.dll` and `LoadLibraryW`s each,
     calling their exported `register_mods` (which in turn calls back into
     `tc_mod_hook_register_mod` in this DLL)
3. **Every `compile()` call** now redirects to `compile_hook` in
   `tc_mod_hook.dll`, which:
   - Builds a `CompileCtx` (pid, seq, src ptr/len, mode/flags, out_buf)
   - Calls each registered mod's `pre_compile(ctx)`
   - Calls the trampoline-back to run the original `compile()`
   - Updates ctx with `mc_len`/`mc_ptr`/`status`/`entry_off`
   - Calls each registered mod's `post_compile(ctx)`
   - Logs to the SDK's per-PID log file
   - Dumps JIT machine code to disk

## How to use

### 1. Build everything

```bash
cd tc-mod-hook
cargo build --release --lib               # produces tc_mod_hook.dll
cargo build --release --bin inject         # produces tc-mod-inject.exe
cd external_mod
cargo build --release                     # produces external_mod.dll
```

### 2. Run a quick smoke test (no game needed)

```bash
cd tc-mod-hook
pwsh -ExecutionPolicy Bypass -File integration-test.ps1
```

Loads the DLL into `test-target.exe` (loads `compile.dll`, sits waiting)
and verifies the trampoline fires. Use this for hook-layer regression
testing without launching the full game.

### 3. Run against the real game

```bash
# 1. Launch Turing Complete (Tauri save manager is NOT needed; just the game)
# 2. From this directory:
pwsh -ExecutionPolicy Bypass -File setup-game.ps1 <game-pid> <path-to-tc_mod_hook.dll>
```

`setup-game.ps1`:
- Waits for the game to load `compile.dll`
- Runs `tc-mod-inject.exe <pid> <dll-path>` to inject
- Verifies marker file at `%TEMP%\tc-mod-hook-<pid>.attached` shows
  `registered mods: [...]`

**Important**: the game must already be running before injection. Steam
must be online (game checks for Steam IPC).

### 4. Inspect what the hook captured

```bash
pwsh -ExecutionPolicy Bypass -File inspect-game.ps1
```

Reads:
- The marker file (proves the hook installed)
- `%TEMP%\tc-mod-hook-<pid>-compile.log` (SDK log, pre/post entries)
- `%TEMP%\tc-mod-hook-mods\logger\<pid>-compile.log` (logger_mod output)
- `%TEMP%\tc-mod-hook-mods\external_mod\<pid>-compile.log` (external_mod output)
- `%TEMP%\tc-mod-hook-mc-dump\<pid>-<seq>.bin` (JIT machine code, one file per compile)

Then cleans up the game process and log files.

### 5. Compare across multiple levels

Run setup-game, click 3-5 different levels, then `inspect-game.ps1`,
then `compare-mc.ps1` to see which levels share the same compiled MC
(strong dedup signal: same level = same MC = same SHA256).

## Writing a mod

The pattern is **decoupled from the host SDK binary**. Mods should NOT
statically link to `tc-mod-hook` — doing so causes Windows to load a
duplicate DLL (Cargo copies `tc_mod_hook.dll` to your mod's
`target/<profile>/deps/`, which your mod's PE import table references),
and you end up with two independent module instances whose static
state (including `MOD_REGISTRY`) doesn't sync.

**The correct pattern** (used in `external_mod/`):

```rust
// 1. Don't declare `tc-mod-hook = { ... }` in Cargo.toml.

// 2. Local #[repr(C)] mirror of CompileCtx.
#[repr(C)]
pub struct CompileCtx {
    pub pid: u32,
    pub seq: u32,
    pub src_str_ptr: *const u8,
    pub mode: i32,
    pub flags: i32,
    pub out_buf: *mut u8,
    pub status: u32,
    pub mc_len: u64,
    pub mc_ptr_payload: *const u8,
    pub entry_off: u32,
}

// 3. Raw FFI declaration for the register function.
type RegisterFn = unsafe extern "C" fn(
    /* PCSTR */ *const u8,                                      // name
    Option<unsafe extern "C" fn(*mut c_void, *const CompileCtx) -> i32>,  // pre_fn
    Option<unsafe extern "C" fn(*mut c_void, *const CompileCtx) -> i32>,  // post_fn
    *mut c_void,                                                // user_data
) -> i32;

// 4. Exported entry point — host calls this via GetProcAddress.
#[no_mangle]
pub unsafe extern "C" fn register_mods() {
    let host = GetModuleHandleW("tc_mod_hook.dll");
    let register: RegisterFn = std::mem::transmute(GetProcAddress(host, "tc_mod_hook_register_mod"));
    register(
        b"my-mod\0".as_ptr(),
        Some(my_pre), Some(my_post),
        std::ptr::null_mut(),
    );
}

unsafe extern "C" fn my_pre(_: *mut c_void, ctx: *const CompileCtx) -> i32 {
    // Read `ctx.src_str_ptr` for DSL source, etc.
    0 // Continue
}

unsafe extern "C" fn my_post(_: *mut c_void, ctx: *const CompileCtx) -> i32 {
    // Read `ctx.mc_len` / `ctx.mc_ptr_payload` for JIT output.
    0 // Continue
}
```

### Build the mod

```bash
cargo build --release         # produces target/release/my_mod.dll
```

### Deploy

```bash
cp target/release/my_mod.dll %TEMP%\tc-mod-hook-mods\
```

Next time the game starts and is injected, the host will `LoadLibraryW` it
and call `register_mods`. **One mod = one DLL = one callback pair.**

## Layout and module map

```
tc-mod-hook/
├── Cargo.toml              # crate-type = ["rlib", "cdylib"]
├── src/
│   ├── lib.rs              # top-level docs + re-exports
│   ├── trampoline.rs       # x64 inline JMP (patch size configurable)
│   ├── mod_api.rs          # Mod trait, CompileCtx, registry
│   ├── logger_mod.rs       # built-in example mod (writes %TEMP%/tc-mod-hook-mods/logger/<pid>-compile.log)
│   ├── dll.rs              # DllMain: install hook, build trampoline-back, register mods
│   └── bin/
│       └── inject.rs       # CreateRemoteThread + LoadLibraryW tool
├── tests/trampoline.rs     # inline hook lifecycle test
├── examples/smoke.rs       # standalone example
├── test-target/            # cdylib that loads compile.dll and sits (for integration tests)
│   ├── Cargo.toml
│   └── src/main.rs
├── external_mod/           # example external mod
│   ├── Cargo.toml          # (NO dependency on tc-mod-hook — see "Writing a mod")
│   └── src/lib.rs
├── setup-game.ps1          # launch game + inject + verify marker
├── inspect-game.ps1        # read logs + cleanup
├── compare-mc.ps1          # multi-level MC fingerprint comparison
└── integration-test.ps1    # full pipeline against test-target.exe
```

## What the mod sees (CompileCtx)

```rust
pub struct CompileCtx {
    pub pid: u32,                 // game process id (for log correlation)
    pub seq: u32,                 // per-process sequence number

    // Pre-state (set before trampoline-back runs)
    pub src_str_ptr: *const u8,   // ptr to NimStringV2 { i64 len, *u8 cap_ptr }
    pub mode: i32,                // compile() arg #3
    pub flags: i32,               // compile() arg #4

    // Post-state (filled AFTER original runs)
    pub out_buf: *mut u8,         // 40-byte output struct
    pub status: u32,              // 0 = success, 13 = compiler error
    pub mc_len: u64,              // JIT machine code length
    pub mc_ptr_payload: *const u8,// JIT machine code pointer (NimStringV2 payload)
    pub entry_off: u32,           // entry point offset within MC
}
```

`CompileCtx::src_str()` returns the DSL source as `Option<&[u8]>` (None if
src is null or zero length). `CompileCtx::machine_code()` returns the JIT
output. **Both are slices pointing into the game process's memory** —
read or copy immediately, don't store.

## Known limitations

- **Hard-coded prologue size 31** — `compile.dll::compile`'s prologue is
  31 bytes. If the game updates and the prologue changes, the hook
  patch would split an instruction and crash the game. Detection via
  auto-disassembly is a future improvement.
- **ModAction only Continue / Abort** — no `ReplaceDsl` (mutate source)
  yet. The infrastructure is there (`ModAction` enum), just needs
  `&mut CompileCtx` signature change + trampoline-back to write to
  src_str_ptr.
- **No AV / Steam-integrity testing** — DLL injection into a real game
  may trigger Defender or Steam's anti-cheat. We haven't tested.
- **Single-thread safety on `compile.dll`** — the SDK adds a Mutex around
  MOD_REGISTRY iteration, but the underlying `compile()` itself is not
  thread-safe (game's responsibility).
- **No unload path** — `DLL_PROCESS_DETACH` just logs. No uninstall of
  the trampoline. The game exiting cleans up.

## Roadmap

- [ ] `ModAction::ReplaceDsl` (modifies DSL source before compile)
- [ ] Auto-detect prologue size via disassembly (remove hard-coded 31)
- [ ] AV / Steam-integrity risk assessment
- [ ] README in main project pointing to tc-mod-hook
- [ ] Example mods:
  - [ ] "Logger" that dumps all level DSL to `dsl-dump/<level-id>.dsl`
  - [ ] "Auto-inject" that prepends a `def` to every level's DSL
  - [ ] "Stats" that counts instructions per level
- [ ] M8-6: SDK README + tutorial

## License

MIT. Game DLLs (`compile.dll`, `game_engine.dll`) are **not** distributed
with this crate — the hook targets them at runtime in the user's own
game process, in line with the SDK's "wrapper, not redistributor"
license boundary (see `docs/20-design/M8-mod-sdk.md` §四).