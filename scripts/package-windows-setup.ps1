# Build setup bootstrapper (setup.exe) via WiX Burn.
# Usage (from repo root):
#   .\scripts\package-windows-setup.ps1 -Version 0.1.0 -OutRoot dist [-Arch x64|arm64]
#   .\scripts\package-windows-setup.ps1 -Version 0.1.0 -InstallScope user -PathScope user
#
# setup.exe is the user-facing bundle layer. It keeps MSI as the execution
# layer, handles VC++ runtime setup, and forwards INSTALLSCOPE/PATHSCOPE.
# Example silent install:
#   .\envr-setup-x64-0.1.0.0.exe /quiet INSTALLSCOPE=user PATHSCOPE=user
param(
    [string]$Version = "0.1.0",
    [string]$OutRoot = "dist",
    [ValidateSet("x64", "arm64")]
    [string]$Arch = "x64",
    [string]$Manufacturer = "envr",
    [string]$VcRedistUrl = "",
    [string]$VcRedistPath = "",
    [ValidateSet("machine", "user")]
    [string]$InstallScope = "machine",
    [ValidateSet("machine", "user", "none")]
    [string]$PathScope = "machine"
)

$ErrorActionPreference = "Stop"

function Normalize-BundleVersion {
    param([string]$InputVersion)

    $clean = $InputVersion.TrimStart('v')
    if ($clean -match '^(\d+)\.(\d+)\.(\d+)(?:[-+].*)?$') {
        $major = [int]$Matches[1]
        $minor = [int]$Matches[2]
        $patch = [int]$Matches[3]
        return "$major.$minor.$patch.0"
    }

    if ($clean -match '^(\d+)\.(\d+)\.(\d+)\.(\d+)$') {
        return $clean
    }

    throw "Version must be Bundle-compatible: Major.Minor.Build[.Revision], got '$InputVersion'."
}

$releaseVersion = $Version
$Version = Normalize-BundleVersion -InputVersion $Version

if (-not $VcRedistUrl) {
    $VcRedistUrl = if ($Arch -eq 'arm64') {
        'https://aka.ms/vs/17/release/vc_redist.arm64.exe'
    } else {
        'https://aka.ms/vs/17/release/vc_redist.x64.exe'
    }
}

$runtimeKeyArch = if ($Arch -eq 'arm64') { 'arm64' } else { 'x64' }
$bundleBuildArch = if ($Arch -eq 'arm64') { 'arm64' } else { 'x64' }

$wixCmd = Get-Command wix -ErrorAction SilentlyContinue
if (-not $wixCmd) {
    throw "WiX v4 CLI not found. Install with: dotnet tool install --global wix"
}

Write-Host "Ensuring WiX extensions (BootstrapperApplications, Util) are available..."
wix extension add -g WixToolset.BootstrapperApplications.wixext -acceptEula wix7
if ($LASTEXITCODE -ne 0) {
    throw "Failed to install WiX extension: WixToolset.BootstrapperApplications.wixext"
}
wix extension add -g WixToolset.Util.wixext -acceptEula wix7
if ($LASTEXITCODE -ne 0) {
    throw "Failed to install WiX extension: WixToolset.Util.wixext"
}
# WiX v7 may print extension identifiers with version/source details that do not
# match the package id verbatim, so do not gate on parsing `wix extension list`.
# The later `wix build -ext ...` invocation is the authoritative validation.

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$msiPath = Join-Path $OutRoot "envr-windows-$Arch-$releaseVersion.msi"
if (-not (Test-Path -LiteralPath $msiPath)) {
    Write-Host "MSI not found. Building MSI first..."
    $env:ENVR_RELEASE_VERSION = $releaseVersion
    & (Join-Path $PSScriptRoot "package-windows-msi.ps1") -Version $Version -OutRoot $OutRoot -Arch $Arch -Manufacturer $Manufacturer -AcceptEula -InstallScope $InstallScope -PathScope $PathScope
}

if (-not (Test-Path -LiteralPath $msiPath)) {
    throw "MSI is still missing after build: $msiPath"
}

