# envr: directory hook for PowerShell — install with: Invoke-Expression (& envr hook powershell)
# Applies merged project env when you cd into a tree with .envr.toml; restores when you leave.
# Optional prompt: function global:prompt { "$(& envr hook prompt)$(& $function:prompt)" }

$script:_envrHookRoot = $null
$script:_envrHookKeys = @()
$script:_envrHookHad = @{}
$script:_envrHookVal = @{}

function global:_envrHookFindRoot {
    $dir = (Get-Location).Path
    while ($true) {
        if ((Test-Path (Join-Path $dir '.envr.toml')) -or (Test-Path (Join-Path $dir '.envr.local.toml'))) {
            return $dir
        }
        $parent = Split-Path -Parent $dir
        if ([string]::IsNullOrEmpty($parent) -or $parent -eq $dir) {
            return $null
        }
        $dir = $parent
    }
}

function global:_envrHookCapture {
    param([string]$Root)

    $script:_envrHookKeys = @()
    $script:_envrHookHad = @{}
    $script:_envrHookVal = @{}

    $keys = & envr hook keys --path $Root 2>$null
    foreach ($k in $keys) {
        if ([string]::IsNullOrWhiteSpace($k)) { continue }
        $script:_envrHookKeys += $k
        if (Test-Path "Env:$k") {
            $script:_envrHookHad[$k] = $true
            $script:_envrHookVal[$k] = (Get-Item "Env:$k").Value
        } else {
            $script:_envrHookHad[$k] = $false
        }
    }
}

function global:_envrHookRestore {
    foreach ($k in $script:_envrHookKeys) {
        if ($script:_envrHookHad[$k]) {
            Set-Item -Path "Env:$k" -Value $script:_envrHookVal[$k]
        } else {
            Remove-Item -Path "Env:$k" -ErrorAction SilentlyContinue
        }
    }
    $script:_envrHookKeys = @()
    $script:_envrHookHad = @{}
    $script:_envrHookVal = @{}
}

function global:_envrHookMain {
    $nextRoot = _envrHookFindRoot
    if ($nextRoot -eq $script:_envrHookRoot) {
        return
    }

    if ($null -ne $script:_envrHookRoot) {
        _envrHookRestore
    }

    if ([string]::IsNullOrWhiteSpace($nextRoot)) {
        $script:_envrHookRoot = $null
        return
    }

    _envrHookCapture -Root $nextRoot
    if ($env:ENVR_HOOK_VERBOSE) {
        Write-Host "envr hook: applying $nextRoot" -ForegroundColor DarkGray
    }

    $out = & envr env --shell posix --path $nextRoot 2>$null
    if ($LASTEXITCODE -eq 0 -and $out) {
        $script:_envrHookRoot = $nextRoot
        Invoke-Expression $out
    } else {
        _envrHookRestore
        $script:_envrHookRoot = $null
    }
}

function global:envr {
    param([Parameter(ValueFromRemainingArguments = $true)]$Args)
    if ($Args.Count -gt 0 -and ($Args[0] -eq 'deactivate' -or $Args[0] -eq 'off')) {
        _envrHookRestore
        $script:_envrHookRoot = $null
        return
    }
    & envr @Args
}

function global:_envrPromptSegment {
    (& envr hook prompt 2>$null) -join ''
}

if (-not (Test-Path Function:\global:prompt)) {
    function global:prompt { "PS $($executionContext.SessionState.Path.CurrentLocation)> " }
}

if (-not $script:_envrHookInstalled) {
    $script:_envrHookInstalled = $true
    $function:prompt = @"
`$(_envrPromptSegment)$(prompt)
"@
    Register-EngineEvent -SourceIdentifier PowerShell.OnIdle -Action { _envrHookMain } | Out-Null
    _envrHookMain
}
