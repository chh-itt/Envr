# Build release binaries and produce a versioned zip + SHA256 checksums (Windows x86_64).
# Usage (from repo root): .\scripts\package-windows-release.ps1 [-Version 0.1.0] [-OutRoot dist]
param(
    [string]$Version = "0.1.0",
    [string]$OutRoot = "dist"
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$destName = "envr-windows-x86_64-$Version"
$dest = Join-Path $OutRoot $destName
New-Item -ItemType Directory -Force -Path $dest | Out-Null

Write-Host "Building release (envr, er, envr-gui, envr-shim)..."
cargo build --release -p envr-cli -p envr-gui -p envr-shim

$bin = Join-Path $root "target\release"
$exes = @("envr.exe", "er.exe", "envr-gui.exe", "envr-shim.exe")
foreach ($e in $exes) {
    $p = Join-Path $bin $e
    if (-not (Test-Path -LiteralPath $p)) {
        throw "Missing $p — build failed or binary name changed."
    }
    Copy-Item -LiteralPath $p -Destination (Join-Path $dest $e)
}

$sumPath = Join-Path $dest "SHA256SUMS.txt"
Remove-Item -Force -ErrorAction SilentlyContinue $sumPath
foreach ($e in $exes) {
    $fp = Join-Path $dest $e
    $h = Get-FileHash -Algorithm SHA256 -LiteralPath $fp
    "$($h.Hash.ToLowerInvariant())  $e" | Add-Content -Path $sumPath -Encoding utf8
}

$zipName = "$destName.zip"
$zipPath = Join-Path $OutRoot $zipName
if (Test-Path -LiteralPath $zipPath) {
    Remove-Item -Force -LiteralPath $zipPath
}
Compress-Archive -Path $dest -DestinationPath $zipPath -CompressionLevel Optimal

$zipHash = Get-FileHash -Algorithm SHA256 -LiteralPath $zipPath
"$($zipHash.Hash.ToLowerInvariant())  $zipName" | Set-Content -Path (Join-Path $OutRoot "SHA256SUMS-archive.txt") -Encoding utf8

Write-Host "Done."
Write-Host "  Folder: $dest"
Write-Host "  Zip:    $zipPath"
Write-Host "  Verify: Get-FileHash -Algorithm SHA256 $zipPath"
