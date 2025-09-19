# Test deployment to Azure VM
Write-Host "Testing agent deployment on Azure VM..." -ForegroundColor Cyan

$resourceGroup = "REMOTE-UI-TEST-RG"
$vmName = "ui-test-vm"

# Simple test command
$script = 'Write-Output "Deployment test successful"; mkdir C:\agent -Force; Write-Output "Directory created"'

Write-Host "Running command on VM..." -ForegroundColor Green
az vm run-command invoke `
    --resource-group $resourceGroup `
    --name $vmName `
    --command-id RunPowerShellScript `
    --scripts $script

Write-Host ""
Write-Host "Agent binary is at: .\target\release\remote-ui-agent.exe" -ForegroundColor Yellow
Write-Host ""
Write-Host "Manual deployment steps:" -ForegroundColor Cyan
Write-Host "1. Connect via RDP: mstsc /v:20.57.76.232" -ForegroundColor White
Write-Host "2. Copy agent binary to VM" -ForegroundColor White
Write-Host "3. Run on VM: .\remote-ui-agent.exe --port 8080" -ForegroundColor White