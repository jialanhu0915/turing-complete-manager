# shim.nim — C-ABI shim that wraps compile.dll::compile
#
# Architecture:
#   Rust process ──► shim.dll::tccCompile(...) ──► compile.dll::compile(...)
#                                                     │
#                                                     └─► writes 40-byte result to caller's buffer
#
# We do NOT construct simulator state in Nim — we just pass through the call.
# Rust owns all the sim_state / settings / commands buffers.

import std/[dynlib, os, strutils]

# ─── Locate compile.dll ───
#
# Search order:
#   1. $COMPILE_DLL_PATH env var (set by Rust loader for explicit control)
#   2. <shim-dir>/compile.dll (next to shim.dll)
#   3. <game-install-dir>/compile.dll  (E:\SteamLibrary\...\Turing Complete\)
#      — auto-detected via Steam registry; falls back to well-known path
#
# We load compile.dll explicitly with an absolute path so Windows DLL search
# order doesn't matter. This is more robust than `importc, dynlib: "compile.dll"`
# which depends on PATH.
const GameInstallGuess = "E:/SteamLibrary/steamapps/common/Turing Complete/compile.dll"

proc findCompileDll(): string =
  ## Returns absolute path to compile.dll, or empty if not found.
  let envPath = getEnv("COMPILE_DLL_PATH", "")
  if envPath.len > 0 and fileExists(envPath):
    return envPath
  # Same dir as shim.dll (relative to this source's build output).
  let here = getAppDir() & "compile.dll"
  if fileExists(here):
    return here
  if fileExists(GameInstallGuess):
    return GameInstallGuess
  return ""

# ─── Explicit LoadLibraryA + GetProcAddress ───
#
# Using dynlib with an absolute path is more reliable than `importc, dynlib`
# because Nim's `importc, dynlib` resolves at load time via dlopen-style search.

when defined(windows):
  type
    HMODULE = pointer
    FARPROC = pointer
  proc LoadLibraryA(name: cstring): HMODULE {.importc, dynlib: "kernel32.dll".}
  proc GetProcAddress(handle: HMODULE, name: cstring): FARPROC {.importc, dynlib: "kernel32.dll".}

var
  compileMod: HMODULE = nil
  compileFnPtr: FARPROC = nil
  nimMainPtr: FARPROC = nil

proc loadCompileDll(): bool =
  if compileMod != nil: return true
  let path = findCompileDll()
  if path.len == 0:
    return false
  compileMod = LoadLibraryA(path.cstring)
  if compileMod == nil:
    return false
  nimMainPtr = GetProcAddress(compileMod, "NimMain")
  compileFnPtr = GetProcAddress(compileMod, "compile")
  return nimMainPtr != nil and compileFnPtr != nil

type
  CompileProc = proc (
    outBuf: pointer,
    src: pointer,     # pointer to NimStringV2 { NI len; NimStrPayload* p }
    mode: cint,
    flags: cint
  ): cint {.cdecl.}

  NimMainProc = proc (): cint {.cdecl.}

# ─── Our exported C-ABI functions ───

proc tccCompile*(
  outBuf: pointer,
  src: cstring,
  mode: cint,
  flags: cint
): cint {.exportc, dynlib, cdecl.} =
  ## Wraps compile.dll::compile.
  ##
  ## compile.dll expects arg2 = POINTER to a NimStringV2 struct
  ## { NI len; NimStrPayload* p; } where NimStrPayload = { cap; char data[] }.
  ## So we build a real Nim `string` here (correct ABI by construction) and
  ## pass `addr code`. Building the string in Nim avoids hand-crafting the
  ## layout in Rust.
  if not loadCompileDll():
    return -1  # DLL_LOAD_FAILED
  # NimMain is auto-called by compile.dll's DllMain on LoadLibraryA
  # (Nim `--app:lib`). Calling it AGAIN here corrupts the
  # `source_buffer.len == 0` "Only call init once" invariant, so we do NOT
  # invoke it explicitly. Verified 2026-08-08: removing the explicit call
  # lets compile() run past the reset_globals assertion.
  var code = $src
  let fn = cast[CompileProc](compileFnPtr)
  result = fn(outBuf, addr code, mode, flags)

proc tccNimMain*() {.exportc, dynlib, cdecl.} =
  ## One-time init. **Currently a no-op** — NimMain is invoked lazily from
  ## tccCompile, in the same execution path the game uses. Calling NimMain
  ## here breaks the source_buffer.len == 0 invariant inside reset_globals.
  discard

proc tccVersion*(): cint {.exportc, dynlib, cdecl.} =
  ## Returns 1 if shim.dll loaded. compile.dll check is lazy.
  return 1

proc tccCompileDllPath*(buf: cstring, bufLen: cint): cint
    {.exportc, dynlib, cdecl.} =
  ## Diagnostic: write the compile.dll path that would be loaded into buf.
  ## Returns 1 if path exists, 0 otherwise.
  let path = findCompileDll()
  if path.len == 0:
    return 0
  if path.len + 1 > bufLen.int:
    return -1
  copyMem(buf, path.cstring, path.len + 1)
  return 1