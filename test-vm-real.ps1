# Test REMOTE Azure VM - NOT LOCAL
Write-Host "TESTING AZURE VM - NOT LOCAL!" -ForegroundColor Red

# Start agent ON THE VM
$cmd = 'cd C:\agent; if (Test-Path agent.exe) { Start-Process C:\agent\agent.exe -ArgumentList "--port 8080" } else { "No agent" }; netstat -an | findstr 8080'

Write-Host "Starting agent ON VM..." -ForegroundColor Yellow
az vm run-command invoke -g REMOTE-UI-TEST-RG -n ui-test-vm --command-id RunPowerShellScript --scripts $cmd

Start-Sleep 10

# Test REMOTE VM
$vmIP = "20.57.76.232"
Write-Host "Testing $vmIP..." -ForegroundColor Cyan

$test = Invoke-WebRequest -Uri "http://${vmIP}:8080/health" -TimeoutSec 5 -UseBasicParsing -ErrorAction SilentlyContinue
if ($test) {
    Write-Host "[SUCCESS] REMOTE VM WORKING!" -ForegroundColor Green
    Write-Host $test.Content
} else {
    Write-Host "Not responding" -ForegroundColor Red
}