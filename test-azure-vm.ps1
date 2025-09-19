# Test script to verify Azure VM connectivity and deploy a simple HTTP server

param(
    [Parameter(Mandatory=$false)]
    [string]$ResourceGroup = "REMOTE-UI-TEST-RG",

    [Parameter(Mandatory=$false)]
    [string]$VMName = "ui-test-vm"
)

Write-Host "=== Azure VM Remote UI Automation Test ===" -ForegroundColor Cyan
Write-Host ""

# Get VM details
Write-Host "Getting VM details..." -ForegroundColor Green
$vmInfo = az vm show -d --resource-group $ResourceGroup --name $VMName | ConvertFrom-Json

Write-Host "VM Name: $($vmInfo.name)" -ForegroundColor Yellow
Write-Host "Public IP: $($vmInfo.publicIps)" -ForegroundColor Yellow
Write-Host "Power State: $($vmInfo.powerState)" -ForegroundColor Yellow
Write-Host ""

# Test connectivity
Write-Host "Testing connectivity to VM..." -ForegroundColor Green
$publicIP = $vmInfo.publicIps

# Test RDP port
Write-Host "Testing RDP port (3389)..." -ForegroundColor Cyan
Test-NetConnection -ComputerName $publicIP -Port 3389 | Format-Table -Property ComputerName, RemotePort, TcpTestSucceeded

# Test Remote Agent port
Write-Host "Testing Remote Agent port (8080)..." -ForegroundColor Cyan
Test-NetConnection -ComputerName $publicIP -Port 8080 | Format-Table -Property ComputerName, RemotePort, TcpTestSucceeded

Write-Host ""
Write-Host "Deploying simple Python HTTP server to test..." -ForegroundColor Green

# Create a simple test script
$testScript = @'
import http.server
import socketserver
import json
from datetime import datetime

PORT = 8080

class TestHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/health':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            response = {
                'status': 'healthy',
                'service': 'remote-ui-automation-test',
                'timestamp': datetime.now().isoformat()
            }
            self.wfile.write(json.dumps(response).encode())
        else:
            super().do_GET()

    def do_POST(self):
        if self.path == '/execute':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)

            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()

            response = {
                'success': True,
                'message': 'Test response from Azure VM',
                'received': json.loads(post_data.decode())
            }
            self.wfile.write(json.dumps(response).encode())

print(f"Starting test server on port {PORT}")
with socketserver.TCPServer(("", PORT), TestHandler) as httpd:
    httpd.serve_forever()
'@

$testScript | Out-File -FilePath "test-server.py" -Encoding UTF8

Write-Host "Using Azure VM Run Command to deploy test server..." -ForegroundColor Green

# Deploy using VM run command
$deployScript = @"
# Create directory
mkdir C:\test-agent 2>nul

# Install Python if needed
if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
    Write-Host "Installing Python..."
    Invoke-WebRequest -Uri "https://www.python.org/ftp/python/3.11.0/python-3.11.0-amd64.exe" -OutFile "C:\temp\python-installer.exe"
    Start-Process -FilePath "C:\temp\python-installer.exe" -ArgumentList "/quiet", "InstallAllUsers=1", "PrependPath=1" -Wait
}

# Create test server script
@'
$($testScript -replace "'", "''")
'@ | Out-File -FilePath "C:\test-agent\server.py" -Encoding UTF8

# Create firewall rule
New-NetFirewallRule -DisplayName "Test Agent Port" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow -ErrorAction SilentlyContinue

# Start the server
Start-Process python -ArgumentList "C:\test-agent\server.py" -WorkingDirectory "C:\test-agent" -WindowStyle Hidden

Write-Host "Test server deployed and started on port 8080"
"@

Write-Host "Running deployment command on VM..." -ForegroundColor Yellow
az vm run-command invoke `
    --resource-group $ResourceGroup `
    --name $VMName `
    --command-id RunPowerShellScript `
    --scripts $deployScript

Write-Host ""
Write-Host "Waiting for server to start..." -ForegroundColor Yellow
Start-Sleep -Seconds 10

Write-Host "Testing HTTP endpoint..." -ForegroundColor Green
try {
    $response = Invoke-RestMethod -Uri "http://${publicIP}:8080/health" -Method Get -TimeoutSec 5
    Write-Host "✓ Health check successful:" -ForegroundColor Green
    $response | ConvertTo-Json | Write-Host
} catch {
    Write-Host "✗ Health check failed: $_" -ForegroundColor Red
}

Write-Host ""
Write-Host "Testing automation endpoint..." -ForegroundColor Green
try {
    $testPayload = @{
        action = @{
            type = "GetApplications"
        }
        request_id = [System.Guid]::NewGuid().ToString()
    } | ConvertTo-Json

    $response = Invoke-RestMethod -Uri "http://${publicIP}:8080/execute" -Method Post -Body $testPayload -ContentType "application/json" -TimeoutSec 5
    Write-Host "✓ Execute endpoint test successful:" -ForegroundColor Green
    $response | ConvertTo-Json | Write-Host
} catch {
    Write-Host "✗ Execute endpoint test failed: $_" -ForegroundColor Red
}

Write-Host ""
Write-Host "=== Test Summary ===" -ForegroundColor Cyan
Write-Host "VM Public IP: $publicIP" -ForegroundColor Yellow
Write-Host "RDP Connection: mstsc /v:${publicIP}:3389" -ForegroundColor Yellow
Write-Host "Username: azureuser" -ForegroundColor Yellow
Write-Host "Password: RemoteUI2024!" -ForegroundColor Yellow
Write-Host ""
Write-Host "To manually test the VM:" -ForegroundColor Green
Write-Host "1. RDP to the VM using the above credentials" -ForegroundColor White
Write-Host "2. Open PowerShell as Administrator" -ForegroundColor White
Write-Host "3. Check if test server is running: netstat -an | findstr :8080" -ForegroundColor White
Write-Host "4. Check firewall rules: Get-NetFirewallRule | Where DisplayName -like '*8080*'" -ForegroundColor White