# PowerShell script to deploy the remote UI agent to Azure VM

param(
    [Parameter(Mandatory=$true)]
    [string]$VMPublicIP,

    [Parameter(Mandatory=$false)]
    [string]$Username = "azureuser",

    [Parameter(Mandatory=$false)]
    [string]$Password = "RemoteUI2024!",

    [Parameter(Mandatory=$false)]
    [string]$AgentPath = ".\terminator-mcp-agent\target\release\remote-ui-agent.exe"
)

Write-Host "Building the remote UI agent..." -ForegroundColor Green
cd terminator-mcp-agent
cargo build --release --bin remote-ui-agent
cd ..

if (-not (Test-Path $AgentPath)) {
    Write-Error "Agent executable not found at: $AgentPath"
    exit 1
}

Write-Host "Creating deployment package..." -ForegroundColor Green

# Create a temporary directory for deployment files
$TempDir = New-TemporaryFile | %{ rm $_; mkdir $_ }
Copy-Item $AgentPath -Destination "$TempDir\remote-ui-agent.exe"

# Create a startup script
$StartupScript = @"
@echo off
cd C:\remote-agent
start /B remote-ui-agent.exe --port 8080 --verbose
echo Remote UI Agent started on port 8080
"@

$StartupScript | Out-File -FilePath "$TempDir\start-agent.bat" -Encoding ASCII

# Create installation script
$InstallScript = @'
# Install script for Remote UI Agent
$ErrorActionPreference = "Stop"

Write-Host "Installing Remote UI Agent..." -ForegroundColor Green

# Create directory
New-Item -ItemType Directory -Path "C:\remote-agent" -Force

# Copy files
Copy-Item ".\remote-ui-agent.exe" -Destination "C:\remote-agent\" -Force
Copy-Item ".\start-agent.bat" -Destination "C:\remote-agent\" -Force

# Create Windows Firewall rule
New-NetFirewallRule -DisplayName "Remote UI Agent" `
                    -Direction Inbound `
                    -Protocol TCP `
                    -LocalPort 8080 `
                    -Action Allow `
                    -Profile Any `
                    -ErrorAction SilentlyContinue

# Install as Windows Service (optional)
# sc.exe create RemoteUIAgent binPath= "C:\remote-agent\remote-ui-agent.exe --port 8080" start= auto

# Start the agent
Start-Process "C:\remote-agent\start-agent.bat" -WindowStyle Hidden

Write-Host "Remote UI Agent installed successfully!" -ForegroundColor Green
Write-Host "Agent is running on port 8080" -ForegroundColor Yellow
'@

$InstallScript | Out-File -FilePath "$TempDir\install.ps1" -Encoding UTF8

Write-Host "Connecting to VM at $VMPublicIP..." -ForegroundColor Green

# Use PsExec or RDP to copy and run the installation
# For now, we'll use Azure VM Extension

Write-Host "Note: Manual deployment steps:" -ForegroundColor Yellow
Write-Host "1. RDP to the VM: mstsc /v:${VMPublicIP}:3389" -ForegroundColor Cyan
Write-Host "2. Copy files from $TempDir to the VM" -ForegroundColor Cyan
Write-Host "3. Run install.ps1 as Administrator" -ForegroundColor Cyan
Write-Host ""
Write-Host "Or use Azure VM Extension for automated deployment:" -ForegroundColor Yellow

$DeployCommand = @"
az vm extension set `
  --resource-group REMOTE-UI-TEST-RG `
  --vm-name ui-test-vm `
  --name CustomScriptExtension `
  --publisher Microsoft.Compute `
  --version 1.10 `
  --settings '{\"commandToExecute\": \"powershell -ExecutionPolicy Bypass -Command \`"Invoke-WebRequest -Uri https://raw.githubusercontent.com/your-org/scripts/main/install-agent.ps1 -OutFile install.ps1; .\\install.ps1\`"\"}'
"@

Write-Host $DeployCommand -ForegroundColor Gray
Write-Host ""
Write-Host "Deployment package created at: $TempDir" -ForegroundColor Green