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

function Get-HelpPathsFromRegistry {
  param(
    [Parameter(Mandatory=$true)]
    [string] $FilePath
  )
  $content = Get-Content -LiteralPath $FilePath -Raw
  # Extract: path: &["foo", "bar", ...]
  $re = [regex]'path:\s*&\[(?<items>[^\]]+)\]'
  $paths = New-Object System.Collections.Generic.List[string]
  foreach ($m in $re.Matches($content)) {
    $items = $m.Groups["items"].Value
    $parts = @()
    foreach ($s in ($items -split ",")) {
      $t = $s.Trim()
      if ($t -match '^"(?<v>[^"]+)"$') { $parts += $Matches["v"] }
    }
    if ($parts.Count -gt 0) {
      $paths.Add(($parts -join " "))
    }
  }
  return ($paths | Sort-Object -Unique)
}

$helpRegistry = Join-Path $repoRoot "crates/envr-cli/src/cli/help_registry/table.inc"
$paths = Get-HelpPathsFromRegistry -FilePath $helpRegistry

# Always include root-level help calls.
$baseCalls = @(
  @{ name = "envr_help_root"; exe = $bins.EnvrExe; args = @("--help") },
  @{ name = "envr_help_shortcuts"; exe = $bins.EnvrExe; args = @("help","shortcuts") },
  @{ name = "er_help_root"; exe = $bins.ErExe; args = @("--help") }
)

foreach ($c in $baseCalls) {
  $log = New-LogFilePath -LogsDir $state.LogsDir -Name $c.name
  $r = Invoke-SmokeCommand -Exe $c.exe -Args $c.args -Env $envIso -Cwd $repoRoot -LogPath $log
  Append-JsonLine -Path $state.ReportJson -Object @{
    ts = (Get-Date).ToString("o")
    kind = "command_help"
    name = $c.name
    exe = $c.exe
    args = $c.args
    exitCode = $r.ExitCode
    log = $r.LogPath
  }
  Assert-Ok -Result $r -StepName $c.name
}

# For each registered help path, run `envr <path...> --help`.
foreach ($p in $paths) {
  $tokens = @()
  if (-not [string]::IsNullOrWhiteSpace($p)) {
    $tokens = $p.Split(" ", [System.StringSplitOptions]::RemoveEmptyEntries)
  }
  if ($tokens.Count -eq 0) { continue }

  $name = "envr_help_" + ($tokens -join "_")
  $args = @($tokens + @("--help"))
  $log = New-LogFilePath -LogsDir $state.LogsDir -Name $name
  $r = Invoke-SmokeCommand -Exe $bins.EnvrExe -Args $args -Env $envIso -Cwd $repoRoot -LogPath $log
  Append-JsonLine -Path $state.ReportJson -Object @{
    ts = (Get-Date).ToString("o")
    kind = "command_help"
    name = $name
    exe = $bins.EnvrExe
    args = $args
    exitCode = $r.ExitCode
    log = $r.LogPath
    path = $p
  }
  Assert-Ok -Result $r -StepName $name
}

"OK: command help coverage complete. Paths=$($paths.Count)"

