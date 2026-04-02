# Fetches Lucide SVGs into crates/envr-gui/assets/icons/lucide/
# Requires network. License: MIT (https://github.com/lucide-icons/lucide)

$ErrorActionPreference = "Stop"
$ver = "0.469.0"
$dest = Join-Path $PSScriptRoot "..\crates\envr-gui\assets\icons\lucide"
$base = "https://raw.githubusercontent.com/lucide-icons/lucide/$ver/icons"
New-Item -ItemType Directory -Force -Path $dest | Out-Null

$icons = @(
    "layout-dashboard","refresh-cw","settings","download","chevrons-up-down",
    "eye-off","circle-alert","package","panel-left-open","x","menu","info"
)
foreach ($name in $icons) {
    $uri = "$base/$name.svg"
    $out = Join-Path $dest "$name.svg"
    Write-Host "GET $uri"
    Invoke-WebRequest -Uri $uri -OutFile $out -UseBasicParsing
}
Set-Content (Join-Path $dest "VERSION.txt") "lucide-icons $ver (MIT) — fetched from $base"
