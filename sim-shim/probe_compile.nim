# probe_compile.nim — minimal Nim test that mimics the game's call to compile.dll
#
# The game (Turing Complete.exe) does:
#   lib = loadLib("compile.dll")
#   nimMain = symAddr(lib, "NimMain"); nimMain()
#   compile = symAddr(lib, "compile"); compile(outBuf, code, mode, flags)
#
# This probe reproduces that exactly, so we can learn the correct string ABI
# without the Rust/shim layer in between.

import std/[dynlib, strutils]

when defined(windows):
  type
    HMODULE = pointer
    FARPROC = pointer
  proc LoadLibraryA(name: cstring): HMODULE {.importc, dynlib: "kernel32.dll".}
  proc GetProcAddress(h: HMODULE, name: cstring): FARPROC {.importc, dynlib: "kernel32.dll".}

let path = r"E:\SteamLibrary\steamapps\common\Turing Complete\compile.dll"
let lib = LoadLibraryA(path)
if lib == nil:
  echo "FAIL: could not load ", path
  quit(1)
echo "compile.dll loaded at ", cast[int](lib)

let nimMainAddr = GetProcAddress(lib, "NimMain")
let compileAddr  = GetProcAddress(lib, "compile")
echo "NimMain @ ", cast[int](nimMainAddr)
echo "compile  @ ", cast[int](compileAddr)

type
  NimMainProc = proc () {.cdecl.}
  CompileProc = proc (outBuf: pointer, src: string, mode: int32, flags: int32): int32 {.cdecl.}

let nimMain = cast[NimMainProc](nimMainAddr)
nimMain()
echo "NimMain called OK"

let compile = cast[CompileProc](compileAddr)
var outBuf: array[40, byte]
let src = "def main() Int { return 1 }"
echo "src string len=", src.len, " addr=", cast[int](addr src)

echo "--- calling compile ---"
let status = compile(addr outBuf, src, 0, 267)
echo "compile returned status=", status
echo "outBuf[0..39] = ", outBuf
