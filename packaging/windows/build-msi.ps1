# PowerShell script to build Windows MSI installer
# Requires: WiX Toolset v4+ (https://wixtoolset.org/)

param(
    [string]$Version = "0.5.0",
    [string]$SourceDir = "..\..\target\release",
    [string]$OutputDir = ".\output"
)

$ErrorActionPreference = "Stop"

Write-Host "Building OpenHush MSI installer v$Version" -ForegroundColor Cyan

# Check prerequisites
if (-not (Get-Command "wix" -ErrorAction SilentlyContinue)) {
    Write-Error "WiX Toolset not found. Install from https://wixtoolset.org/"
    exit 1
}

# Check if binary exists
if (-not (Test-Path "$SourceDir\openhush.exe")) {
    Write-Error "openhush.exe not found in $SourceDir"
    Write-Host "Build with: cargo build --release"
    exit 1
}

# Create output directory
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

# Create LICENSE.rtf if not exists
if (-not (Test-Path "$SourceDir\LICENSE.rtf")) {
    Write-Host "Creating LICENSE.rtf..."
    $license = Get-Content "..\..\LICENSE" -Raw
    $rtf = "{\rtf1\ansi\deff0 {\fonttbl {\f0 Consolas;}}\f0\fs20 $($license -replace "`n", "\par ")}}"
    $rtf | Out-File -FilePath "$SourceDir\LICENSE.rtf" -Encoding ASCII
}

# Create placeholder icon if not exists
if (-not (Test-Path "$SourceDir\openhush.ico")) {
    Write-Host "Warning: openhush.ico not found, MSI will be built without icon" -ForegroundColor Yellow
}

# Build MSI
Write-Host "Running WiX build..."
wix build openhush.wxs `
    -d SourceDir="$SourceDir" `
    -d Version="$Version" `
    -o "$OutputDir\OpenHush-$Version-x64.msi"

if ($LASTEXITCODE -eq 0) {
    Write-Host "MSI created: $OutputDir\OpenHush-$Version-x64.msi" -ForegroundColor Green
} else {
    Write-Error "WiX build failed"
    exit 1
}

# Calculate SHA256
$hash = Get-FileHash "$OutputDir\OpenHush-$Version-x64.msi" -Algorithm SHA256
Write-Host "SHA256: $($hash.Hash)" -ForegroundColor Yellow
