#Requires -Version 7
<#
.SYNOPSIS
  Measure envr CLI smoke commands and compare to scripts/perf/baseline.json (T903).

.PARAMETER EnvrPath
  Path to envr executable. Default: "envr" (must be on PATH).

.PARAMETER RepoRoot
  Repository root (for baseline.json). Default: two levels above this script.
#>
param(
    [string]$EnvrPath = "envr",
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
)

$ErrorActionPreference = "Stop"
$baselinePath = Join-Path $RepoRoot "scripts/perf/baseline.json"
if (-not (Test-Path $baselinePath)) {
    Write-Error "Missing baseline: $baselinePath"
}

$baseline = Get-Content $baselinePath -Raw | ConvertFrom-Json
$runs = 5
$dropFirst = 1

function MedianMs([double[]]$values) {
    $sorted = $values | Sort-Object
    $mid = [math]::Floor($sorted.Count / 2)
    if (($sorted.Count % 2) -eq 1) { return $sorted[$mid] }
    return ($sorted[$mid - 1] + $sorted[$mid]) / 2.0
}

function Measure-CommandMedian([string[]]$ArgumentList) {
    $times = @()
    for ($i = 0; $i -lt ($runs + $dropFirst); $i++) {
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        & $EnvrPath @ArgumentList 1>$null 2>$null
        $sw.Stop()
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Command failed: $EnvrPath $($ArgumentList -join ' ') (exit $LASTEXITCODE)"
        }
        if ($i -ge $dropFirst) {
            $times += $sw.Elapsed.TotalMilliseconds
        }
    }
    return [double](MedianMs $times)
}

$helpMs = Measure-CommandMedian @("--help")
$doctorMs = Measure-CommandMedian @("doctor", "--format", "json", "--quiet")

Write-Host "median envr --help:        ${helpMs} ms (max $($baseline.help_ms_max))"
Write-Host "median envr doctor (json): ${doctorMs} ms (max $($baseline.doctor_json_ms_max))"

$failed = $false
if ($helpMs -gt [double]$baseline.help_ms_max) {
    Write-Warning "HELP regression: ${helpMs} ms > $($baseline.help_ms_max) ms"
    $failed = $true
}
if ($doctorMs -gt [double]$baseline.doctor_json_ms_max) {
    Write-Warning "DOCTOR regression: ${doctorMs} ms > $($baseline.doctor_json_ms_max) ms"
    $failed = $true
}

if ($failed) { exit 1 }
exit 0
