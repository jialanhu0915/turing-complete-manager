# Integration test for tc-mod-hook
# 1. Start test-target (loads compile.dll, sleeps)
# 2. Inject tc_mod_hook.dll via inject.exe
# 3. Verify marker + status
# 4. Cleanup

$ErrorActionPreference = "Stop"

$root          = "B:\VS_Code_Project\turing-complete-manager\tc-mod-hook"
$compileDll    = "E:\SteamLibrary\steamapps\common\Turing Complete\compile.dll"
$hookDll       = "$root\target\release\tc_mod_hook.dll"
$injectExe     = "$root\target\release\inject.exe"
$testTargetExe = "$root\test-target\target\release\test-target.exe"
$stdoutLog     = "$root\test-target.stdout.log"

Write-Host "=== Stage 1: start test-target ==="
Write-Host "  exe: $testTargetExe"
Write-Host "  arg: $compileDll"

# Use System.Diagnostics.Process directly — Start-Process mangles args with spaces.
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $testTargetExe
$psi.Arguments = '"' + $compileDll + '"'
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError  = $true
$proc = [System.Diagnostics.Process]::Start($psi)
$targetPid = $proc.Id
Write-Host "  test-target PID = $targetPid"

# Give it time to load compile.dll
Start-Sleep -Seconds 2

Write-Host ""
Write-Host "=== Stage 2: inject tc_mod_hook.dll ==="
& $injectExe $targetPid $hookDll
$rc = $LASTEXITCODE
Write-Host "  inject exit code = $rc"

Start-Sleep -Seconds 1

Write-Host ""
Write-Host "=== Stage 3: marker file ==="
$markers = Get-ChildItem -Path $env:TEMP -Filter "tc-mod-hook-*.attached" -ErrorAction SilentlyContinue |
           Sort-Object LastWriteTime -Descending
if ($markers.Count -eq 0) {
    Write-Host "  [FAIL] NO marker found in $env:TEMP" -ForegroundColor Red
} else {
    foreach ($m in $markers | Select-Object -First 3) {
        Write-Host "  [OK] marker: $($m.FullName)"
        Write-Host "  --- contents:"
        Get-Content $m.FullName | ForEach-Object { Write-Host "    | $_" }
        Write-Host ""
    }
}

Write-Host "=== Stage 4: test-target stdout ==="
$proc.Refresh()
if (-not $proc.HasExited) {
    Write-Host "  (process still alive)"
} else {
    Write-Host "  (process exited with code $($proc.ExitCode))"
}

Write-Host ""
Write-Host "=== Stage 5: cleanup ==="
try {
    if (-not $proc.HasExited) {
        Stop-Process -Id $targetPid -Force -ErrorAction Stop
        Write-Host "  test-target (PID $targetPid) stopped"
    } else {
        Write-Host "  test-target already exited"
    }
} catch {
    Write-Host "  test-target already exited"
}

# Cleanup marker files we created
Get-ChildItem -Path $env:TEMP -Filter "tc-mod-hook-*.attached" -ErrorAction SilentlyContinue |
    Where-Object { $_.LastWriteTime -gt (Get-Date).AddMinutes(-5) } |
    Remove-Item -Force
Get-ChildItem -Path $env:TEMP -Filter "tc-mod-hook-*-compile.log" -ErrorAction SilentlyContinue |
    Where-Object { $_.LastWriteTime -gt (Get-Date).AddMinutes(-5) } |
    Remove-Item -Force

Write-Host "  marker/log files cleaned"