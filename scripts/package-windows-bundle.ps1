# Build both MSI and setup.exe in one command.
# Usage (from repo root):
#   .\scripts\package-windows-bundle.ps1 -Version 0.1.0 -OutRoot dist
param(
    [string]$Version = "0.1.0",
    [string]$OutRoot = "dist",
    [string]$Manufacturer = "envr",
    [string]$VcRedistUrl = "https://aka.ms/vs/17/release/vc_redist.x64.exe",
    [string]$VcRedistPath = ""
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "Step 1/2: Building MSI..."
& (Join-Path $scriptRoot "package-windows-msi.ps1") `
    -Version $Version `
    -OutRoot $OutRoot `
    -Manufacturer $Manufacturer

Write-Host "Step 2/2: Building setup.exe..."
& (Join-Path $scriptRoot "package-windows-setup.ps1") `
    -Version $Version `
    -OutRoot $OutRoot `
    -Manufacturer $Manufacturer `
    -VcRedistUrl $VcRedistUrl `
    -VcRedistPath $VcRedistPath

Write-Host "All done."
Write-Host "  MSI:   $(Join-Path $OutRoot "envr-windows-x64-$Version.msi")"
Write-Host "  Setup: $(Join-Path $OutRoot "envr-setup-x64-$Version.exe")"
