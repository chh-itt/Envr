param(
  [string[]] $Only = @(),
  [string] $From = "",
  [int] $StepTimeoutSec = 900
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path ".").Path
Import-Module (Join-Path $repoRoot "smoke/pwsh/lib/SmokeLib.psm1") -Force -DisableNameChecking

$state = Initialize-SmokeState -RepoRoot $repoRoot
$bins = Resolve-EnvrExe -RepoRoot $repoRoot
$envIso = Get-IsolatedEnvrEnv -RepoRoot $repoRoot

if (-not (Test-Path -LiteralPath $state.ReportJson)) {
  New-Item -ItemType File -Force -Path $state.ReportJson | Out-Null
}

$fixturesRoot = Join-Path $repoRoot "smoke/runtime-fixtures"
$fixtureFiles = Get-ChildItem -LiteralPath $fixturesRoot -Recurse -Filter "fixture.json" | Sort-Object FullName

function Write-FixtureFiles {
  param(
    [Parameter(Mandatory=$true)]
    [string] $Dir,
    [Parameter(Mandatory=$true)]
    [hashtable] $WriteFiles
  )
  foreach ($k in $WriteFiles.Keys) {
    $p = Join-Path $Dir $k
    $parent = Split-Path -Parent $p
    if ($parent -and -not (Test-Path -LiteralPath $parent)) {
      New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    [string]$WriteFiles[$k] | Set-Content -LiteralPath $p -Encoding UTF8
  }
}

function Assert-Regex {
  param(
    [Parameter(Mandatory=$true)]
    [string] $Text,
    [Parameter(Mandatory=$true)]
    [string] $Pattern,
    [Parameter(Mandatory=$true)]
    [string] $Label
  )
  if (-not [regex]::IsMatch($Text, $Pattern)) {
    throw "Assertion failed: $Label did not match regex. Pattern=$Pattern"
  }
}

function Invoke-FixtureStep {
  param(
    [Parameter(Mandatory=$true)]
    [string] $Runtime,
    [Parameter(Mandatory=$true)]
    [string[]] $Command,
    [Parameter(Mandatory=$true)]
    [hashtable] $Expect,
    [Parameter(Mandatory=$true)]
    [string] $WorkDir,
    [Parameter(Mandatory=$true)]
    [string] $StepName,
    [Parameter(Mandatory=$true)]
    [int] $TimeoutSec
  )

  $args = @("exec","--lang",$Runtime,"--") + $Command
  $log = New-LogFilePath -LogsDir $state.LogsDir -Name $StepName
  $r = Invoke-SmokeCommand -Exe $bins.EnvrExe -Args $args -Env $envIso -Cwd $WorkDir -TimeoutSec $TimeoutSec -LogPath $log

  Append-JsonLine -Path $state.ReportJson -Object @{
    ts = (Get-Date).ToString("o")
    kind = "fixture_step"
    runtime = $Runtime
    step = $StepName
    command = $Command
    exitCode = $r.ExitCode
    timedOut = $r.TimedOut
    timeoutSec = $TimeoutSec
    log = $r.LogPath
  }

  if ($r.TimedOut) {
    throw "Step timed out after ${TimeoutSec}s. See log: $($r.LogPath)"
  }

  if ($Expect.ContainsKey("exit_code")) {
    $want = [int]$Expect.exit_code
    if ($r.ExitCode -ne $want) {
      throw "Exit code mismatch: want=$want got=$($r.ExitCode). See log: $($r.LogPath)"
    }
  } elseif ($r.ExitCode -ne 0) {
    throw "Non-zero exit code: $($r.ExitCode). See log: $($r.LogPath)"
  }

  if ($Expect.ContainsKey("stdout_regex")) {
    Assert-Regex -Text $r.Stdout -Pattern ([string]$Expect.stdout_regex) -Label "stdout"
  }
  if ($Expect.ContainsKey("stderr_regex")) {
    Assert-Regex -Text $r.Stderr -Pattern ([string]$Expect.stderr_regex) -Label "stderr"
  }

  return $true
}

function Test-RuntimeHasCurrent {
  param(
    [Parameter(Mandatory=$true)]
    [string] $Runtime
  )
  $log = New-LogFilePath -LogsDir $state.LogsDir -Name ("list_{0}_for_fixture" -f $Runtime)
  $r = Invoke-SmokeCommand -Exe $bins.EnvrExe -Args @("--format","json","list",$Runtime) -Env $envIso -Cwd $repoRoot -TimeoutSec $StepTimeoutSec -LogPath $log
  if ($r.TimedOut -or $r.ExitCode -ne 0) {
    return $false
  }
  try {
    $obj = $r.Stdout | ConvertFrom-Json
    $rows = $obj.data.installed_runtimes
    if (-not $rows) { return $false }
    $row = $rows | Where-Object { $_.kind -eq $Runtime } | Select-Object -First 1
    if (-not $row -or -not $row.versions) { return $false }
    foreach ($v in $row.versions) {
      if ($v.current -eq $true) { return $true }
    }
    return $false
  } catch {
    return $false
  }
}

function Get-LogPathFromErrorMessage {
  param(
    [Parameter(Mandatory=$true)]
    [string] $Message
  )
  $m = [regex]::Match($Message, "See log:\s+(.+)$")
  if ($m.Success) { return $m.Groups[1].Value.Trim() }
  return $null
}

$ok = 0
$fail = 0
$summary = @()
$summary += "## Runtime fixtures summary"
$summary += ""
$summary += "- time: $((Get-Date).ToString("o"))"
$selected = @($fixtureFiles)

if ($Only.Count -gt 0) {
  $onlySet = @{}
  foreach ($o in $Only) {
    foreach ($p in ($o -split ",")) {
      $k = $p.Trim().ToLowerInvariant()
      if (-not [string]::IsNullOrWhiteSpace($k)) { $onlySet[$k] = $true }
    }
  }
  $selected = @($selected | Where-Object {
    $runtimeName = (Split-Path -Parent $_.FullName | Split-Path -Leaf).ToLowerInvariant()
    $onlySet.ContainsKey($runtimeName)
  })
}

if (-not [string]::IsNullOrWhiteSpace($From)) {
  $fromKey = $From.Trim().ToLowerInvariant()
  $idx = -1
  for ($i = 0; $i -lt $selected.Count; $i++) {
    $runtimeName = (Split-Path -Parent $selected[$i].FullName | Split-Path -Leaf).ToLowerInvariant()
    if ($runtimeName -eq $fromKey) { $idx = $i; break }
  }
  if ($idx -ge 0) {
    $selected = @($selected[$idx..($selected.Count - 1)])
  }
}

$summary += "- fixtures: $($selected.Count)"
$summary += "- step_timeout_sec: $StepTimeoutSec"
$summary += ""

for ($n = 0; $n -lt $selected.Count; $n++) {
  $ff = $selected[$n]
  $fixtureDirName = Split-Path -Parent $ff.FullName | Split-Path -Leaf
  $fixtureObj = (Get-Content -LiteralPath $ff.FullName -Raw) | ConvertFrom-Json
  $runtime = [string]$fixtureObj.runtime
  if ([string]::IsNullOrWhiteSpace($runtime)) { $runtime = $fixtureDirName }
  Write-Host ("[{0}/{1}] runtime={2} start" -f ($n + 1), $selected.Count, $runtime)

  if (-not (Test-RuntimeHasCurrent -Runtime $runtime)) {
    $summary += "- [SKIP] $runtime — no current version set"
    Write-Host ("[{0}/{1}] runtime={2} SKIP: no current version set" -f ($n + 1), $selected.Count, $runtime)
    continue
  }

  $runDir = Join-Path $state.StateDir ("work/" + $runtime + "/" + (Get-Date).ToString("yyyyMMdd_HHmmss"))
  New-Item -ItemType Directory -Force -Path $runDir | Out-Null

  try {
    if ($fixtureObj.PSObject.Properties.Name -contains "write_files") {
      $wf = @{}
      foreach ($p in $fixtureObj.write_files.PSObject.Properties) { $wf[$p.Name] = [string]$p.Value }
      Write-FixtureFiles -Dir $runDir -WriteFiles $wf
    }

    if ($fixtureObj.PSObject.Properties.Name -contains "steps") {
      $i = 0
      foreach ($s in $fixtureObj.steps) {
        $i++
        $cmd = @()
        foreach ($t in $s.command) { $cmd += [string]$t }
        $exp = @{}
        if ($s.expect) {
          foreach ($p in $s.expect.PSObject.Properties) { $exp[$p.Name] = $p.Value }
        }
        Write-Host ("  - step {0}: {1}" -f $i, ($cmd -join " "))
        Invoke-FixtureStep -Runtime $runtime -Command $cmd -Expect $exp -WorkDir $runDir -StepName ("fixture_{0}_{1}" -f $runtime,$i) -TimeoutSec $StepTimeoutSec | Out-Null
      }
    } else {
      $cmd = @()
      foreach ($t in $fixtureObj.command) { $cmd += [string]$t }
      $exp = @{}
      if ($fixtureObj.expect) {
        foreach ($p in $fixtureObj.expect.PSObject.Properties) { $exp[$p.Name] = $p.Value }
      }
      Write-Host ("  - step 1: {0}" -f ($cmd -join " "))
      Invoke-FixtureStep -Runtime $runtime -Command $cmd -Expect $exp -WorkDir $runDir -StepName ("fixture_{0}" -f $runtime) -TimeoutSec $StepTimeoutSec | Out-Null
    }

    $ok++
    $summary += "- ✅ $runtime"
    Write-Host ("[{0}/{1}] runtime={2} OK" -f ($n + 1), $selected.Count, $runtime)
  } catch {
    $msg = $_.Exception.Message
    $logPath = Get-LogPathFromErrorMessage -Message $msg
    $skipAsKnownEnvIssue = $false
    if ($logPath -and (Test-Path -LiteralPath $logPath)) {
      $raw = Get-Content -LiteralPath $logPath -Raw
      if ($raw -match "'""erl\.exe""' .*不是内部或外部命令" -or
          $raw -match "Unable to find git in your PATH" -or
          $raw -match "Illegal char <\?> at index" -or
          ($runtime -eq "flutter" -and $raw -match "exec command failed to start: program not found")) {
        $skipAsKnownEnvIssue = $true
      }
    }

    if ($skipAsKnownEnvIssue) {
      $summary += "- [SKIP] $runtime — known host/env issue ($msg)"
      Write-Host ("[{0}/{1}] runtime={2} SKIP: known host/env issue" -f ($n + 1), $selected.Count, $runtime)
      continue
    }

    $fail++
    $summary += "- ❌ $runtime — $msg"
    Write-Host ("[{0}/{1}] runtime={2} FAIL: {3}" -f ($n + 1), $selected.Count, $runtime, $msg)
  }
}

$summary += ""
$summary += "- ok: $ok"
$summary += "- fail: $fail"
$summaryText = ($summary -join "`r`n") + "`r`n"
$summaryText | Set-Content -LiteralPath $state.SummaryMd -Encoding UTF8

"OK: fixtures run complete. ok=$ok fail=$fail"

