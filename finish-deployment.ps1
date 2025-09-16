# Finish agent deployment on VM
Write-Host "Completing agent deployment..." -ForegroundColor Cyan

$finish = @'
cd C:\agent
if (Test-Path a.b64) {
    $b = Get-Content a.b64 -Raw
    [IO.File]::WriteAllBytes("C:\agent\agent.exe", [Convert]::FromBase64String($b))
    Remove-Item a.b64 -Force
    netsh advfirewall firewall add rule name="UIAgent" dir=in action=allow protocol=TCP localport=8080 | Out-Null
    Start-Process C:\agent\agent.exe -ArgumentList "--port 8080" -WindowStyle Hidden
    Start-Sleep 3
    "Agent started"
} else { "No file to decode" }
'@

$result = az vm run-command invoke -g REMOTE-UI-TEST-RG -n ui-test-vm --command-id RunPowerShellScript --scripts $finish --output json 2>$null

if ($result) {
    $parsed = $result | ConvertFrom-Json
    Write-Host $parsed.value[0].message
}

Start-Sleep 10
Write-Host "Testing agent..." -ForegroundColor Green

try {
    $response = Invoke-WebRequest -Uri "http://20.57.76.232:8080/health" -TimeoutSec 5 -UseBasicParsing
    Write-Host "[SUCCESS] Agent is running on Azure VM!" -ForegroundColor Green
    $response.Content
} catch {
    Write-Host "[INFO] May need manual verification" -ForegroundColor Yellow
}