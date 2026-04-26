param(
  [switch] $UninstallAfter,
  [int] $StepTimeoutSec = 900
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$script:LastRemoteFailureReason = $null

$repoRoot = (Resolve-Path ".").Path
Import-Module (Join-Path $repoRoot "smoke/pwsh/lib/SmokeLib.psm1") -Force -DisableNameChecking

$state = Initialize-SmokeState -RepoRoot $repoRoot
$bins = Resolve-EnvrExe -RepoRoot $repoRoot
$envIso = Get-IsolatedEnvrEnv -RepoRoot $repoRoot

function Get-RuntimeStepTimeoutSec {
  param(
    [Parameter(Mandatory=$false)]
    [string] $RuntimeKey = "",
    [Parameter(Mandatory=$false)]
    [string] $StepKind = ""
  )
  $timeout = $StepTimeoutSec
  if ($RuntimeKey -eq "racket" -and ($StepKind -eq "remote" -or $StepKind -like "install_*")) {
    # Racket mirrors can be extremely slow in constrained networks.
    $timeout = [Math]::Max($timeout, 21600)
  }
  return $timeout
}

if (-not (Test-Path -LiteralPath $state.ReportJson)) {
  New-Item -ItemType File -Force -Path $state.ReportJson | Out-Null
}

function Get-VersionSortKey {
  param(
    [Parameter(Mandatory=$true)]
    [string] $Version
  )
  $matches = [regex]::Matches($Version, "\d+")
  if ($matches.Count -eq 0) {
    return "0|" + $Version
  }
  $parts = @()
  foreach ($m in $matches) {
    $parts += ([int]$m.Value).ToString("D6")
  }
  return "1|" + ($parts -join ".")
}

function Select-BestVersionLabel {
  param(
    [Parameter(Mandatory=$true)]
    $Versions
  )
  $labels = @()
  foreach ($v in $Versions) {
    if ($v -and $v.version) { $labels += [string]$v.version }
  }
  if ($labels.Count -eq 0) { return $null }
  $best = $labels | Sort-Object @{ Expression = { Get-VersionSortKey -Version $_ }; Descending = $true }, @{ Expression = { $_ }; Descending = $true } | Select-Object -First 1
  return [string]$best
}

function Get-RemoteFirstVersionLabel {
  param(
    [Parameter(Mandatory=$true)]
    [string] $RuntimeKey
  )
  $timeout = Get-RuntimeStepTimeoutSec -RuntimeKey $RuntimeKey -StepKind "remote"
  $log = New-LogFilePath -LogsDir $state.LogsDir -Name "remote_$RuntimeKey"
  $r = Invoke-SmokeCommand -Exe $bins.EnvrExe -Args @("--format","json","remote",$RuntimeKey,"-u") -Env $envIso -Cwd $repoRoot -TimeoutSec $timeout -LogPath $log
  $script:LastRemoteFailureReason = $null
  Append-JsonLine -Path $state.ReportJson -Object @{
    ts = (Get-Date).ToString("o")
    kind = "runtime_remote"
    runtime = $RuntimeKey
    exitCode = $r.ExitCode
    log = $r.LogPath
  }
  if ($r.TimedOut) {
    $script:LastRemoteFailureReason = "remote timed out (${timeout}s)"
    Append-JsonLine -Path $state.ReportJson -Object @{
      ts = (Get-Date).ToString("o")
      kind = "runtime_skip"
      runtime = $RuntimeKey
      reason = "remote_timed_out"
      timeoutSec = $timeout
      log = $r.LogPath
    }
    return $null
  }
  if ($r.ExitCode -ne 0) {
    try {
      $obj = $r.Stdout | ConvertFrom-Json
      $msg = [string]$obj.message
      if ([string]::IsNullOrWhiteSpace($msg) -and $obj.data -and $obj.data.error -and $obj.data.error.message) {
        $msg = [string]$obj.data.error.message
      }
      if (-not [string]::IsNullOrWhiteSpace($msg)) {
        $script:LastRemoteFailureReason = $msg
      } else {
        $script:LastRemoteFailureReason = "remote command failed (exit=$($r.ExitCode))"
      }
    } catch {
      $script:LastRemoteFailureReason = "remote command failed (exit=$($r.ExitCode))"
    }
    return $null
  }

  try {
    $obj = $r.Stdout | ConvertFrom-Json
    $rows = $obj.data.remote_runtimes
    if (-not $rows -or $rows.Count -eq 0) {
      $script:LastRemoteFailureReason = "remote list is empty"
      return $null
    }
    $row = $rows | Where-Object { $_.kind -eq $RuntimeKey } | Select-Object -First 1
    if (-not $row) { $row = $rows | Select-Object -First 1 }
    if (-not $row.versions -or $row.versions.Count -eq 0) {
      $script:LastRemoteFailureReason = "remote versions are empty for runtime"
      return $null
    }
    return Select-BestVersionLabel -Versions $row.versions
  } catch {
    $script:LastRemoteFailureReason = "failed to parse remote JSON output"
    return $null
  }
}

function Invoke-EnvrStep {
  param(
    [Parameter(Mandatory=$true)]
    [string] $Name,
    [Parameter(Mandatory=$false)]
    [string] $RuntimeKey = "",
    [Parameter(Mandatory=$true)]
    [string[]] $Args
  )
  $timeout = Get-RuntimeStepTimeoutSec -RuntimeKey $RuntimeKey -StepKind $Name
  $log = New-LogFilePath -LogsDir $state.LogsDir -Name $Name
  $r = Invoke-SmokeCommand -Exe $bins.EnvrExe -Args $Args -Env $envIso -Cwd $repoRoot -TimeoutSec $timeout -LogPath $log
  Append-JsonLine -Path $state.ReportJson -Object @{
    ts = (Get-Date).ToString("o")
    kind = "runtime_step"
    name = $Name
    args = $Args
    exitCode = $r.ExitCode
    timedOut = $r.TimedOut
    timeoutSec = $timeout
    log = $r.LogPath
  }
  if ($r.TimedOut) {
    Write-Host "  step timeout (${timeout}s): $Name"
  }
  return $r
}

function Set-PythonSmokeInstallPolicy {
  Write-Host "  apply python smoke policy: official sources + nuget distribution"
  $cfg1 = Invoke-EnvrStep -Name "config_python_download_source_official" -RuntimeKey "python" -Args @("config","set","runtime.python.download_source","official")
  $cfg2 = Invoke-EnvrStep -Name "config_python_pip_registry_mode_official" -RuntimeKey "python" -Args @("config","set","runtime.python.pip_registry_mode","official")
  $cfg3 = Invoke-EnvrStep -Name "config_python_windows_distribution_nuget" -RuntimeKey "python" -Args @("config","set","runtime.python.windows_distribution","nuget")
  if ($cfg1.ExitCode -ne 0 -or $cfg2.ExitCode -ne 0 -or $cfg3.ExitCode -ne 0) {
    Write-Host "  warning: failed to apply full python smoke policy; continuing with current settings"
  }
}

function Get-InstalledVersionSet {
  param(
    [Parameter(Mandatory=$true)]
    [string] $RuntimeKey
  )
  $set = @{}
  $timeout = Get-RuntimeStepTimeoutSec -RuntimeKey $RuntimeKey -StepKind "list"
  $log = New-LogFilePath -LogsDir $state.LogsDir -Name "list_$RuntimeKey"
  $r = Invoke-SmokeCommand -Exe $bins.EnvrExe -Args @("--format","json","list",$RuntimeKey) -Env $envIso -Cwd $repoRoot -TimeoutSec $timeout -LogPath $log
  Append-JsonLine -Path $state.ReportJson -Object @{
    ts = (Get-Date).ToString("o")
    kind = "runtime_list"
    runtime = $RuntimeKey
    exitCode = $r.ExitCode
    log = $r.LogPath
  }
  if ($r.TimedOut -or $r.ExitCode -ne 0) { return $set }

  try {
    $obj = $r.Stdout | ConvertFrom-Json
    $rows = $obj.data.installed_runtimes
    if (-not $rows) { return $set }
    $row = $rows | Where-Object { $_.kind -eq $RuntimeKey } | Select-Object -First 1
    if (-not $row -or -not $row.versions) { return $set }
    foreach ($v in $row.versions) {
      if ($v.version) { $set[[string]$v.version] = $true }
    }
  } catch {
    # Best-effort only; return empty set on parse mismatch.
  }
  return $set
}

$probeByRuntime = @{
  node = @{ tool = "node"; args = @("-e","console.log(1+1)") }
  python = @{ tool = "python"; args = @("-c","print(1+1)") }
  java = @{ tool = "java"; args = @("-version") }
  kotlin = @{ tool = "kotlinc"; args = @("-version") }
  scala = @{ tool = "scala"; args = @("-version") }
  clojure = @{ tool = "clojure"; args = @("-Sdescribe") }
  groovy = @{ tool = "groovy"; args = @("--version") }
  terraform = @{ tool = "terraform"; args = @("version") }
  v = @{ tool = "v"; args = @("version") }
  odin = @{ tool = "odin"; args = @("version") }
  purescript = @{ tool = "purs"; args = @("--version") }
  elm = @{ tool = "elm"; args = @("--help") }
  gleam = @{ tool = "gleam"; args = @("--version") }
  racket = @{ tool = "racket"; args = @("-e","(displayln (+ 1 1))") }
  dart = @{ tool = "dart"; args = @("--version") }
  flutter = @{ tool = "flutter"; args = @("--version") }
  go = @{ tool = "go"; args = @("version") }
  ruby = @{ tool = "ruby"; args = @("-e","puts(1+1)") }
  elixir = @{ tool = "elixir"; args = @("-e","IO.puts(1+1)") }
  erlang = @{ tool = "erl"; args = @("-noshell","-eval",'io:format("~p~n",[2]),init:stop().') }
  php = @{ tool = "php"; args = @("-r","echo 1+1;") }
  deno = @{ tool = "deno"; args = @("eval","console.log(1+1)") }
  bun = @{ tool = "bun"; args = @("-e","console.log(1+1)") }
  dotnet = @{ tool = "dotnet"; args = @("--version") }
  zig = @{ tool = "zig"; args = @("version") }
  julia = @{ tool = "julia"; args = @("-e","println(1+1)") }
  janet = @{ tool = "janet"; args = @("-e","(print (+ 1 1))") }
  c3 = @{ tool = "c3c"; args = @("--version") }
  babashka = @{ tool = "bb"; args = @("-e","(println (+ 1 1))") }
  sbcl = @{ tool = "sbcl"; args = @("--non-interactive","--eval",'(format t "~a" (+ 1 1))') }
  haxe = @{ tool = "haxe"; args = @("-version") }
  lua = @{ tool = "lua"; args = @("-e","print(1+1)") }
  nim = @{ tool = "nim"; args = @("--version") }
  crystal = @{ tool = "crystal"; args = @("eval","puts 2") }
  perl = @{ tool = "perl"; args = @("-e","print 1+1") }
  unison = @{ tool = "ucm"; args = @("version") }
  r = @{ tool = "Rscript"; args = @("-e","cat(1+1)") }
}

# Order matters for host-runtime dependencies (Java → Kotlin/Scala/Clojure/Groovy; Erlang → Gleam).
$runtimes = @(
  "node","python",
  "java","kotlin","scala","clojure","groovy",
  "erlang","gleam",
  "terraform","v","odin","purescript","elm",
  "racket","dart","flutter",
  "go","ruby","elixir",
  "php","deno","bun","dotnet",
  "zig","julia","janet","c3","babashka","sbcl","haxe","lua","nim","crystal","perl","unison",
  "r"
)

foreach ($rt in $runtimes) {
  Write-Host "== runtime: $rt =="
  if ($rt -eq "python") {
    Set-PythonSmokeInstallPolicy
  }
  $label = Get-RemoteFirstVersionLabel -RuntimeKey $rt
  if (-not $label) {
    $reasonText = if ([string]::IsNullOrWhiteSpace($script:LastRemoteFailureReason)) { "remote_empty_or_failed" } else { $script:LastRemoteFailureReason }
    Write-Host "  skip: $reasonText"
    Append-JsonLine -Path $state.ReportJson -Object @{
      ts = (Get-Date).ToString("o")
      kind = "runtime_skip"
      runtime = $rt
      reason = $reasonText
    }
    continue
  }

  $installed = Get-InstalledVersionSet -RuntimeKey $rt
  if ($installed.ContainsKey($label)) {
    Write-Host "  skip install: $rt $label already installed"
    Append-JsonLine -Path $state.ReportJson -Object @{
      ts = (Get-Date).ToString("o")
      kind = "runtime_install_skip"
      runtime = $rt
      version = $label
      reason = "already_installed"
    }
  } else {
    Write-Host "  install: $rt $label"
    $install = Invoke-EnvrStep -Name "install_$rt" -RuntimeKey $rt -Args @("install",$rt,$label)
    if ($install.ExitCode -ne 0) { continue }
  }

  Write-Host "  use: $rt $label"
  $use = Invoke-EnvrStep -Name "use_$rt" -RuntimeKey $rt -Args @("use",$rt,$label)
  if ($use.ExitCode -ne 0) { continue }

  if ($probeByRuntime.ContainsKey($rt)) {
    $probe = $probeByRuntime[$rt]
    $tool = $probe.tool
    $pargs = @("--format","json","exec","--lang",$rt,"--",$tool) + $probe.args
    Write-Host "  exec probe: $tool"
    $exec = Invoke-EnvrStep -Name "exec_$rt" -RuntimeKey $rt -Args $pargs
    # Do not hard-fail the whole loop; record and proceed to uninstall.
  }

  if ($UninstallAfter) {
    $un = Invoke-EnvrStep -Name "uninstall_$rt" -RuntimeKey $rt -Args @("uninstall",$rt,$label,"--yes","--force")
  }
}

# Rust is special: do not force managed install here (network-heavy). Just record rustc if available.
$rustLog = New-LogFilePath -LogsDir $state.LogsDir -Name "exec_rust_rustc"
$rustRes = Invoke-SmokeCommand -Exe $bins.EnvrExe -Args @("--format","json","exec","--lang","rust","--","rustc","-V") -Env $envIso -Cwd $repoRoot -TimeoutSec $StepTimeoutSec -LogPath $rustLog
Append-JsonLine -Path $state.ReportJson -Object @{
  ts = (Get-Date).ToString("o")
  kind = "runtime_step"
  name = "exec_rust_rustc"
  exitCode = $rustRes.ExitCode
  log = $rustRes.LogPath
}

if ($UninstallAfter) {
  "OK: runtime lifecycle smoke finished with uninstall cleanup (see report.json)."
} else {
  "OK: runtime lifecycle smoke finished (installed/current kept for fixture run)."
}

