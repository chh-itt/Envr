#Requires -Version 5.1
<#
.SYNOPSIS
  Measure envr-gui process Private Bytes across GUI scenarios and generate baseline docs (GUI-100).

.PARAMETER GuiExe
  GUI executable to start. Default: "envr-gui.exe" (must be discoverable via PATH or provide absolute path).

.PARAMETER Repeat
  How many times to repeat the full scenario sequence (user still needs to mark boundaries each time).

.PARAMETER SampleIntervalMs
  Private Bytes sampling interval (ms). Default: 1000.

.PARAMETER IncludeChildProcesses
  If set, memory metric becomes process-tree Private Bytes (root + descendants).

.PARAMETER Force
  Overwrite results/<date>-baseline.md (we ship a template by default).
#>
param(
    [string]$GuiExe = "envr-gui.exe",
    [int]$Repeat = 1,
    [int]$SampleIntervalMs = 1000,
    [int]$GuiStartDelaySec = 1,
    [switch]$IncludeChildProcesses = $true,
    [switch]$Force = $true
)

$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$OutputDir = Join-Path $RepoRoot "docs/perf/memory-diagnosis/results"
$OutDate = (Get-Date -Format "yyyy-MM-dd")
$BaselinePath = Join-Path $OutputDir "${OutDate}-baseline.md"
$OutDataJson = Join-Path $OutputDir "${OutDate}-baseline.data.json"

if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

function Median([double[]]$values) {
    $sorted = $values | Sort-Object
    $count = $sorted.Count
    if ($count -eq 0) { return 0 }
    $mid = [math]::Floor($count / 2)
    if (($count % 2) -eq 1) { return $sorted[$mid] }
    return ($sorted[$mid - 1] + $sorted[$mid]) / 2.0
}

function TryGet-PrivateBytes([int]$procId) {
    try {
        $p = Get-Process -Id $procId -ErrorAction Stop
        return [double]$p.PrivateMemorySize64
    } catch {
        return $null
    }
}

function Get-DescendantProcessIds([int]$rootProcId) {
    $all = @(Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId)
    $childrenByParent = @{}
    foreach ($p in $all) {
        $ppid = [int]$p.ParentProcessId
        if (-not $childrenByParent.ContainsKey($ppid)) {
            $childrenByParent[$ppid] = New-Object System.Collections.Generic.List[int]
        }
        $childrenByParent[$ppid].Add([int]$p.ProcessId)
    }

    $seen = New-Object System.Collections.Generic.HashSet[int]
    $queue = New-Object System.Collections.Generic.Queue[int]
    $queue.Enqueue($rootProcId)
    [void]$seen.Add($rootProcId)

    while ($queue.Count -gt 0) {
        $cur = $queue.Dequeue()
        if (-not $childrenByParent.ContainsKey($cur)) {
            continue
        }
        foreach ($child in $childrenByParent[$cur]) {
            if ($seen.Add($child)) {
                $queue.Enqueue($child)
            }
        }
    }
    return @($seen)
}

function TryGet-PrivateBytesProcessTree([int]$rootProcId) {
    try {
        $ids = Get-DescendantProcessIds $rootProcId
        $sum = 0.0
        foreach ($id in $ids) {
            $p = Get-Process -Id $id -ErrorAction SilentlyContinue
            if ($null -ne $p) {
                $sum += [double]$p.PrivateMemorySize64
            }
        }
        return $sum
    } catch {
        return $null
    }
}

function TryGet-PrivateBytesMetric([int]$rootProcId) {
    if ($IncludeChildProcesses) {
        return TryGet-PrivateBytesProcessTree $rootProcId
    }
    return TryGet-PrivateBytes $rootProcId
}

function SamplePhase([int]$procId, [string]$PhaseName, [int]$StartDelaySec, [int]$DurationSec) {
    if ($StartDelaySec -gt 0) {
        Start-Sleep -Seconds $StartDelaySec
    }

    $samples = New-Object System.Collections.Generic.List[object]
    $t0 = Get-Date
    $baseline = $null

    while ($true) {
        $now = Get-Date
        $elapsedSec = ($now - $t0).TotalSeconds
        if ($elapsedSec -ge $DurationSec) { break }

        $pb = TryGet-PrivateBytesMetric $procId
        if ($pb -ne $null) {
            if ($baseline -eq $null) { $baseline = $pb }
            $delta = $pb - $baseline
            $samples.Add([pscustomobject]@{
                t_ms = [int](($now - $t0).TotalMilliseconds)
                private_bytes = $pb
                delta_bytes = $delta
            })
        }

        Start-Sleep -Milliseconds $SampleIntervalMs
    }

    return $samples
}

function BytesToMB([double]$bytes) {
    return $bytes / 1MB
}

