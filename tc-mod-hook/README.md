# tc-mod-hook

> Hook layer for the Turing Complete mod SDK (M8-5).
> Hijack `compile.dll::compile` in a running game process so mods can
> register compile-time callbacks.

## Status

**PoC, 2026-08-19.** See `docs/20-design/M8-mod-sdk.md` §五 for the milestone.

This PoC delivers:

- Inline x64 trampoline primitive (`install_inline_hook` /
  `uninstall_inline_hook` in [`src/trampoline.rs`](src/trampoline.rs)).
- `DllMain` for the `cdylib` build ([`src/dll.rs`](src/dll.rs)).
- Unit test on a fake function ([`tests/trampoline.rs`](tests/trampoline.rs)).
- Standalone smoke example ([`examples/smoke.rs`](examples/smoke.rs)).

This PoC does **not** yet deliver:

- DLL injection (`CreateRemoteThread` + `LoadLibrary`) — separate tool, next phase.
- Mod callback API (`Mod` trait, `PreCompile` / `PostCompile` pipeline).
- Hooking the actual `compile.dll::compile` symbol (requires runtime config
  handoff to the injected DLL).

## How to build & test

```bash
cd tc-mod-hook
cargo test             # run the unit test (Windows only)
cargo run --example smoke
```

To build the actual DLL for injection:

```bash
cargo build --release --lib  # produces tc-mod-hook.dll in target/release/
```

## What the trampoline does

Replaces the first 14 bytes of a target function with an absolute JMP:

```
FF 25 00 00 00 00       ; JMP [RIP+0]   (6 bytes)
<8-byte absolute addr>  ; replacement   (8 bytes)
```

After install, every call to the target jumps to the replacement instead.
On uninstall, the original 14 bytes are restored.

## License

MIT. Game DLLs (`compile.dll` etc.) are **not** distributed with this crate
— the hook targets them at runtime in the user's own game process, in line
with the SDK's "wrapper, not redistributor" license boundary
(see `docs/20-design/M8-mod-sdk.md` §四).