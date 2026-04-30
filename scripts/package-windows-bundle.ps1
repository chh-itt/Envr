# Build both MSI and setup.exe in one command.
# Usage (from repo root):
#   .\scripts\package-windows-bundle.ps1 -Version 0.1.0 -OutRoot dist -Arch x64|arm64
param(
    [string]$Version = "0.1.0",
    [string]$OutRoot = "dist",
    [ValidateSet("x64", "arm64")]
    [string]$Arch = "x64",
    [string]$Manufacturer = "envr",
    [string]$VcRedistUrl = "https://aka.ms/vs/17/release/vc_redist.x64.exe",
    [string]$VcRedistPath = ""
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

if ($Arch -eq "arm64") {
    $VcRedistUrl = "https://aka.ms/vs/17/release/vc_redist.arm64.exe"
    if (-not $VcRedistPath) {
        $VcRedistPath = ""
    }
}

$target = if ($Arch -eq "arm64") { "aarch64-pc-windows-msvc" } else { "" }

Write-Host "Step 1/2: Building MSI..."
& (Join-Path $scriptRoot "package-windows-msi.ps1") `
    -Version $Version `
    -OutRoot $OutRoot `
    -Arch $Arch `
    -Manufacturer $Manufacturer `
    -Target $target

Write-Host "Step 2/2: Building setup.exe..."
& (Join-Path $scriptRoot "package-windows-setup.ps1") `
    -Version $Version `
    -OutRoot $OutRoot `
    -Arch $Arch `
    -Manufacturer $Manufacturer `
    -VcRedistUrl $VcRedistUrl `
    -VcRedistPath $VcRedistPath

Write-Host "All done."
Write-Host "  MSI:   $(Join-Path $OutRoot "envr-windows-$Arch-$Version.msi")"
Write-Host "  Setup: $(Join-Path $OutRoot "envr-setup-$Arch-$Version.exe")"
