# Build release binaries and produce a versioned zip + SHA256 checksums (Windows).
# Usage (from repo root): .\scripts\package-windows-release.ps1 [-Version 0.1.0] [-OutRoot dist] [-Arch x86_64|arm64] [-Target <rust-target>]
param(
    [string]$Version = "0.1.0",
    [string]$OutRoot = "dist",
    [ValidateSet("x86_64", "arm64")]
    [string]$Arch = "x86_64",
    [string]$Target = ""
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$destName = "envr-windows-$Arch-$Version"
$dest = Join-Path $OutRoot $destName
New-Item -ItemType Directory -Force -Path $dest | Out-Null

Write-Host "Building release (envr, er, envr-gui, envr-shim)..."
if ($Target) {
    if ($Target -eq "aarch64-pc-windows-msvc") {
        $env:CC_aarch64_pc_windows_msvc = "clang-cl"
        $env:CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_LINKER = "rust-lld"
    }
    cargo build --release --target $Target -p envr-cli -p envr-gui -p envr-shim
} else {
    cargo build --release -p envr-cli -p envr-gui -p envr-shim
}

$bin = if ($Target) { Join-Path $root "target\$Target\release" } else { Join-Path $root "target\release" }
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