function ComputePhaseStats($samples) {
    if ($samples.Count -lt 2) {
        return [pscustomobject]@{
            start_mb = 0
            end_mb = 0
            max_delta_mb = 0
        }
    }
    $start = [double]$samples[0].private_bytes
    $end = [double]$samples[$samples.Count - 1].private_bytes
    $maxDelta = ($samples | ForEach-Object { [double]$_.delta_bytes } | Measure-Object -Maximum).Maximum
    [pscustomobject]@{
        start_mb = BytesToMB $start
        end_mb = BytesToMB $end
        max_delta_mb = BytesToMB $maxDelta
    }
}

$phases = @(
    # Start sampling from phase start; we already capture from process start.
    @{ name = "S1_cold_start"; settle = 0; duration = 60 },
    @{ name = "S2_navigation_toggle"; settle = 0; duration = 60 },
    @{ name = "S3_long_list_scroll"; settle = 0; duration = 90 },
    @{ name = "S4_download_panel"; settle = 0; duration = 180 },
    @{ name = "S5_resource_release"; settle = 0; duration = 60 }
)

if ($Repeat -lt 1) {
    throw "Repeat must be >= 1"
}

$allRuns = New-Object System.Collections.Generic.List[object]

for ($runIdx = 1; $runIdx -le $Repeat; $runIdx++) {
    Write-Host "=== Run $runIdx/$Repeat ===" -ForegroundColor Yellow

    # Start GUI
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $GuiExe
    $psi.WorkingDirectory = $RepoRoot
    $psi.UseShellExecute = $true

    $proc = [System.Diagnostics.Process]::Start($psi)
    if (-not $proc) {
        throw "Failed to start GUI exe: $GuiExe"
    }
    $guiPid = $proc.Id

    # Give the window time to appear, but keep it small for early peak capture.
    Start-Sleep -Seconds $GuiStartDelaySec

    $runResult = [pscustomobject]@{
        run_index = $runIdx
        pid = $guiPid
        phases = @()
    }

    foreach ($phase in $phases) {
        # Ensure process still alive
        $alivePb = TryGet-PrivateBytesMetric $guiPid
        if ($alivePb -eq $null) {
            throw "Process exited unexpectedly during $($phase.name)"
        }

        Write-Host ">>> Phase: $($phase.name)" -ForegroundColor Green

        $samples = SamplePhase -procId $guiPid -PhaseName $phase.name -StartDelaySec $phase.settle -DurationSec $phase.duration
        $stats = ComputePhaseStats $samples

        $runResult.phases += [pscustomobject]@{
            phase = $phase.name
            stats = $stats
            samples_count = $samples.Count
        }
    }

    # Best-effort kill to keep system clean between repeats
    try {
        $p = Get-Process -Id $guiPid -ErrorAction SilentlyContinue
        if ($p -ne $null) { $p.CloseMainWindow() | Out-Null; Start-Sleep -Seconds 3; if (-not $p.HasExited) { $p.Kill() } }
    } catch {}

    $allRuns.Add($runResult) | Out-Null
}

# Save raw data (for audit)
$dataObj = [pscustomobject]@{
    meta = @{
        repo_root = $RepoRoot
        gui_exe = $GuiExe
        out_date = $OutDate
        sample_interval_ms = $SampleIntervalMs
        repeat = $Repeat
        metric_scope = $(if ($IncludeChildProcesses) { "process_tree_private_bytes" } else { "process_private_bytes" })
    }
    runs = $allRuns
}
$dataObj | ConvertTo-Json -Depth 10 | Out-File -Encoding utf8 $OutDataJson

# Aggregate and generate conclusion
$phaseNames = $phases | ForEach-Object { $_.name }

function Avg([double[]]$vals) {
    if ($vals.Count -eq 0) { return 0 }
    $sum = 0.0
    foreach ($v in $vals) { $sum += $v }
    return $sum / $vals.Count
}

$phaseRows = New-Object System.Collections.Generic.List[object]

foreach ($pn in $phaseNames) {
    $sStart = @()
    $sEnd = @()
    $sMaxDelta = @()
    foreach ($r in $allRuns) {
        $ph = $r.phases | Where-Object { $_.phase -eq $pn } | Select-Object -First 1
        if ($ph -ne $null) {
            $sStart += [double]$ph.stats.start_mb
            $sEnd += [double]$ph.stats.end_mb
            $sMaxDelta += [double]$ph.stats.max_delta_mb
        }
    }
    $phaseRows.Add([pscustomobject]@{
        phase = $pn
        avg_start_mb = Avg $sStart
        avg_end_mb = Avg $sEnd
        avg_max_delta_mb = Avg $sMaxDelta
    }) | Out-Null
}

