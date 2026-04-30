# Build release binaries and produce a Windows MSI via WiX v4.
# Usage (from repo root):
#   .\scripts\package-windows-msi.ps1 -Version 0.1.0 -OutRoot dist
param(
    [string]$Version = "0.1.0",
    [string]$OutRoot = "dist",
    [ValidateSet("x64", "arm64")]
    [string]$Arch = "x64",
    [string]$Target = "",
    [string]$Manufacturer = "envr",
    [switch]$AcceptEula
)

$ErrorActionPreference = "Stop"

function Normalize-MsiVersion {
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

    throw "Version must be MSI-compatible: Major.Minor.Build[.Revision], got '$InputVersion'."
}

$Version = Normalize-MsiVersion -InputVersion $Version

$wixCmd = Get-Command wix -ErrorAction SilentlyContinue
if (-not $wixCmd) {
    throw "WiX v4 CLI not found. Install with: dotnet tool install --global wix"
}

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$destName = "envr-windows-$Arch-$Version"
$dest = Join-Path $OutRoot $destName
New-Item -ItemType Directory -Force -Path $dest | Out-Null

Write-Host "Building release (envr, er, envr-gui, envr-shim)..."
if ($Target) {
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
    Copy-Item -LiteralPath $p -Destination (Join-Path $dest $e) -Force
}

$wxsPath = Join-Path $dest "envr-installer.wxs"
$msiPath = Join-Path $OutRoot "envr-windows-x64-$Version.msi"

$upgradeCode = "9A3D6A5A-41AF-4F1D-9A57-0E37D08D0B9F"
$cmpMainCode = "A31C22B6-6CC9-42E1-B37C-6FFFA36A5E20"
$cmpErCode = "BC753FAE-2D65-4D4D-9C5E-B45B2D73D3B9"
$cmpGuiCode = "D31D3D68-5A58-4A3A-8EE8-036A71D66F44"
$cmpShimCode = "98C8CB8F-8F6B-4AF2-A93E-76B17393D2D5"
$cmpPathCode = "3E936F40-E47B-4A5E-BBD6-33E9A7D50328"

@"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="envr" Manufacturer="$Manufacturer" Version="$Version" UpgradeCode="$upgradeCode" Language="1033" Scope="perMachine">
    <SummaryInformation Description="envr runtime manager" />
    <MajorUpgrade DowngradeErrorMessage="A newer version of envr is already installed." />
    <MediaTemplate EmbedCab="yes" />

    <StandardDirectory Id="ProgramFiles64Folder">
      <Directory Id="INSTALLFOLDER" Name="envr">
        <Component Id="cmp_envr_exe" Guid="$cmpMainCode">
          <File Id="fil_envr_exe" Source="$dest\envr.exe" KeyPath="yes" />
        </Component>
        <Component Id="cmp_er_exe" Guid="$cmpErCode">
          <File Id="fil_er_exe" Source="$dest\er.exe" KeyPath="yes" />
        </Component>
        <Component Id="cmp_envr_gui_exe" Guid="$cmpGuiCode">
          <File Id="fil_envr_gui_exe" Source="$dest\envr-gui.exe" KeyPath="yes" />
        </Component>
        <Component Id="cmp_envr_shim_exe" Guid="$cmpShimCode">
          <File Id="fil_envr_shim_exe" Source="$dest\envr-shim.exe" KeyPath="yes" />
        </Component>
        <Component Id="cmp_path_env" Guid="$cmpPathCode">
          <CreateFolder />
          <Environment Id="envr_path_machine" Name="PATH" System="yes" Action="set" Part="last" Value="[INSTALLFOLDER]" />
        </Component>
      </Directory>
    </StandardDirectory>

    <Feature Id="MainFeature" Title="envr" Level="1">
      <ComponentRef Id="cmp_envr_exe" />
      <ComponentRef Id="cmp_er_exe" />
      <ComponentRef Id="cmp_envr_gui_exe" />
      <ComponentRef Id="cmp_envr_shim_exe" />
      <ComponentRef Id="cmp_path_env" />
    </Feature>
  </Package>
</Wix>
"@ | Set-Content -Path $wxsPath -Encoding utf8

if (Test-Path -LiteralPath $msiPath) {
    Remove-Item -Force -LiteralPath $msiPath
}

Write-Host "Building MSI via WiX..."
$wixArgs = @("build")
if ($AcceptEula) {
    $wixArgs += @("-acceptEula", "wix7")
}
$wixArgs += @($wxsPath, "-arch", $Arch, "-o", $msiPath)
& wix @wixArgs
if ($LASTEXITCODE -ne 0) {
    throw "WiX build failed with exit code $LASTEXITCODE."
}

if (-not (Test-Path -LiteralPath $msiPath)) {
    throw "MSI build finished without output file: $msiPath"
}

$hash = Get-FileHash -Algorithm SHA256 -LiteralPath $msiPath
"$($hash.Hash.ToLowerInvariant())  $(Split-Path -Leaf $msiPath)" |
    Set-Content -Path (Join-Path $OutRoot "SHA256SUMS-msi.txt") -Encoding utf8

Write-Host "Done."
Write-Host "  Stage: $dest"
Write-Host "  MSI:   $msiPath"
Write-Host "  Verify: Get-FileHash -Algorithm SHA256 $msiPath"
