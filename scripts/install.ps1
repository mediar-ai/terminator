<#
Quick installer for Terminator CLI (Windows)
Usage (latest): powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/mediar-ai/terminator/main/scripts/install.ps1 | iex"
Usage (specific): powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/mediar-ai/terminator/main/scripts/install.ps1 | iex" -ArgumentList 'cli-v1.2.3'
#>
param(
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"
$Repo = "mediar-ai/terminator"

function Get-Latest {
  (Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest").tag_name
}

if (-not $Version) {
  $Version = Get-Latest
}

$arch = switch (([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture).ToString()) {
  "Arm64" { "aarch64" }
  "X64"  { "x86_64" }
  Default { throw "Unsupported architecture" }
}

$archive = "terminator-cli-windows-$arch.zip"
$url = "https://github.com/$Repo/releases/download/$Version/$archive"
$tempFile = Join-Path $env:TEMP $archive
Write-Host "Downloading $url" -ForegroundColor Cyan
Invoke-WebRequest -Uri $url -OutFile $tempFile -UseBasicParsing

$tempDir = Join-Path $env:TEMP "terminator-cli"
if (Test-Path $tempDir) { Remove-Item $tempDir -Recurse -Force }
New-Item -ItemType Directory -Path $tempDir | Out-Null
Expand-Archive -Path $tempFile -DestinationPath $tempDir -Force

$binPath = Join-Path $tempDir "terminator-cli.exe"
$installDir = "$env:ProgramFiles"
$destPath = Join-Path $installDir "terminator-cli.exe"
Move-Item -Path $binPath -Destination $destPath -Force

Write-Host "✅ Terminator CLI installed at $destPath. Add it to your PATH if necessary." -ForegroundColor Green