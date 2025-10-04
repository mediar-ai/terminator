<#
Quick installer for Terminator Agent (Windows)
Usage (latest): powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/mediar-ai/terminator/main/scripts/install-agent.ps1 | iex"
Usage (specific): powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/mediar-ai/terminator/main/scripts/install-agent.ps1 | iex" -ArgumentList 'cli-v1.2.3'
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
  "Arm64" { 
    Write-Host "❌ Terminator Agent is not available for ARM64 architecture" -ForegroundColor Red
    Write-Host "Only Terminator CLI is available for ARM64/aarch64" -ForegroundColor Yellow
    Write-Host "Please use the CLI installer instead." -ForegroundColor Yellow
    exit 1
  }
  "X64"  { "x64" }
  Default { throw "Unsupported architecture" }
}

$archive = "terminator-mcp-agent-win32-$arch-msvc.zip"
$url = "https://github.com/$Repo/releases/download/$Version/$archive"
$tempFile = Join-Path $env:TEMP $archive
Write-Host "Downloading $url" -ForegroundColor Cyan
Invoke-WebRequest -Uri $url -OutFile $tempFile -UseBasicParsing

$tempDir = Join-Path $env:TEMP "terminator-agent"
if (Test-Path $tempDir) { Remove-Item $tempDir -Recurse -Force }
New-Item -ItemType Directory -Path $tempDir | Out-Null
Expand-Archive -Path $tempFile -DestinationPath $tempDir -Force

$binPath = Join-Path $tempDir "terminator-mcp-agent.exe"
$installDir = "$env:ProgramFiles"
$destPath = Join-Path $installDir "terminator-mcp-agent.exe"
Move-Item -Path $binPath -Destination $destPath -Force

# Set permissions for all users to read and execute
icacls "$destPath" /grant "Users:(RX)" | Out-Null

$currentIP = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -notlike "*Loopback*" -and $_.IPAddress -notlike "169.254.*" } | Select-Object -First 1).IPAddress

Write-Host "✅ Terminator Agent installed at $destPath" -ForegroundColor Green
Write-Host ""
Write-Host "You can also run it like:" -ForegroundColor Cyan
Write-Host "`"$destPath`" --transport http --port 3000 --host $currentIP" -ForegroundColor Yellow
Write-Host ""
Write-Host "⚠️  Do not forget to adjust firewall settings." -ForegroundColor Red
