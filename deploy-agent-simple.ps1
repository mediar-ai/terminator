# Simple script to deploy remote UI agent to Azure VM

Write-Host "=== Deploying Remote UI Agent to Azure VM ===" -ForegroundColor Cyan

$resourceGroup = "REMOTE-UI-TEST-RG"
$vmName = "ui-test-vm"

# First, create a simple test script to verify deployment works
$testScript = @'
Write-Host "Testing agent deployment on VM..."
Write-Host "Current directory: $(Get-Location)"
Write-Host "Creating agent directory..."

# Create directory
New-Item -ItemType Directory -Path "C:\remote-agent" -Force | Out-Null

# Create a test file
"Agent deployed at $(Get-Date)" | Out-File -FilePath "C:\remote-agent\deployment.txt"

# Check if deployment worked
if (Test-Path "C:\remote-agent\deployment.txt") {
    Write-Host "SUCCESS: Agent directory created and accessible"
    Get-Content "C:\remote-agent\deployment.txt"
} else {
    Write-Host "ERROR: Failed to create agent directory"
}

# Open firewall port for agent
Write-Host "Configuring firewall for port 8080..."
New-NetFirewallRule -DisplayName "Remote UI Agent Port 8080" `
    -Direction Inbound `
    -Protocol TCP `
    -LocalPort 8080 `
    -Action Allow `
    -Profile Any `
    -ErrorAction SilentlyContinue | Out-Null

Write-Host "Firewall rule configured"

# Display network configuration
Write-Host "Network interfaces:"
Get-NetIPAddress | Where-Object {$_.AddressFamily -eq "IPv4" -and $_.PrefixOrigin -ne "WellKnown"} | Format-Table IPAddress, InterfaceAlias

Write-Host "Agent deployment test complete"
'@

Write-Host "Running deployment test on VM..." -ForegroundColor Green
$result = az vm run-command invoke `
    --resource-group $resourceGroup `
    --name $vmName `
    --command-id RunPowerShellScript `
    --scripts $testScript `
    --output json | ConvertFrom-Json

if ($result.value) {
    $stdout = $result.value | Where-Object {$_.code -eq "ComponentStatus/StdOut/succeeded"}
    if ($stdout -and $stdout.message) {
        Write-Host "VM Output:" -ForegroundColor Yellow
        Write-Host $stdout.message
    }

    $stderr = $result.value | Where-Object {$_.code -eq "ComponentStatus/StdErr/succeeded"}
    if ($stderr -and $stderr.message) {
        Write-Host "Errors:" -ForegroundColor Red
        Write-Host $stderr.message
    }
}

Write-Host ""
Write-Host "=== Next Steps ===" -ForegroundColor Cyan
Write-Host "1. Copy the agent binary to the VM via RDP" -ForegroundColor White
Write-Host "2. Run the agent on the VM: .\remote-ui-agent.exe --port 8080" -ForegroundColor White
Write-Host "3. Test connectivity from local machine" -ForegroundColor White
Write-Host ""
Write-Host "Agent binary location: .\target\release\remote-ui-agent.exe" -ForegroundColor Yellow