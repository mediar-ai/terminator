# Basic VM connector test script
Write-Host "=== Testing VM Connector with Azure VM ===" -ForegroundColor Cyan

# Set environment variables
$env:VM_TYPE = "azure"
$env:AZURE_SUBSCRIPTION_ID = "5c0a60d0-92cf-47ca-9430-b462bc2fe194"
$env:AZURE_RESOURCE_GROUP = "REMOTE-UI-TEST-RG"
$env:AZURE_VM_NAME = "ui-test-vm"

Write-Host "Environment configured:" -ForegroundColor Green
Write-Host "  VM_TYPE: $env:VM_TYPE"
Write-Host "  Resource Group: $env:AZURE_RESOURCE_GROUP"
Write-Host "  VM Name: $env:AZURE_VM_NAME"

# Test VM status
Write-Host ""
Write-Host "Getting VM status..." -ForegroundColor Green
$status = az vm get-instance-view `
    --resource-group $env:AZURE_RESOURCE_GROUP `
    --name $env:AZURE_VM_NAME `
    --query "instanceView.statuses[1].displayStatus" `
    -o tsv

Write-Host "VM Status: $status" -ForegroundColor Yellow

# Get VM public IP
Write-Host ""
Write-Host "Getting VM public IP..." -ForegroundColor Green
$publicIP = az vm show -d `
    --resource-group $env:AZURE_RESOURCE_GROUP `
    --name $env:AZURE_VM_NAME `
    --query "publicIps" `
    -o tsv

Write-Host "Public IP: $publicIP" -ForegroundColor Yellow

# Test connectivity
Write-Host ""
Write-Host "Testing RDP connectivity..." -ForegroundColor Green
Test-NetConnection -ComputerName $publicIP -Port 3389 -WarningAction SilentlyContinue | Format-Table TcpTestSucceeded

Write-Host ""
Write-Host "Testing agent port connectivity..." -ForegroundColor Green
Test-NetConnection -ComputerName $publicIP -Port 8080 -WarningAction SilentlyContinue | Format-Table TcpTestSucceeded

Write-Host ""
Write-Host "=== Summary ===" -ForegroundColor Cyan
Write-Host "VM Name: ui-test-vm" -ForegroundColor Green
Write-Host "Resource Group: REMOTE-UI-TEST-RG" -ForegroundColor Green
Write-Host "Public IP: $publicIP" -ForegroundColor Green
Write-Host "VM Status: $status" -ForegroundColor Green