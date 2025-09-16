# Check if agent is running on Azure VM
$vmIP = "20.57.76.232"

Write-Host "Checking Azure VM Agent..." -ForegroundColor Cyan

# Quick health check
try {
    $response = Invoke-WebRequest -Uri "http://${vmIP}:8080/health" -TimeoutSec 3 -UseBasicParsing -ErrorAction Stop
    Write-Host "[SUCCESS] Agent is running!" -ForegroundColor Green
    $response.Content
} catch {
    Write-Host "[INFO] Agent not responding on port 8080" -ForegroundColor Yellow

    # Check VM
    Write-Host "Checking VM status..." -ForegroundColor Cyan
    az vm get-instance-view -g REMOTE-UI-TEST-RG -n ui-test-vm --query "instanceView.statuses[1].displayStatus" -o tsv
}