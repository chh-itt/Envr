# envr: directory hook for PowerShell — install by adding the snippet returned from `envr hook powershell` to your PowerShell profile.
# It watches the current location, applies `.envr.toml`-based environment changes on directory transitions, and restores prior values when leaving.
# Tip: run `envr hook status` / `envr hook doctor` for diagnostics when the profile snippet is installed but activation does not happen.

function global:Test-EnvrHookRoot {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path)) { return $null }
    $current = Resolve-Path -LiteralPath $Path -ErrorAction SilentlyContinue
    if (-not $current) { return $null }
    $dir = $current.Path
    while ($dir) {
        if (Test-Path -LiteralPath (Join-Path $dir '.envr.toml') -PathType Leaf -ErrorAction SilentlyContinue) { return $dir }
        if (Test-Path -LiteralPath (Join-Path $dir '.envr.local.toml') -PathType Leaf -ErrorAction SilentlyContinue) { return $dir }
        $parent = Split-Path -Parent $dir
        if ([string]::IsNullOrWhiteSpace($parent) -or $parent -eq $dir) { break }
        $dir = $parent
    }
    return $null
}

function global:Get-EnvrHookState {
    if (-not $script:EnvrHookState) {
        $script:EnvrHookState = [ordered]@{
            Root = $null
            Vars = @{}
            Had = @{}
        }
    }
    return $script:EnvrHookState
}

function global:Save-EnvrHookEnv {
    param([string[]]$Keys)
    $state = Get-EnvrHookState
    $state.Vars = @{}
    $state.Had = @{}
    foreach ($k in $Keys) {
        if ([string]::IsNullOrWhiteSpace($k)) { continue }
        $had = [System.Environment]::GetEnvironmentVariable($k, 'Process') -ne $null
        $state.Had[$k] = $had
        if ($had) {
            $state.Vars[$k] = [System.Environment]::GetEnvironmentVariable($k, 'Process')
        }
    }
}

function global:Restore-EnvrHookEnv {
    $state = Get-EnvrHookState
    foreach ($k in @($state.Vars.Keys)) {
        if ($state.Had.ContainsKey($k) -and $state.Had[$k]) {
            [System.Environment]::SetEnvironmentVariable($k, $state.Vars[$k], 'Process')
        } else {
            [System.Environment]::SetEnvironmentVariable($k, $null, 'Process')
        }
    }
    $state.Root = $null
    $state.Vars = @{}
    $state.Had = @{}
}

function global:Invoke-EnvrHookRefresh {
    $state = Get-EnvrHookState
    $nextRoot = Test-EnvrHookRoot (Get-Location).Path
    if ($state.Root -eq $nextRoot) { return }
    if ($state.Root) { Restore-EnvrHookEnv }
    if (-not $nextRoot) { return }
    $keys = @(envr hook keys --path $nextRoot 2>$null)
    Save-EnvrHookEnv -Keys $keys
    $envScript = @(envr env --shell powershell --path $nextRoot 2>$null) -join "`n"
    if ([string]::IsNullOrWhiteSpace($envScript)) {
        Restore-EnvrHookEnv
        return
    }
    $state.Root = $nextRoot
    Invoke-Expression $envScript
}

Set-Alias -Scope Global envr command envr
if (-not (Get-Command -Name global:Prompt-EnvrHook -ErrorAction SilentlyContinue)) {
    function global:Prompt-EnvrHook {
        Invoke-EnvrHookRefresh
        if (Get-Command -Name prompt -ErrorAction SilentlyContinue) {
            Microsoft.PowerShell.Core\Invoke-Expression (prompt)
        }
    }
}

if (-not $global:__envr_hook_powershell_initialized) {
    $global:__envr_hook_powershell_initialized = $true
    Invoke-EnvrHookRefresh
}
