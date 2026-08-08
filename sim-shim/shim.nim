# shim.nim — C-ABI shim that wraps compile.dll::compile
#
# Architecture:
#   Rust process ──► shim.dll::tccCompile(...) ──► compile.dll::compile(...)
#                                                     │
#                                                     └─► writes 40-byte result to caller's buffer
#
# We do NOT construct simulator state in Nim — we just pass through the call.
# Rust owns all the sim_state / settings / commands buffers.

# ─── compile.dll forward declarations (4-arg signature per Phase A reverse) ───
# NOTE: cdecl omitted — compile.dll uses Microsoft x64 convention which Nim's
# `importc` defaults to (cdecl on Windows).

proc nimMain() {.importc, dynlib: "compile.dll".}

proc compile(
  outBuf: pointer,
  srcStr: pointer,
  mode: cint,
  flags: cint
): cint {.importc, dynlib: "compile.dll".}
  # Per Phase A: outBuf is filled with 5 fields (40 bytes).
  # The function pointer to invoke the JIT'd code is at offset 0x20.
  # Returns low 32 bits of rax (likely a status enum).

# ─── Our exported C-ABI functions ───

proc tccCompile*(
  outBuf: pointer,
  srcStr: pointer,
  mode: cint,
  flags: cint
): cint {.exportc, dynlib, cdecl.} =
  ## Wraps compile.dll::compile.
  ## Caller (Rust) provides both buffers.
  result = compile(outBuf, srcStr, mode, flags)

proc tccNimMain*() {.exportc, dynlib, cdecl.} =
  ## One-time init. Caller must invoke exactly once before tccCompile.
  nimMain()

proc tccVersion*(): cint {.exportc, dynlib, cdecl.} =
  ## For sanity check: shim.dll loaded successfully.
  return 1