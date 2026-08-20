# compare-mc.ps1 — analyze JIT machine code dumps across multiple compile() calls.
#
# Use this AFTER running setup-game + playing several levels + inspect-game.
# Reads all *.bin files in %TEMP%\tc-mod-hook-mc-dump\ and reports:
#   - file size + matched entry_off from log
#   - entry-function byte hash (= fingerprint for "is this the same level?")
#   - groups dumps by fingerprint to detect duplicate compilations of same level

$ErrorActionPreference = "Stop"

$dumpDir = Join-Path $env:TEMP "tc-mod-hook-mc-dump"

$pidFile = Join-Path (Split-Path $PSCommandPath -Parent) ".last-game-pid"
$gamePid = $null
if (Test-Path $pidFile) {
    $gamePid = Get-Content $pidFile -Raw | ForEach-Object { $_.Trim() } | Select-Object -First 1
}
$logFile = if ($gamePid) { Join-Path $env:TEMP "tc-mod-hook-$gamePid-compile.log" } else { $null }

Write-Host "=== MC dump analysis ===" -ForegroundColor Cyan
Write-Host "  dump dir: $dumpDir"
Write-Host "  log file: $logFile"
Write-Host ""

$files = @(Get-ChildItem -Path $dumpDir -Filter "*.bin" -ErrorAction SilentlyContinue |
           Sort-Object Name)

if ($files.Count -eq 0) {
    Write-Host "  [FAIL] no .bin files in $dumpDir" -ForegroundColor Red
    exit 1
}

# --- Parse log into seq → (mc_len, entry_off) map ------------------------------
$logMap = @{}
if ($logFile -and (Test-Path $logFile)) {
    Get-Content $logFile | ForEach-Object {
        $line = $_
        # Match: [post] compile() returned: status=0 mc_len=34874 mc_ptr=0x... entry_off=8521 ...
        if ($line -match '\[post\] compile\(\) returned: status=0 mc_len=(\d+) .* entry_off=(\d+)') {
            $mcLen = [int]$Matches[1]
            $entryOff = [int]$Matches[2]
            $logMap[$mcLen] = $entryOff
        }
    }
}

# If no .last-game-pid was found, try deriving the PID from dump filenames
# (inspect-game.ps1 deletes .last-game-pid during cleanup).
if (-not $logFile -or -not (Test-Path $logFile)) {
    $firstDump = $files | Select-Object -First 1
    if ($firstDump) {
        $parts = $firstDump.BaseName.Split('-')
        if ($parts.Count -ge 1 -and $parts[0] -match '^\d+$') {
            $derivedPid = $parts[0]
            $logFile = Join-Path $env:TEMP "tc-mod-hook-$derivedPid-compile.log"
            if (Test-Path $logFile) {
                Write-Host "  (recovered log from dump filename: $logFile)" -ForegroundColor DarkGray
                Get-Content $logFile | ForEach-Object {
                    $line = $_
                    if ($line -match '\[post\] compile\(\) returned: status=0 mc_len=(\d+) .* entry_off=(\d+)') {
                        $mcLen = [int]$Matches[1]
                        $entryOff = [int]$Matches[2]
                        $logMap[$mcLen] = $entryOff
                    }
                }
            }
        }
    }
}

Write-Host "=== Files ==="
Write-Host ("  {0,-25} {1,8}  {2,12}  {3}" -f "file", "size", "entry_off", "note")
Write-Host ("  {0,-25} {1,8}  {2,12}  {3}" -f "----", "----", "---------", "----")

