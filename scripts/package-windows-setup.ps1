# Build setup bootstrapper (setup.exe) via WiX Burn.
# Usage (from repo root):
#   .\scripts\package-windows-setup.ps1 -Version 0.1.0 -OutRoot dist
param(
    [string]$Version = "0.1.0",
    [string]$OutRoot = "dist",
    [string]$Manufacturer = "envr",
    [string]$VcRedistUrl = "https://aka.ms/vs/17/release/vc_redist.x64.exe",
    [string]$VcRedistPath = ""
)

$ErrorActionPreference = "Stop"

if ($Version -notmatch '^\d+\.\d+\.\d+(\.\d+)?$') {
    throw "Version must be Bundle-compatible: Major.Minor.Build[.Revision], got '$Version'."
}

$wixCmd = Get-Command wix -ErrorAction SilentlyContinue
if (-not $wixCmd) {
    throw "WiX v4 CLI not found. Install with: dotnet tool install --global wix"
}

Write-Host "Ensuring WiX extensions (BootstrapperApplications, Util) are available..."
wix extension add -g WixToolset.BootstrapperApplications.wixext
if ($LASTEXITCODE -ne 0) {
    throw "Failed to install WiX extension: WixToolset.BootstrapperApplications.wixext"
}
wix extension add -g WixToolset.Util.wixext
if ($LASTEXITCODE -ne 0) {
    throw "Failed to install WiX extension: WixToolset.Util.wixext"
}
$extList = wix extension list -g | Out-String
if ($extList -notmatch "WixToolset\.BootstrapperApplications\.wixext") {
    throw "WiX extension WixToolset.BootstrapperApplications.wixext is missing after install."
}
if ($extList -notmatch "WixToolset\.Util\.wixext") {
    throw "WiX extension WixToolset.Util.wixext is missing after install."
}

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$msiPath = Join-Path $OutRoot "envr-windows-x64-$Version.msi"
if (-not (Test-Path -LiteralPath $msiPath)) {
    Write-Host "MSI not found. Building MSI first..."
    & (Join-Path $PSScriptRoot "package-windows-msi.ps1") -Version $Version -OutRoot $OutRoot -Manufacturer $Manufacturer
}

if (-not (Test-Path -LiteralPath $msiPath)) {
    throw "MSI is still missing after build: $msiPath"
}

$stageName = "envr-windows-setup-x64-$Version"
$stage = Join-Path $OutRoot $stageName
New-Item -ItemType Directory -Force -Path $stage | Out-Null

$vcRedistExe = Join-Path $stage "vc_redist.x64.exe"
if ($VcRedistPath -and (Test-Path -LiteralPath $VcRedistPath)) {
    Copy-Item -LiteralPath $VcRedistPath -Destination $vcRedistExe -Force
}
else {
    Write-Host "Downloading VC++ Redistributable from $VcRedistUrl ..."
    Invoke-WebRequest -Uri $VcRedistUrl -OutFile $vcRedistExe
}

if (-not (Test-Path -LiteralPath $vcRedistExe)) {
    throw "vc_redist.x64.exe is missing: $vcRedistExe"
}

$bundleWxs = Join-Path $stage "envr-setup.wxs"
$setupExe = Join-Path $OutRoot "envr-setup-x64-$Version.exe"
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
                         Key="SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64"
                         Value="Installed"
                         Result="value" />

    <Chain>
      <ExePackage Id="VcRedistX64"
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
                  Vital="yes" />
    </Chain>
  </Bundle>
</Wix>
"@ | Set-Content -Path $bundleWxs -Encoding utf8

if (Test-Path -LiteralPath $setupExe) {
    Remove-Item -Force -LiteralPath $setupExe
}

Write-Host "Building setup bootstrapper via WiX..."
wix build -acceptEula wix7 -ext WixToolset.BootstrapperApplications.wixext -ext WixToolset.Util.wixext $bundleWxs -arch x64 -o $setupExe
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
