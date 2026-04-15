[CmdletBinding()]
param(
    [int]$Iterations = 120,
    [int]$Warmup = 20,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

if ($Iterations -le 0) {
    throw "Iterations must be > 0."
}
if ($Warmup -lt 0) {
    throw "Warmup must be >= 0."
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$shimExe = Join-Path $repoRoot "target\release\envr-shim.exe"

if (-not $SkipBuild) {
    Write-Host "Building envr-shim (release)..."
    cargo build -p envr-shim --release
}
if (-not (Test-Path $shimExe)) {
    throw "Missing shim executable: $shimExe"
}

$tempRoot = Join-Path $env:TEMP ("envr-shim-bench-" + [guid]::NewGuid().ToString("N"))
$runtimeRoot = Join-Path $tempRoot "runtime"
$projectDir = Join-Path $tempRoot "project"
$nodeHome = Join-Path $runtimeRoot "runtimes\node\versions\20.0.0"
$nodeCurrent = Join-Path $runtimeRoot "runtimes\node\current"
$cacheDir = Join-Path $runtimeRoot "cache"

try {
    New-Item -ItemType Directory -Force -Path $nodeHome | Out-Null
    New-Item -ItemType Directory -Force -Path $projectDir | Out-Null
    Copy-Item -Force $env:ComSpec (Join-Path $nodeHome "node.exe")
    New-Item -ItemType Directory -Force -Path (Split-Path $nodeCurrent -Parent) | Out-Null
    [System.IO.File]::WriteAllText(
        $nodeCurrent,
        $nodeHome,
        [System.Text.UTF8Encoding]::new($false)
    )

    $deps = 1..400 | ForEach-Object { '"dep-{0}":"^1.0.0"' -f $_ }
    $packageJson = @"
{"name":"bench","version":"1.0.0","engines":{"node":"^20"},"dependencies":{${($deps -join ',')}}}
"@
    [System.IO.File]::WriteAllText(
        (Join-Path $projectDir "package.json"),
        $packageJson,
        [System.Text.UTF8Encoding]::new($false)
    )

    function Invoke-Scenario {
        param([string]$CacheFlag)

        if (Test-Path $cacheDir) {
            Remove-Item -Recurse -Force $cacheDir
        }

        $nodeHintUs = New-Object System.Collections.Generic.List[long]
        $prepareUs = New-Object System.Collections.Generic.List[long]

        Push-Location $projectDir
        try {
            for ($i = 0; $i -lt ($Iterations + $Warmup); $i++) {
                $env:ENVR_RUNTIME_ROOT = $runtimeRoot
                $env:ENVR_SHIM_TRACE_TIMING = "1"
                $env:ENVR_SHIM_NODE_ENGINES_HINT = "1"
                $env:ENVR_SHIM_NODE_ENGINES_HINT_CACHE = $CacheFlag

                $stdoutFile = Join-Path $tempRoot "shim-stdout.txt"
                $stderrFile = Join-Path $tempRoot "shim-stderr.txt"
                Remove-Item -Force $stdoutFile, $stderrFile -ErrorAction SilentlyContinue
                $proc = Start-Process -FilePath $shimExe -ArgumentList @("node", "/c", "exit", "0") -NoNewWindow -PassThru -Wait -WorkingDirectory $projectDir -RedirectStandardOutput $stdoutFile -RedirectStandardError $stderrFile
                $raw = if (Test-Path $stderrFile) { Get-Content -Raw $stderrFile } else { "" }
                if ($proc.ExitCode -ne 0) {
                    throw "shim invocation failed with exit code $($proc.ExitCode)`n$raw"
                }

                $m = [regex]::Match(
                    $raw,
                    "prepare_total_us=(\d+).*node_hint_us=(\d+)",
                    [System.Text.RegularExpressions.RegexOptions]::Singleline
                )
                if (-not $m.Success) {
                    throw "Timing line not found in output:`n$raw"
                }
                if ($i -ge $Warmup) {
                    [void]$prepareUs.Add([long]$m.Groups[1].Value)
                    [void]$nodeHintUs.Add([long]$m.Groups[2].Value)
                }
            }
        } finally {
            Pop-Location
        }

        return [PSCustomObject]@{
            cache_flag = $CacheFlag
            samples = $Iterations
            prepare_avg_us = [math]::Round(($prepareUs | Measure-Object -Average).Average, 2)
            node_hint_avg_us = [math]::Round(($nodeHintUs | Measure-Object -Average).Average, 2)
            prepare_p95_us = ($prepareUs | Sort-Object)[[int]([math]::Floor($prepareUs.Count * 0.95))]
            node_hint_p95_us = ($nodeHintUs | Sort-Object)[[int]([math]::Floor($nodeHintUs.Count * 0.95))]
        }
    }

    Write-Host "Running baseline (cache disabled)..."
    $disabled = Invoke-Scenario -CacheFlag "0"
    Write-Host "Running optimized (cache enabled)..."
    $enabled = Invoke-Scenario -CacheFlag "1"

    $hintDelta = [math]::Round((($disabled.node_hint_avg_us - $enabled.node_hint_avg_us) / [math]::Max($disabled.node_hint_avg_us, 1)) * 100, 2)
    $prepareDelta = [math]::Round((($disabled.prepare_avg_us - $enabled.prepare_avg_us) / [math]::Max($disabled.prepare_avg_us, 1)) * 100, 2)

    Write-Host ""
    Write-Host "=== envr-shim node hint cache comparison ==="
    @($disabled, $enabled) | Format-Table -AutoSize
    Write-Host ""
    Write-Host ("node_hint_avg improvement: {0}%" -f $hintDelta)
    Write-Host ("prepare_avg improvement:   {0}%" -f $prepareDelta)
} finally {
    Remove-Item -Recurse -Force $tempRoot -ErrorAction SilentlyContinue
}
