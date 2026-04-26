Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path ".").Path
Import-Module (Join-Path $repoRoot "smoke/pwsh/lib/SmokeLib.psm1") -Force -DisableNameChecking

$state = Initialize-SmokeState -RepoRoot $repoRoot
$bins = Resolve-EnvrExe -RepoRoot $repoRoot
$envIso = Get-IsolatedEnvrEnv -RepoRoot $repoRoot

$bootstrap = @{
  ts = (Get-Date).ToString("o")
  kind = "bootstrap"
  repoRoot = $repoRoot
  envrExe = $bins.EnvrExe
  erExe = $bins.ErExe
  isolated = $envIso
}

if (-not (Test-Path -LiteralPath $state.ReportJson)) {
  New-Item -ItemType File -Force -Path $state.ReportJson | Out-Null
}
Append-JsonLine -Path $state.ReportJson -Object $bootstrap

"envr: $($bins.EnvrExe)"
"er:   $($bins.ErExe)"
"ENVR_ROOT=$($envIso.ENVR_ROOT)"
"ENVR_RUNTIME_ROOT=$($envIso.ENVR_RUNTIME_ROOT)"
"state: $($state.StateDir)"

