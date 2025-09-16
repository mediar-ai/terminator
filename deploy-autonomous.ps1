# Autonomous deployment of agent to Azure VM
$resourceGroup = "REMOTE-UI-TEST-RG"
$vmName = "ui-test-vm"
$agentPath = ".\target\release\remote-ui-agent.exe"

Write-Host "Starting autonomous deployment..." -ForegroundColor Cyan

# Read and encode agent
$fileBytes = [System.IO.File]::ReadAllBytes($agentPath)
$base64 = [Convert]::ToBase64String($fileBytes)
$chunkSize = 50000
$chunks = [Math]::Ceiling($base64.Length / $chunkSize)

Write-Host "Agent: $([Math]::Round($fileBytes.Length / 1MB, 2)) MB in $chunks chunks" -ForegroundColor Yellow

# Setup VM directory
$setup = 'mkdir C:\agent -Force; Remove-Item C:\agent\* -Force -ErrorAction SilentlyContinue; "Ready"'
az vm run-command invoke -g $resourceGroup -n $vmName --command-id RunPowerShellScript --scripts $setup --output none

# Transfer chunks
for ($i = 0; $i -lt $chunks; $i++) {
    $chunk = $base64.Substring($i * $chunkSize, [Math]::Min($chunkSize, $base64.Length - $i * $chunkSize))
    Write-Host "Chunk $($i+1)/$chunks..." -NoNewline
    $cmd = "Add-Content -Path C:\agent\a.b64 -Value '$chunk'"
    az vm run-command invoke -g $resourceGroup -n $vmName --command-id RunPowerShellScript --scripts $cmd --output none
    Write-Host " done" -ForegroundColor Green
}

# Decode and run
Write-Host "Starting agent..." -ForegroundColor Green
$run = @'
cd C:\agent
$b = Get-Content a.b64 -Raw
[IO.File]::WriteAllBytes("C:\agent\agent.exe", [Convert]::FromBase64String($b))
Remove-Item a.b64
netsh advfirewall firewall add rule name="Agent8080" dir=in action=allow protocol=TCP localport=8080
Start-Process -FilePath C:\agent\agent.exe -ArgumentList "--port 8080" -WindowStyle Hidden
Start-Sleep 3
netstat -an | findstr 8080
'@

az vm run-command invoke -g $resourceGroup -n $vmName --command-id RunPowerShellScript --scripts $run

Write-Host "Done! Testing..." -ForegroundColor Cyan
Start-Sleep 5

$test = Invoke-WebRequest -Uri "http://20.57.76.232:8080/health" -TimeoutSec 5 -UseBasicParsing -ErrorAction SilentlyContinue
if ($test) {
    Write-Host "SUCCESS! Agent running on VM" -ForegroundColor Green
    Write-Host $test.Content
} else {
    Write-Host "Checking..." -ForegroundColor Yellow
}