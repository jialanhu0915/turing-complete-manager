# Stage 1: launch game + inject + leave running.
# Run this script, then MANUALLY click a level in the game.
# After you've triggered a level, run inspect-game.ps1 to see the log.

$ErrorActionPreference = "Stop"

$root          = "B:\VS_Code_Project\turing-complete-manager\tc-mod-hook"
$tcExe         = "E:\SteamLibrary\steamapps\common\Turing Complete\Turing Complete.exe"
$tcDir         = "E:\SteamLibrary\steamapps\common\Turing Complete"
$hookDll       = "$root\target\release\tc_mod_hook.dll"
$injectExe     = "$root\target\release\inject.exe"

# Pre-cleanup old markers/logs (older than 5 min)
Get-ChildItem -Path $env:TEMP -Filter "tc-mod-hook-*.attached" -ErrorAction SilentlyContinue |
    Where-Object { $_.LastWriteTime -lt (Get-Date).AddMinutes(-5) } | Remove-Item -Force
Get-ChildItem -Path $env:TEMP -Filter "tc-mod-hook-*-compile.log" -ErrorAction SilentlyContinue |
    Where-Object { $_.LastWriteTime -lt (Get-Date).AddMinutes(-5) } | Remove-Item -Force

Write-Host "=== Stage 1: launch Turing Complete ==="
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $tcExe
$psi.WorkingDirectory = $tcDir
$psi.UseShellExecute = $false
$proc = [System.Diagnostics.Process]::Start($psi)
$gamePid = $proc.Id
Write-Host "  game PID = $gamePid"

Write-Host ""
Write-Host "=== Stage 2: wait for game UI (15s) ==="
Start-Sleep -Seconds 15
if ($proc.HasExited) {
    Write-Host "  [FAIL] game exited early with code $($proc.ExitCode)" -ForegroundColor Red
    exit 1
}
Write-Host "  game still alive"

Write-Host ""
Write-Host "=== Stage 3: inject tc_mod_hook.dll ==="
& $injectExe $gamePid $hookDll
$rc = $LASTEXITCODE
Write-Host "  inject exit code = $rc"

Start-Sleep -Seconds 2

Write-Host ""
Write-Host "=== Stage 4: marker file ==="
$markers = Get-ChildItem -Path $env:TEMP -Filter "tc-mod-hook-*.attached" -ErrorAction SilentlyContinue |
           Sort-Object LastWriteTime -Descending
if ($markers.Count -eq 0) {
    Write-Host "  [FAIL] NO marker found in $env:TEMP" -ForegroundColor Red
    exit 1
}
$latest = $markers[0]
Write-Host "  [OK] marker: $($latest.FullName)"
Get-Content $latest.FullName | ForEach-Object { Write-Host "    | $_" }

Write-Host ""
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "Game is RUNNING with hook + trampoline-back installed." -ForegroundColor Cyan
Write-Host "" -ForegroundColor Cyan
Write-Host "NEXT STEPS:" -ForegroundColor Cyan
Write-Host "  1. Open Turing Complete (it's already running, PID $gamePid)" -ForegroundColor Cyan
Write-Host "  2. Click any level — anything that runs a circuit should trigger" -ForegroundColor Cyan
Write-Host "  3. Run inspect-game.ps1 to see what the hook captured" -ForegroundColor Cyan
Write-Host "" -ForegroundColor Cyan
Write-Host "LOG FILE:" -ForegroundColor Cyan
Write-Host "  $env:TEMP\tc-mod-hook-$gamePid-compile.log" -ForegroundColor Cyan
Write-Host "" -ForegroundColor Cyan
Write-Host "Game will NOT be auto-killed. Stop it yourself with taskkill /F /PID $gamePid" -ForegroundColor Cyan
Write-Host "or run inspect-game.ps1 which cleans up." -ForegroundColor Cyan
Write-Host "===============================================" -ForegroundColor Cyan

# Save PID for the inspect script
$gamePid | Out-File -FilePath "$root\.last-game-pid" -Encoding ASCII -NoNewline
Write-Host ""
Write-Host "Game PID saved to $root\.last-game-pid"