# strlayout.nim — inspect how Nim represents `string` in C (oracle)
# Compile with -c and grep nimcache for NimStringDesc / NimStringV2.

proc foreign(outBuf: pointer, s: string, mode: int32, flags: int32): int32
  {.importc: "foreign", cdecl.}

var buf: array[40, byte]
let code = "def main() Int { return 1 }"
discard foreign(addr buf, code, 0, 267)
