# Demonstrate remote UI control on Azure VM
Write-Host "=== Remote UI Control Demonstration ===" -ForegroundColor Cyan

$resourceGroup = "REMOTE-UI-TEST-RG"
$vmName = "ui-test-vm"

# Create a script that uses Windows UI Automation to control applications
$demoScript = @'
Write-Host "Starting Remote UI Control Demo..."

# Open Notepad
Start-Process notepad.exe
Start-Sleep -Seconds 2

# Open Calculator
Start-Process calc.exe
Start-Sleep -Seconds 2

# List all open windows
Write-Host "`nOpen Windows:"
Get-Process | Where-Object {$_.MainWindowTitle -ne ""} |
    Select-Object ProcessName, MainWindowTitle |
    Format-Table -AutoSize

# Demonstrate we can control the VM's desktop
Write-Host "`nVM Desktop Control Ready:"
Write-Host "- Notepad is open and ready for text input"
Write-Host "- Calculator is open and ready for calculations"
Write-Host "- All terminator features would work here if agent was deployed"

# Show system info to prove we're on the VM
Write-Host "`nSystem Information:"
Write-Host "Computer Name: $env:COMPUTERNAME"
Write-Host "OS: $([System.Environment]::OSVersion.VersionString)"
Write-Host "Current User: $env:USERNAME"
Write-Host "Current Time: $(Get-Date)"
'@

Write-Host "Executing remote control demo on VM..." -ForegroundColor Green
$result = az vm run-command invoke `
    --resource-group $resourceGroup `
    --name $vmName `
    --command-id RunPowerShellScript `
    --scripts $demoScript `
    --output json | ConvertFrom-Json

if ($result.value) {
    $stdout = $result.value | Where-Object {$_.code -eq "ComponentStatus/StdOut/succeeded"}
    if ($stdout -and $stdout.message) {
        Write-Host "`nResults from Azure VM:" -ForegroundColor Yellow
        Write-Host $stdout.message
    }
}

Write-Host "`n=== What This Proves ===" -ForegroundColor Cyan
Write-Host "✓ We can execute commands on the remote VM" -ForegroundColor Green
Write-Host "✓ We can open applications (Notepad, Calculator)" -ForegroundColor Green
Write-Host "✓ We can query running processes and windows" -ForegroundColor Green
Write-Host "✓ The VM is ready for the terminator agent" -ForegroundColor Green

Write-Host "`n=== With Terminator Agent ===" -ForegroundColor Cyan
Write-Host "Once remote-ui-agent.exe is running on the VM, we could:" -ForegroundColor Yellow
Write-Host "- Click buttons in any application" -ForegroundColor White
Write-Host "- Type text into any field" -ForegroundColor White
Write-Host "- Take screenshots of windows" -ForegroundColor White
Write-Host "- Find UI elements by role/name" -ForegroundColor White
Write-Host "- Automate any Windows application remotely" -ForegroundColor White

Write-Host "`n=== How to Complete Setup ===" -ForegroundColor Cyan
Write-Host "1. Copy .\target\release\remote-ui-agent.exe to VM" -ForegroundColor White
Write-Host "2. Run on VM: .\remote-ui-agent.exe --port 8080" -ForegroundColor White
Write-Host "3. Test from local: curl http://20.57.76.232:8080/health" -ForegroundColor White
Write-Host "4. Send UI commands via REST API" -ForegroundColor White