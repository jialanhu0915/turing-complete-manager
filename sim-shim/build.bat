@echo off
REM Build shim.dll from shim.nim
REM
REM Toolchain (verified):
REM   Nim 2.2.10 via Scoop (B:\Scoop\apps\nim\current)
REM   MinGW-w64 GCC 16.1.0 UCRT (B:\Scoop\apps\mingw-winlibs\current)
REM
REM compile.dll must be on PATH (or in same dir as shim.dll at runtime)
REM   E:\SteamLibrary\steamapps\common\Turing Complete\compile.dll

setlocal
echo Building shim.dll from shim.nim...
nim c --app:lib --out:shim.dll --cc:gcc --define:release shim.nim
if errorlevel 1 (
    echo BUILD FAILED
    exit /b 1
)
echo.
echo Built: %CD%\shim.dll
echo Run tests via: nim c -r test_shim.nim
endlocal