# Deploy and run the agent on Azure VM
Write-Host "Deploying Remote UI Agent to Azure VM..." -ForegroundColor Cyan

$resourceGroup = "REMOTE-UI-TEST-RG"
$vmName = "ui-test-vm"

# First, let's download the agent binary from a temporary location
# Since we can't directly upload files, we'll use a workaround

$deployScript = @'
Write-Host "Setting up Remote UI Agent on VM..."

# Create agent directory
New-Item -ItemType Directory -Path "C:\remote-agent" -Force | Out-Null

# For testing, create a simple mock agent that responds to requests
$agentCode = @'
# Simple HTTP server for testing
Add-Type -AssemblyName System.Net.Http

$listener = New-Object System.Net.HttpListener
$listener.Prefixes.Add("http://+:8080/")
$listener.Start()

Write-Host "Simple test server started on port 8080"

while ($listener.IsListening) {
    try {
        $context = $listener.GetContext()
        $request = $context.Request
        $response = $context.Response

        Write-Host "Received request: $($request.HttpMethod) $($request.Url.LocalPath)"

        $responseString = ""

        switch ($request.Url.LocalPath) {
            "/health" {
                $responseString = '{"status":"healthy","service":"remote-ui-test","timestamp":"' + (Get-Date -Format o) + '"}'
            }
            "/execute" {
                # Simple response for testing
                $responseString = '{"success":true,"message":"Command received on Azure VM","vm_name":"' + $env:COMPUTERNAME + '"}'
            }
            default {
                $responseString = '{"error":"Unknown endpoint"}'
            }
        }

        $buffer = [System.Text.Encoding]::UTF8.GetBytes($responseString)
        $response.ContentLength64 = $buffer.Length
        $response.OutputStream.Write($buffer, 0, $buffer.Length)
        $response.Close()
    }
    catch {
        Write-Host "Error: $_"
    }
}
'@

$agentCode | Out-File -FilePath "C:\remote-agent\test-server.ps1" -Encoding UTF8

# Configure firewall
Write-Host "Configuring firewall..."
Remove-NetFirewallRule -DisplayName "Remote Agent Port 8080" -ErrorAction SilentlyContinue
New-NetFirewallRule -DisplayName "Remote Agent Port 8080" `
    -Direction Inbound `
    -Protocol TCP `
    -LocalPort 8080 `
    -Action Allow `
    -Profile Any | Out-Null

# Start the test server
Write-Host "Starting test server..."
Start-Process powershell.exe -ArgumentList "-ExecutionPolicy Bypass -File C:\remote-agent\test-server.ps1" -WindowStyle Hidden

Write-Host "Test server deployment complete"
Write-Host "Server should be accessible on port 8080"

# Check if it's running
Start-Sleep -Seconds 2
netstat -an | findstr :8080
'@

Write-Host "Executing deployment script on VM..." -ForegroundColor Green
$result = az vm run-command invoke `
    --resource-group $resourceGroup `
    --name $vmName `
    --command-id RunPowerShellScript `
    --scripts $deployScript `
    --output json | ConvertFrom-Json

if ($result.value) {
    $stdout = $result.value | Where-Object {$_.code -eq "ComponentStatus/StdOut/succeeded"}
    if ($stdout -and $stdout.message) {
        Write-Host "`nVM Output:" -ForegroundColor Yellow
        Write-Host $stdout.message
    }
}

Write-Host "`nWaiting for server to initialize..." -ForegroundColor Yellow
Start-Sleep -Seconds 5

# Test connectivity
Write-Host "`nTesting connectivity to Azure VM agent..." -ForegroundColor Green
$publicIP = "20.57.76.232"

try {
    $response = Invoke-WebRequest -Uri "http://${publicIP}:8080/health" -TimeoutSec 10 -UseBasicParsing
    Write-Host "✓ Agent is accessible!" -ForegroundColor Green
    Write-Host "Response: $($response.Content)" -ForegroundColor White
} catch {
    Write-Host "✗ Could not connect to agent on VM" -ForegroundColor Red
    Write-Host "Error: $_" -ForegroundColor Gray
    Write-Host "`nThis might be due to Windows Firewall or NSG rules." -ForegroundColor Yellow
    Write-Host "You may need to RDP to the VM and check if the server is running." -ForegroundColor Yellow
}