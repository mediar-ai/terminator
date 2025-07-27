# Test script for Docker Windows setup
# This script tests the Terminator MCP Agent Docker container

param(
    [string]$ContainerUrl = "http://localhost:8080",
    [switch]$Verbose
)

Write-Host "🧪 Testing Terminator MCP Agent Docker Setup" -ForegroundColor Cyan
Write-Host "=" * 50

# Function to test HTTP endpoint
function Test-HttpEndpoint {
    param(
        [string]$Url,
        [string]$Description
    )
    
    Write-Host "🔍 Testing $Description..." -NoNewline
    
    try {
        $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 10
        if ($response.StatusCode -eq 200) {
            Write-Host " ✅ OK" -ForegroundColor Green
            if ($Verbose) {
                Write-Host "   Response: $($response.Content)" -ForegroundColor Gray
            }
            return $true
        } else {
            Write-Host " ❌ Failed (Status: $($response.StatusCode))" -ForegroundColor Red
            return $false
        }
    } catch {
        Write-Host " ❌ Failed ($($_.Exception.Message))" -ForegroundColor Red
        return $false
    }
}

# Function to check if container is running
function Test-ContainerStatus {
    Write-Host "🐳 Checking Docker container status..." -NoNewline
    
    try {
        $containers = docker ps --filter "name=terminator-mcp-windows" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
        
        if ($containers -match "terminator-mcp-windows") {
            Write-Host " ✅ Running" -ForegroundColor Green
            if ($Verbose) {
                Write-Host "   Container info:" -ForegroundColor Gray
                Write-Host "   $containers" -ForegroundColor Gray
            }
            return $true
        } else {
            Write-Host " ❌ Not running" -ForegroundColor Red
            Write-Host "   💡 Start with: docker-compose -f docker-compose.windows.yml up" -ForegroundColor Yellow
            return $false
        }
    } catch {
        Write-Host " ❌ Docker not available ($($_.Exception.Message))" -ForegroundColor Red
        return $false
    }
}

# Function to check Docker mode
function Test-DockerMode {
    Write-Host "🔧 Checking Docker container mode..." -NoNewline
    
    try {
        $dockerInfo = docker version --format json | ConvertFrom-Json
        $serverOS = $dockerInfo.Server.Os
        
        if ($serverOS -eq "windows") {
            Write-Host " ✅ Windows containers" -ForegroundColor Green
            return $true
        } else {
            Write-Host " ❌ Linux containers (need Windows)" -ForegroundColor Red
            Write-Host "   💡 Switch with: & `"C:\Program Files\Docker\Docker\DockerCli.exe`" -SwitchDaemon" -ForegroundColor Yellow
            return $false
        }
    } catch {
        Write-Host " ❌ Cannot determine Docker mode" -ForegroundColor Red
        return $false
    }
}

# Function to test MCP functionality
function Test-MCPFunctionality {
    param([string]$BaseUrl)
    
    Write-Host "🔧 Testing MCP functionality..." -NoNewline
    
    # This is a basic test - for full MCP testing, use the Python client
    try {
        $mcpUrl = "$BaseUrl/mcp"
        $response = Invoke-WebRequest -Uri $mcpUrl -UseBasicParsing -TimeoutSec 10
        
        # MCP endpoint should return some response (might be an error for GET, but should respond)
        Write-Host " ✅ MCP endpoint responding" -ForegroundColor Green
        if ($Verbose) {
            Write-Host "   MCP URL: $mcpUrl" -ForegroundColor Gray
        }
        return $true
    } catch {
        Write-Host " ❌ MCP endpoint not responding" -ForegroundColor Red
        return $false
    }
}

# Main test sequence
Write-Host ""

# Test 1: Docker mode
$dockerModeOk = Test-DockerMode

# Test 2: Container status
$containerOk = Test-ContainerStatus

# Test 3: Health endpoint
$healthOk = Test-HttpEndpoint "$ContainerUrl/health" "Health endpoint"

# Test 4: MCP endpoint
$mcpOk = Test-MCPFunctionality $ContainerUrl

# Summary
Write-Host ""
Write-Host "📊 Test Summary:" -ForegroundColor Cyan
Write-Host "=" * 30

$tests = @(
    @{ Name = "Docker Windows Mode"; Status = $dockerModeOk },
    @{ Name = "Container Running"; Status = $containerOk },
    @{ Name = "Health Endpoint"; Status = $healthOk },
    @{ Name = "MCP Endpoint"; Status = $mcpOk }
)

$passedTests = 0
foreach ($test in $tests) {
    $status = if ($test.Status) { "✅ PASS" } else { "❌ FAIL" }
    $color = if ($test.Status) { "Green" } else { "Red" }
    Write-Host "  $($test.Name): " -NoNewline
    Write-Host $status -ForegroundColor $color
    if ($test.Status) { $passedTests++ }
}

Write-Host ""
Write-Host "Results: $passedTests/$($tests.Count) tests passed" -ForegroundColor $(if ($passedTests -eq $tests.Count) { "Green" } else { "Yellow" })

# Recommendations
if ($passedTests -lt $tests.Count) {
    Write-Host ""
    Write-Host "🔧 Troubleshooting Steps:" -ForegroundColor Yellow
    Write-Host "1. Ensure Docker Desktop is running in Windows container mode"
    Write-Host "2. Build and start the container:"
    Write-Host "   cd docker"
    Write-Host "   docker-compose -f docker-compose.windows.yml up --build"
    Write-Host "3. Check container logs:"
    Write-Host "   docker logs terminator-mcp-windows"
    Write-Host "4. Test with Python client:"
    Write-Host "   python examples/docker_mcp_simple.py --demo"
}

if ($passedTests -eq $tests.Count) {
    Write-Host ""
    Write-Host "🎉 All tests passed! The Docker setup is working correctly." -ForegroundColor Green
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Cyan
    Write-Host "• Test with Python client: python examples/docker_mcp_simple.py"
    Write-Host "• Run interactive mode: python examples/docker_mcp_client.py"
    Write-Host "• Connect your MCP client to: $ContainerUrl/mcp"
}

Write-Host ""