$topPhase = ($phaseRows | Sort-Object -Property avg_max_delta_mb -Descending | Select-Object -First 1).phase

$likely = @()
switch ($topPhase) {
    "S1_cold_start" { $likely += "Fonts/init resources/texture atlases (big growth on first screen)" }
    "S2_navigation_toggle" { $likely += "Page switches rebuild widget tree/resources (big growth when toggling)" }
    "S3_long_list_scroll" { $likely += "List rendering/virtualization boundary issues; fonts or texture atlas growth" }
    "S4_download_panel" { $likely += "Download panel dynamic rendering (progress/icons/textures) creates resources not freed" }
    "S5_resource_release" { $likely += "Resource release/cache lifecycle problem (memory does not drop after leaving page)" }
    default { $likely += "Need to inspect the raw curve for attribution" }
}

function OneLineCause($s) {
    # Remove CR/LF without relying on PowerShell's backtick escape sequences.
    $s2 = $s -replace ([char]13).ToString(), ""
    $s2 = $s2 -replace ([char]10).ToString(), ""
    return $s2
}

$likely1 = $likely[0]

if (Test-Path $BaselinePath) {
    if (-not $Force) {
        $newPath = Join-Path $OutputDir "${OutDate}-baseline.NEW.md"
        Write-Host "Baseline exists; writing to $newPath (use -Force to overwrite expected path)." -ForegroundColor Yellow
        $BaselinePath = $newPath
    }
}

$commitSha = ""
try { $commitSha = (git rev-parse --short HEAD 2>$null) } catch {}

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# GUI-100 WGPU Memory Root-Cause Baseline")
$lines.Add("")
$lines.Add("Date: $OutDate")
$lines.Add("Status: GENERATED (script collects Private Bytes and writes an attribution template)")
$lines.Add("")
$lines.Add("## Environment (fill in)")
$lines.Add("")
$lines.Add("- OS: $([System.Environment]::OSVersion.VersionString)")
$lines.Add("- GPU: TBD (fill in Dedicated/Shared manually from Task Manager)")
$lines.Add("- WGPU_BACKEND: (app may set it to gl)")
$lines.Add("- envr commit: $commitSha")
$lines.Add("- sample interval: $SampleIntervalMs ms")
$lines.Add("")
$lines.Add("## Metric definition")
$lines.Add("")
$lines.Add("- Auto: " + $(if ($IncludeChildProcesses) { "process-tree Private Bytes (root + child processes, Windows)" } else { "process Private Bytes (root process only, Windows)" }))
$lines.Add("- Manual: GPU Dedicated/Shared (Task Manager -> Performance -> GPU)")
$lines.Add("")
$lines.Add("## Scenario results (S1~S5)")
$lines.Add("")
$lines.Add("| Phase | Private Bytes start (MB) | Private Bytes end (MB) | Private Bytes max delta (MB) | GPU Dedicated/Shared (start/end) | Notes |")
$lines.Add("|-------|-----------------------------|--------------------------|---------------------------------|------------------------------------|-------|")

foreach ($row in $phaseRows) {
    # Keep GPU column as TBD for manual fill.
    $start = [double]$row.avg_start_mb
    $end = [double]$row.avg_end_mb
    $maxd = [double]$row.avg_max_delta_mb
    $culture = [System.Globalization.CultureInfo]::InvariantCulture
    $lines.Add(
        "| $($row.phase) | " +
        $start.ToString('F2', $culture) + " | " +
        $end.ToString('F2', $culture) + " | " +
        $maxd.ToString('F2', $culture) + " | TBD | " +
        " |"
    )
}

$lines.Add("")
$lines.Add("## Attribution (based on which phase shows the biggest growth)")
$lines.Add("")
$lines.Add("Top growth phase (by avg max delta): $topPhase")
$lines.Add("")
$lines.Add("1. Most likely cause: " + (OneLineCause $likely1))
$lines.Add("2. Second likely cause: TBD (please use the raw curve + GPU trend to confirm)")
$lines.Add("3. Disproof / evidence (fill at least 1):")
$lines.Add("   - Example: if deltas in phases other than $topPhase are much smaller, treat that hypothesis as falsified/low priority.")
$lines.Add("")
$lines.Add("## Next steps")
$lines.Add("")
$lines.Add("- GUI-101 (mitigation experiments): pick 1-2 small toggles to validate this baseline's most likely cause")
$lines.Add("")

$text = [string]::Join([Environment]::NewLine, $lines)
$text | Out-File -Encoding utf8 $BaselinePath

Write-Host "Generated: $BaselinePath" -ForegroundColor Green
Write-Host "Raw data:  $OutDataJson" -ForegroundColor Green

