Write-Host "Starting agent ON THE AZURE VM..." -ForegroundColor Red

$start = @'
cd C:\agent
if (Test-Path agent.exe) {
    Write-Output "Starting agent..."
    Start-Job -ScriptBlock { C:\agent\agent.exe --port 8080 }
    Start-Sleep 5
    Get-Process | Where Name -like "*agent*" | Select Name, Id
    netstat -an | Select-String ":8080"
} else {
    Write-Output "No agent.exe found"
    dir C:\agent
}
'@

az vm run-command invoke -g REMOTE-UI-TEST-RG -n ui-test-vm --command-id RunPowerShellScript --scripts $start