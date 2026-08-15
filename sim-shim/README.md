# sim-shim

Nim C-ABI shim DLL that wraps `compile.dll::compile` from Turing Complete game.

## Purpose

`turing-complete-manager` uses this shim to drive the game's actual JIT compiler
(`compile.dll`) so that LLM-generated circuits can be verified by the same engine
the game uses — avoiding the systematic divergence between our offline simulator
(`tc-save-lab/simulate.py`) and the in-game one.

## Architecture

```
┌────────────────────────────────────┐
│ turing-complete-manager (Rust/Tauri)│
│   └── src-tauri/src/dll/runtime.rs  │
└──────────────┬─────────────────────┘
               │ libloading
               ▼
┌────────────────────────────────────┐
│ shim.dll (this directory)          │
│   ├── tccNimMain()   init once     │
│   ├── tccCompile()   pass through  │
│   └── tccVersion()   sanity check  │
└──────────────┬─────────────────────┘
               │ dynlib
               ▼
┌────────────────────────────────────┐
│ compile.dll (game's Nim JIT)       │
│   ├── NimMain()                     │
│   ├── NimDestroyGlobals()           │
│   └── compile()  ← 4 args,         │
│                   40-byte output   │
└────────────────────────────────────┘
```

## Files

| File | Purpose |
|---|---|
| `shim.nim` | Source. Just importc-forwards `compile.dll::compile`, exports `tccCompile`/`tccNimMain`/`tccVersion` |
| `build.bat` | `nim c --app:lib --out:shim.dll shim.nim` |
| `shim.dll` | Built artifact (137 KB at last build) |
| `README.md` | This file |

## Build

Requirements:
- Nim 2.2.10 (verified locally via Scoop)
- MinGW-w64 GCC (UCRT) — Nim's default backend

```cmd
cd sim-shim
build.bat
```

Output: `sim-shim/shim.dll`

## Runtime requirements

`compile.dll` must be loadable by `shim.dll` at runtime. Two options:

1. **Same directory**: Copy `compile.dll` (and its `libgcc_s_seh-1.dll`,
   `libwinpthread-1.dll`) into `sim-shim/` next to `shim.dll`.
2. **On PATH**: Make sure the game install dir
   (`E:\SteamLibrary\steamapps\common\Turing Complete\`) is on PATH.

## Exports

```c
int tccNimMain(void);           // call once before tccCompile
int tccCompile(void* outBuf,    // 40-byte result struct (caller-allocated)
               void* srcStr,    // Nim string {int64 length, ptr char_data}
               int   mode,      // low 32 bits; 0 = normal, 1 = log tokens
               int   flags);    // low 32 bits; semantics TBD
int tccVersion(void);           // returns 1 if loaded successfully
```

For argument semantics, see `docs/10-investigation/compile-signature.md`.

## Status (2026-08-08)

- [x] Shim compiles (`nim c --app:lib --out:shim.dll shim.nim`)
- [x] Exports verified via `objdump -p shim.dll` — 3 functions
- [ ] **tccCompile() actually invokes compile.dll successfully** — pending Rust test (Phase D)
- [ ] **JIT function pointer call convention** — unknown; need to write Nim test or trace exe runtime

## Why no `simulator_types.nim` / `native_alloc.nim` stubs?

Initial plan called for these stubs. After Phase A reverse engineering showed that
`compile.dll::compile` takes only C-ABI args (no Nim types cross the boundary),
we don't need them. The shim is a pure pass-through. See
`docs/10-investigation/compile-signature.md` for the analysis.

If we later need to **execute** the compiled JIT machine code (vs just produce it),
we'll need to either:
- Reverse-engineer the exe's `jit_function` to learn the calling convention, or
- Call directly into `compile.dll::compile`'s output buffer's function pointer field
  after loading it via `VirtualAlloc` + memcpy + `VirtualProtect(PAGE_EXECUTE)`.