Set-StrictMode -Version Latest

function Get-SmokeRoot {
  param(
    [Parameter(Mandatory=$false)]
    [string] $RepoRoot
  )
  if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = (Resolve-Path ".").Path
  }
  return (Join-Path $RepoRoot "smoke")
}

function Get-SmokeStateDir {
  param(
    [Parameter(Mandatory=$true)]
    [string] $RepoRoot
  )
  $smokeRoot = Get-SmokeRoot -RepoRoot $RepoRoot
  return (Join-Path $smokeRoot ".state")
}

function Initialize-SmokeState {
  param(
    [Parameter(Mandatory=$true)]
    [string] $RepoRoot
  )
  $state = Get-SmokeStateDir -RepoRoot $RepoRoot
  $logs = Join-Path $state "logs"
  $root = Join-Path $state "root"
  New-Item -ItemType Directory -Force -Path $state, $logs, $root | Out-Null
  return @{
    StateDir = $state
    LogsDir  = $logs
    RootDir  = $root
    ReportJson = (Join-Path $state "report.json")
    SummaryMd  = (Join-Path $state "summary.md")
  }
}

function Resolve-EnvrExe {
  param(
    [Parameter(Mandatory=$true)]
    [string] $RepoRoot
  )

  $preferredBins = @()
  if ($env:ENVR_SMOKE_BIN_DIR -and -not [string]::IsNullOrWhiteSpace($env:ENVR_SMOKE_BIN_DIR)) {
    $preferredBins += $env:ENVR_SMOKE_BIN_DIR
  }
  $preferredBins += (Join-Path $RepoRoot "target/release")
  $preferredBins += (Join-Path $RepoRoot "target-smoke/release")

  $envr = $null
  $er = $null
  foreach ($binDir in $preferredBins) {
    if (-not $envr) {
      $cand = Join-Path $binDir "envr.exe"
      if (Test-Path -LiteralPath $cand) { $envr = (Resolve-Path $cand).Path }
    }
    if (-not $er) {
      $cand = Join-Path $binDir "er.exe"
      if (Test-Path -LiteralPath $cand) { $er = (Resolve-Path $cand).Path }
    }
  }

  if (-not $envr) {
    $cmd = Get-Command envr.exe -ErrorAction SilentlyContinue
    if ($cmd) { $envr = $cmd.Source }
  }
  if (-not $er) {
    $cmd = Get-Command er.exe -ErrorAction SilentlyContinue
    if ($cmd) { $er = $cmd.Source }
  }

  if (-not $envr) { throw "envr.exe not found. Build with 'cargo build --release' or ensure envr.exe is on PATH." }
  if (-not $er) { throw "er.exe not found. Build with 'cargo build --release' or ensure er.exe is on PATH." }

  return @{ EnvrExe = $envr; ErExe = $er }
}

function Get-IsolatedEnvrEnv {
  param(
    [Parameter(Mandatory=$true)]
    [string] $RepoRoot
  )
  $state = Initialize-SmokeState -RepoRoot $RepoRoot
  $root = $state.RootDir
  return @{
    ENVR_ROOT = $root
    ENVR_RUNTIME_ROOT = $root
  }
}

function New-LogFilePath {
  param(
    [Parameter(Mandatory=$true)]
    [string] $LogsDir,
    [Parameter(Mandatory=$true)]
    [string] $Name
  )
  $ts = (Get-Date).ToString("yyyyMMdd_HHmmss")
  $safe = ($Name -replace "[^a-zA-Z0-9._-]+","_")
  return (Join-Path $LogsDir "$ts`_$safe.log")
}

function Invoke-SmokeCommand {
  param(
    [Parameter(Mandatory=$true)]
    [string] $Exe,
    [Parameter(Mandatory=$false)]
    [string[]] $Args = @(),
    [Parameter(Mandatory=$false)]
    [hashtable] $Env = @{},
    [Parameter(Mandatory=$false)]
    [string] $Cwd = $null,
    [Parameter(Mandatory=$false)]
    [int] $TimeoutSec = 0,
    [Parameter(Mandatory=$true)]
    [string] $LogPath
  )

  $psi = New-Object System.Diagnostics.ProcessStartInfo
  $psi.FileName = $Exe
  $psi.Arguments = ($Args | ForEach-Object { if ($_ -match '\s') { '"{0}"' -f $_ } else { $_ } }) -join ' '
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError  = $true
  $psi.UseShellExecute = $false
  if ($Cwd) { $psi.WorkingDirectory = $Cwd }

  foreach ($k in $Env.Keys) {
    $psi.Environment[$k] = [string]$Env[$k]
  }

  $p = New-Object System.Diagnostics.Process
  $p.StartInfo = $psi

  $timedOut = $false
  $started = Get-Date
  $null = $p.Start()
  $stdoutTask = $p.StandardOutput.ReadToEndAsync()
  $stderrTask = $p.StandardError.ReadToEndAsync()
  if ($TimeoutSec -gt 0) {
    $deadline = $started.AddSeconds($TimeoutSec)
    while (-not $p.HasExited -and (Get-Date) -lt $deadline) {
      Start-Sleep -Milliseconds 500
    }
    if (-not $p.HasExited) {
      $timedOut = $true
      try { $p.Kill($true) } catch { try { $p.Kill() } catch {} }
    }
  } else {
    $p.WaitForExit()
  }

  if (-not $p.HasExited) { $p.WaitForExit() }
  $stdout = $stdoutTask.GetAwaiter().GetResult()
  $stderr = $stderrTask.GetAwaiter().GetResult()

  $content = @()
  $content += "exe: $Exe"
  $content += "args: $($psi.Arguments)"
  if ($Cwd) { $content += "cwd: $Cwd" }
  if ($Env.Count -gt 0) {
    $content += "env:"
    foreach ($k in ($Env.Keys | Sort-Object)) { $content += "  $k=$($Env[$k])" }
  }
  $content += "exit_code: $($p.ExitCode)"
  $content += "timed_out: $timedOut"
  if ($TimeoutSec -gt 0) { $content += "timeout_sec: $TimeoutSec" }
  $content += "---- stdout ----"
  $content += $stdout
  $content += "---- stderr ----"
  $content += $stderr
  ($content -join "`r`n") | Set-Content -LiteralPath $LogPath -Encoding UTF8

  return @{
    ExitCode = $p.ExitCode
    TimedOut = $timedOut
    Stdout = $stdout
    Stderr = $stderr
    LogPath = $LogPath
  }
}

function Assert-Ok {
  param(
    [Parameter(Mandatory=$true)]
    [hashtable] $Result,
    [Parameter(Mandatory=$true)]
    [string] $StepName
  )
  if ($Result.ExitCode -ne 0) {
    throw "Step failed: $StepName (exit=$($Result.ExitCode)). See log: $($Result.LogPath)"
  }
}

function Append-JsonLine {
  param(
    [Parameter(Mandatory=$true)]
    [string] $Path,
    [Parameter(Mandatory=$true)]
    [hashtable] $Object
  )
  $json = ($Object | ConvertTo-Json -Depth 10 -Compress)
  Add-Content -LiteralPath $Path -Value $json -Encoding UTF8
}

Export-ModuleMember -Function `
  Get-SmokeRoot, Get-SmokeStateDir, Initialize-SmokeState, Resolve-EnvrExe, Get-IsolatedEnvrEnv, `
  New-LogFilePath, Invoke-SmokeCommand, Assert-Ok, Append-JsonLine