$stageName = "envr-windows-setup-$Arch-$releaseVersion"
$stage = Join-Path $OutRoot $stageName
New-Item -ItemType Directory -Force -Path $stage | Out-Null

$vcRedistExe = Join-Path $stage "vc_redist.$Arch.exe"
if ($VcRedistPath -and (Test-Path -LiteralPath $VcRedistPath)) {
    Copy-Item -LiteralPath $VcRedistPath -Destination $vcRedistExe -Force
}
else {
    Write-Host "Downloading VC++ Redistributable from $VcRedistUrl ..."
    Invoke-WebRequest -Uri $VcRedistUrl -OutFile $vcRedistExe
}

if (-not (Test-Path -LiteralPath $vcRedistExe)) {
    throw "vc_redist.$Arch.exe is missing: $vcRedistExe"
}

$bundleWxs = Join-Path $stage "envr-setup.wxs"
$setupExe = Join-Path $OutRoot "envr-windows-$Arch-$releaseVersion-setup.exe"
$bundleUpgradeCode = "2A9B6C0D-B2A0-4E91-B4BF-04EAF172A7A8"

@"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs"
     xmlns:bal="http://wixtoolset.org/schemas/v4/wxs/bal"
     xmlns:util="http://wixtoolset.org/schemas/v4/wxs/util">
  <Bundle Name="envr Setup"
          Version="$Version"
          Manufacturer="$Manufacturer"
          UpgradeCode="$bundleUpgradeCode">
    <BootstrapperApplication>
      <bal:WixStandardBootstrapperApplication Theme="hyperlinkLicense"
                                              LicenseUrl="https://www.apache.org/licenses/LICENSE-2.0.txt" />
    </BootstrapperApplication>

    <util:RegistrySearch Id="VcRuntimeInstalled"
                         Variable="VCREDIST_INSTALLED"
                         Root="HKLM"
                         Key="SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\$runtimeKeyArch"
                         Value="Installed"
                         Result="value" />

    <Chain>
      <ExePackage Id="VcRedist"
                  SourceFile="$vcRedistExe"
                  PerMachine="yes"
                  Permanent="yes"
                  Vital="yes"
                  DetectCondition="VCREDIST_INSTALLED"
                  InstallArguments="/install /quiet /norestart"
                  RepairArguments="/repair /quiet /norestart"
                  UninstallArguments="/uninstall /quiet /norestart" />

      <MsiPackage Id="EnvrMsi"
                  SourceFile="$msiPath"
                  Visible="no"
                  Vital="yes">
        <MsiProperty Name="INSTALLSCOPE" Value="$InstallScope" />
        <MsiProperty Name="PATHSCOPE" Value="$PathScope" />
      </MsiPackage>
    </Chain>
  </Bundle>
</Wix>
"@ | Set-Content -Path $bundleWxs -Encoding utf8

if (Test-Path -LiteralPath $setupExe) {
    Remove-Item -Force -LiteralPath $setupExe
}

Write-Host "Building setup bootstrapper via WiX..."
wix build -acceptEula wix7 -ext WixToolset.BootstrapperApplications.wixext -ext WixToolset.Util.wixext $bundleWxs -arch $bundleBuildArch -o $setupExe
if ($LASTEXITCODE -ne 0) {
    throw "WiX bundle build failed with exit code $LASTEXITCODE."
}

if (-not (Test-Path -LiteralPath $setupExe)) {
    throw "setup.exe build finished without output file: $setupExe"
}

$hash = Get-FileHash -Algorithm SHA256 -LiteralPath $setupExe
"$($hash.Hash.ToLowerInvariant())  $(Split-Path -Leaf $setupExe)" |
    Set-Content -Path (Join-Path $OutRoot "SHA256SUMS-setup.txt") -Encoding utf8

Write-Host "Done."
Write-Host "  MSI:    $msiPath"
Write-Host "  Setup:  $setupExe"
Write-Host "  Verify: Get-FileHash -Algorithm SHA256 $setupExe"
