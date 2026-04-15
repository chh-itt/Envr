#Requires -Version 5.1
<#
.SYNOPSIS
  One-click benchmark for shim settings hot path.

.DESCRIPTION
  Runs `cargo bench -p envr-shim-core --bench shim_settings_snapshot -- --noplot`,
  parses Criterion output, and prints optimization delta:
  - legacy_multi_load_cold_process
  - single_snapshot_cold_process

.PARAMETER RepoRoot
  Repository root. Default: two levels above this script.

.PARAMETER AdditionalBenchArgs
  Extra args appended after `--` for criterion (optional).
#>
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path,
    [string[]]$AdditionalBenchArgs = @("--noplot")
)

$ErrorActionPreference = "Stop"

function Parse-UsFromLine([string]$line) {
    # Example:
    # time:   [178.85 µs 181.94 µs 185.68 µs]
    if ($line -notmatch "time:\s+\[\s*([0-9]+(?:\.[0-9]+)?)\s*(ns|us|µs|ms)\s+([0-9]+(?:\.[0-9]+)?)\s*(?:ns|us|µs|ms)\s+([0-9]+(?:\.[0-9]+)?)\s*(?:ns|us|µs|ms)\s*\]") {
        return $null
    }
    $mid = [double]$Matches[3]
    $unit = $Matches[2]
    switch ($unit) {
        "ns" { return $mid / 1000.0 }
        "us" { return $mid }
        "µs" { return $mid }
        "ms" { return $mid * 1000.0 }
        default { return $null }
    }
}

Push-Location $RepoRoot
try {
    $benchArgs = @("bench", "-p", "envr-shim-core", "--bench", "shim_settings_snapshot", "--")
    $benchArgs += $AdditionalBenchArgs

    Write-Host "Running: cargo $($benchArgs -join ' ')"
    $prevErrAction = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $output = & cargo @benchArgs 2>&1
    $ErrorActionPreference = $prevErrAction
    if ($LASTEXITCODE -ne 0) {
        $output | Write-Host
        throw "Benchmark command failed with exit code $LASTEXITCODE"
    }

    $legacySeen = $false
    $singleSeen = $false
    $legacyUs = $null
    $singleUs = $null

    foreach ($line in $output) {
        if ($line -match "shim_settings_hot_path/legacy_multi_load_cold_process$") {
            $legacySeen = $true
            $singleSeen = $false
            continue
        }
        if ($line -match "shim_settings_hot_path/single_snapshot_cold_process$") {
            $singleSeen = $true
            $legacySeen = $false
            continue
        }
        $parsed = Parse-UsFromLine $line
        if ($null -eq $parsed) {
            continue
        }
        if ($legacySeen -and $null -eq $legacyUs) {
            $legacyUs = $parsed
            $legacySeen = $false
            continue
        }
        if ($singleSeen -and $null -eq $singleUs) {
            $singleUs = $parsed
            $singleSeen = $false
            continue
        }
    }

    if ($null -eq $legacyUs -or $null -eq $singleUs) {
        throw "Failed to parse benchmark output. Re-run and inspect raw output."
    }

    $savedUs = $legacyUs - $singleUs
    $savedPct = if ($legacyUs -gt 0) { ($savedUs / $legacyUs) * 100.0 } else { 0.0 }
    $speedup = if ($singleUs -gt 0) { $legacyUs / $singleUs } else { [double]::PositiveInfinity }

    Write-Host ""
    Write-Host "Shim settings hot-path benchmark (median estimate)"
    Write-Host ("- legacy_multi_load_cold_process : {0:N3} us" -f $legacyUs)
    Write-Host ("- single_snapshot_cold_process   : {0:N3} us" -f $singleUs)
    Write-Host ("- saved                          : {0:N3} us ({1:N2}%)" -f $savedUs, $savedPct)
    Write-Host ("- speedup                        : {0:N2}x" -f $speedup)
}
finally {
    Pop-Location
}