$rows = @()
foreach ($f in $files) {
    $size = [int]$f.Length  # force Int32 to match $logMap keys
    $name = $f.BaseName
    $parts = $name.Split('-')
    $seq = if ($parts.Count -ge 2) { $parts[1] } else { "?" }

    # entry_off from log (matched by mc_len)
    $entryOff = if ($logMap.ContainsKey($size)) { $logMap[$size] } else { $null }
    $note = ""
    if (-not $entryOff) {
        $note = "(no log match)"
    } elseif ($entryOff -gt $size) {
        $note = "(entry_off > size, suspicious)"
        $entryOff = $null
    }

    $entryFileOff = if ($entryOff) { $entryOff + 8 } else { $null }

    Write-Host ("  {0,-25} {1,8}  {2,12}  {3}" -f $f.Name, $size, $(if ($entryOff) { $entryOff } else { "?" }), $note)

    $rows += [pscustomobject]@{
        file = $f
        size = $size
        seq = $seq
        entryOff = $entryOff
        entryFileOff = $entryFileOff
    }
}

Write-Host ""
Write-Host "=== Entry-function fingerprints ==="
Write-Host "  Hash covers the FIRST 1 KiB starting at file offset (entry_off + 8)."
Write-Host "  Bytes before that are shared helper functions (same across all levels) and"
Write-Host "  the NimStringV2 8-byte cap header — they don't help distinguish levels."
Write-Host ""

$hashGroups = @{}
foreach ($r in $rows) {
    if (-not $r.entryFileOff) { continue }
    $bytes = [System.IO.File]::ReadAllBytes($r.file.FullName)
    $start = [int]$r.entryFileOff
    if ($start -ge $bytes.Length) { continue }
    $len = [Math]::Min(1024, $bytes.Length - $start)
    if ($len -le 0) { continue }
    $slice = $bytes[$start..($start + $len - 1)]
    $hash = [System.Security.Cryptography.SHA256]::HashData($slice)
    $hashHex = -join ($hash | ForEach-Object { $_.ToString("x2") }) | Select-Object -First 16

    if (-not $hashGroups.ContainsKey($hashHex)) {
        $hashGroups[$hashHex] = @()
    }
    $hashGroups[$hashHex] += [pscustomobject]@{
        file = $r.file.Name
        size = $r.size
        entryOff = $r.entryOff
    }
}

$idx = 0
foreach ($k in ($hashGroups.Keys | Sort-Object)) {
    $group = $hashGroups[$k]
    $idx++
    $sizes = ($group | ForEach-Object { $_.size } | Sort-Object -Unique) -join ", "
    $label = if ($group.Count -gt 1) {
        "[DUPLICATE ×$($group.Count)]"
    } else {
        "[unique level]"
    }
    Write-Host ("  Group {0}: hash={1}... size={2} {3}" -f $idx, $k, $sizes, $label) -ForegroundColor Yellow
    foreach ($g in $group) {
        Write-Host ("    {0,-20} size={1,8} entry_off={2}" -f $g.file, $g.size, $g.entryOff)
    }
}

Write-Host ""
Write-Host "=== Summary ==="
Write-Host ("  Total dump files:    {0}" -f $files.Count)
Write-Host ("  Distinct levels:     {0}" -f $hashGroups.Count)
Write-Host ("  Duplicate compilations: {0}" -f ($files.Count - $hashGroups.Count))
Write-Host ""
Write-Host "=== Cross-reference with log ==="
if ($logMap.Count -gt 0) {
    Write-Host "  mc_len values seen in log (unique):"
    foreach ($k in ($logMap.Keys | Sort-Object)) {
        Write-Host ("    mc_len={0,7} → entry_off={1}" -f $k, $logMap[$k])
    }
}

Write-Host ""
Write-Host "=== objdump hints (next step) ==="
Write-Host "  Per level's entry function:"
Write-Host "    objdump -b binary -m i386:x86-64 -D --start-address=<entry_off+8> \"
Write-Host "           <path\to\PID-seq.bin> | less"
Write-Host ""
Write-Host "  Helper function area (shared across levels, file offset 8..entry_off+8):"
Write-Host "    objdump -b binary -m i386:x86-64 -D --start-address=8 \"
Write-Host "           --stop-address=<entry_off+8> <path\to\PID-seq.bin> | less"