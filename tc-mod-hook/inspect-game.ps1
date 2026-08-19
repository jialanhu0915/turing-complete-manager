# Stage 2: inspect compile log + cleanup
# Use this after running setup-game.ps1 and triggering compile() manually.

$ErrorActionPreference = "Stop"

$root      = "B:\VS_Code_Project\turing-complete-manager\tc-mod-hook"
$pidFile   = "$root\.last-game-pid"

if (-not (Test-Path $pidFile)) {
    Write-Host "No PID file at $pidFile — did you run setup-game.ps1 first?" -ForegroundColor Red
    exit 1
}

$gamePid = Get-Content $pidFile -Raw | ForEach-Object { $_.Trim() } | Select-Object -First 1
Write-Host "Game PID from $pidFile = $gamePid"

# Check if process still alive
$proc = Get-Process -Id $gamePid -ErrorAction SilentlyContinue
if ($null -eq $proc) {
    Write-Host "  game process is no longer running" -ForegroundColor Yellow
} else {
    Write-Host "  game process: $($proc.ProcessName), running"
}

Write-Host ""
Write-Host "=== Marker file ==="
$markerPath = Join-Path $env:TEMP "tc-mod-hook-$gamePid.attached"
if (Test-Path $markerPath) {
    Get-Content $markerPath | ForEach-Object { Write-Host "  | $_" }
} else {
    Write-Host "  (no marker for PID $gamePid)"
}

Write-Host ""
Write-Host "=== Compile log ==="
$logPath = Join-Path $env:TEMP "tc-mod-hook-$gamePid-compile.log"
if (Test-Path $logPath) {
    $lines = Get-Content $logPath
    Write-Host "  log path: $logPath"
    Write-Host "  total entries: $($lines.Count)"
    Write-Host ""
    if ($lines.Count -eq 0) {
        Write-Host "  (empty — compile() never fired)" -ForegroundColor Yellow
    } else {
        Write-Host "  ----- log content -----"
        $lines | ForEach-Object { Write-Host "    | $_" }
        Write-Host "  ----- end -----"
    }
} else {
    Write-Host "  (no log file for PID $gamePid)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "=== Cleanup ==="
try {
    if ($null -ne $proc) {
        Stop-Process -Id $gamePid -Force -ErrorAction Stop
        Write-Host "  game (PID $gamePid) stopped"
    } else {
        Write-Host "  game already exited"
    }
} catch {
    Write-Host "  game stop failed: $_"
}

Start-Sleep -Seconds 1
Remove-Item -Force $pidFile -ErrorAction SilentlyContinue
Write-Host "  PID file removed"