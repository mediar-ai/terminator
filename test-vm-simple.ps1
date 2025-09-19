# Simple test to verify VM is accessible and can run our code

Write-Host "=== Testing Azure VM Connection ===" -ForegroundColor Cyan

$publicIP = "20.57.76.232"
$resourceGroup = "REMOTE-UI-TEST-RG"
$vmName = "ui-test-vm"

Write-Host "VM Public IP: $publicIP" -ForegroundColor Yellow

# Test basic connectivity
Write-Host "`nTesting RDP connectivity..." -ForegroundColor Green
$rdpTest = Test-NetConnection -ComputerName $publicIP -Port 3389 -WarningAction SilentlyContinue
if ($rdpTest.TcpTestSucceeded) {
    Write-Host "✓ RDP port is open" -ForegroundColor Green
} else {
    Write-Host "✗ RDP port is not accessible" -ForegroundColor Red
}

# Run a simple command on the VM to verify it's working
Write-Host "`nRunning test command on VM..." -ForegroundColor Green

$testScript = @'
Write-Output "VM is accessible and running"
Write-Output "Computer Name: $env:COMPUTERNAME"
Write-Output "Windows Version: $([System.Environment]::OSVersion.VersionString)"
Write-Output "Current Time: $(Get-Date)"

# Check if PowerShell execution is working
Write-Output "PowerShell Version: $($PSVersionTable.PSVersion)"

# Test network
Write-Output "Testing outbound connectivity..."
Test-NetConnection google.com -Port 80 -InformationLevel Quiet

# Check firewall rules
Write-Output "Checking firewall rules for port 8080..."
Get-NetFirewallRule | Where-Object {$_.DisplayName -like "*8080*"} | Select-Object DisplayName, Enabled, Action

# Check if any process is listening on port 8080
Write-Output "Checking if port 8080 is in use..."
netstat -an | findstr :8080
'@

Write-Host "Executing script on VM via Azure..." -ForegroundColor Yellow
$result = az vm run-command invoke `
    --resource-group $resourceGroup `
    --name $vmName `
    --command-id RunPowerShellScript `
    --scripts $testScript `
    --output json | ConvertFrom-Json

if ($result.value) {
    Write-Host "`nVM Response:" -ForegroundColor Green

    # Get stdout
    $stdout = $result.value | Where-Object {$_.code -eq "ComponentStatus/StdOut/succeeded"}
    if ($stdout.message) {
        Write-Host $stdout.message -ForegroundColor White
    }

    # Get stderr if any
    $stderr = $result.value | Where-Object {$_.code -eq "ComponentStatus/StdErr/succeeded"}
    if ($stderr.message) {
        Write-Host "Errors:" -ForegroundColor Red
        Write-Host $stderr.message -ForegroundColor Red
    }
}

Write-Host "`n=== Creating Simple HTTP Test Server ===" -ForegroundColor Cyan

# Create a simple HTTP server using netsh http
$serverScript = @'
Write-Host "Setting up HTTP listener on port 8080..." -ForegroundColor Green

# Create firewall rule
New-NetFirewallRule -DisplayName "Test HTTP Port 8080" `
    -Direction Inbound `
    -Protocol TCP `
    -LocalPort 8080 `
    -Action Allow `
    -Profile Any `
    -ErrorAction SilentlyContinue | Out-Null

# Reserve URL for HTTP.SYS
netsh http add urlacl url=http://+:8080/ user=Everyone

# Simple Python HTTP server as fallback
$pythonScript = @"
from http.server import HTTPServer, BaseHTTPRequestHandler
import json

class TestHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-type', 'application/json')
        self.end_headers()
        response = {'status': 'healthy', 'message': 'Test server on Azure VM'}
        self.wfile.write(json.dumps(response).encode())

    def do_POST(self):
        self.do_GET()

print('Starting HTTP server on port 8080...')
server = HTTPServer(('', 8080), TestHandler)
server.serve_forever()
"@ | Out-File -FilePath C:\test-server.py -Encoding UTF8

# Try to start with Python if available
if (Get-Command python -ErrorAction SilentlyContinue) {
    Write-Host "Starting Python HTTP server..." -ForegroundColor Green
    Start-Process python -ArgumentList "C:\test-server.py" -WindowStyle Hidden
} else {
    Write-Host "Python not found, using PowerShell HTTP listener..." -ForegroundColor Yellow
    # PowerShell HTTP listener
    $http = [System.Net.HttpListener]::new()
    $http.Prefixes.Add("http://+:8080/")
    $http.Start()
    Write-Host "HTTP Server started on port 8080"

    while ($http.IsListening) {
        $context = $http.GetContext()
        $response = $context.Response
        $content = '{"status":"healthy","message":"PowerShell test server"}'
        $buffer = [System.Text.Encoding]::UTF8.GetBytes($content)
        $response.ContentLength64 = $buffer.Length
        $response.OutputStream.Write($buffer, 0, $buffer.Length)
        $response.Close()
    }
}

Write-Host "Test server configured on port 8080"
'@

Write-Host "Deploying test HTTP server to VM..." -ForegroundColor Yellow
$deployResult = az vm run-command invoke `
    --resource-group $resourceGroup `
    --name $vmName `
    --command-id RunPowerShellScript `
    --scripts $serverScript `
    --no-wait

Write-Host "Server deployment initiated (running in background)" -ForegroundColor Green

Write-Host "`nWaiting for server to start..." -ForegroundColor Yellow
Start-Sleep -Seconds 15

Write-Host "`nTesting HTTP endpoint from local machine..." -ForegroundColor Green
try {
    $response = Invoke-WebRequest -Uri "http://${publicIP}:8080/" -TimeoutSec 5 -UseBasicParsing
    Write-Host "✓ HTTP test successful!" -ForegroundColor Green
    Write-Host "Response: $($response.Content)" -ForegroundColor White
} catch {
    Write-Host "✗ HTTP test failed (this is expected if Windows Firewall is blocking external access)" -ForegroundColor Yellow
    Write-Host "Error: $_" -ForegroundColor Gray
}

Write-Host "`n=== Connection Instructions ===" -ForegroundColor Cyan
Write-Host "To connect via RDP and test manually:" -ForegroundColor Green
Write-Host "1. Open Remote Desktop Connection (mstsc.exe)" -ForegroundColor White
Write-Host "2. Computer: $publicIP" -ForegroundColor White
Write-Host "3. Username: azureuser" -ForegroundColor White
Write-Host "4. Password: RemoteUI2024!" -ForegroundColor White
Write-Host ""
Write-Host "Once connected, open PowerShell and run:" -ForegroundColor Green
Write-Host "  Test-NetConnection localhost -Port 8080" -ForegroundColor White
Write-Host "  Invoke-RestMethod http://localhost:8080/" -ForegroundColor White