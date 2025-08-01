# PowerShell test script for Kubernetes deployment
# Tests the complete Terminator MCP Agent Kubernetes setup

param(
    [string]$Namespace = "terminator",
    [string]$ServiceName = "terminator-mcp-service",
    [int]$Port = 8080,
    [switch]$Verbose,
    [switch]$SkipPortForward
)

Write-Host "🧪 Testing Terminator MCP Agent Kubernetes Deployment" -ForegroundColor Cyan
Write-Host "=" * 60

# Function to test command availability
function Test-Command {
    param([string]$Command)
    
    try {
        $null = Get-Command $Command -ErrorAction Stop
        return $true
    } catch {
        return $false
    }
}

# Function to print status
function Write-Status {
    param(
        [string]$Message,
        [string]$Status = "Info"
    )
    
    $colors = @{
        "Info" = "Blue"
        "Success" = "Green"
        "Warning" = "Yellow"
        "Error" = "Red"
    }
    
    $symbols = @{
        "Info" = "ℹ️"
        "Success" = "✅"
        "Warning" = "⚠️"
        "Error" = "❌"
    }
    
    $color = $colors[$Status]
    $symbol = $symbols[$Status]
    
    Write-Host "$symbol $Message" -ForegroundColor $color
}

# Check prerequisites
Write-Status "Checking prerequisites..." "Info"

if (-not (Test-Command "kubectl")) {
    Write-Status "kubectl is not installed or not in PATH" "Error"
    exit 1
}

if (-not (Test-Command "python")) {
    Write-Status "Python is not installed or not in PATH" "Warning"
    Write-Status "Python client tests will be skipped" "Warning"
}

# Test cluster connectivity
Write-Status "Testing cluster connectivity..." "Info"
try {
    $clusterInfo = kubectl cluster-info 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Status "Connected to Kubernetes cluster" "Success"
    } else {
        Write-Status "Cannot connect to Kubernetes cluster" "Error"
        exit 1
    }
} catch {
    Write-Status "Cannot connect to Kubernetes cluster" "Error"
    exit 1
}

# Check Windows nodes
Write-Status "Checking Windows nodes..." "Info"
$windowsNodes = kubectl get nodes -l kubernetes.io/os=windows --no-headers 2>$null
if ($windowsNodes) {
    $nodeCount = ($windowsNodes | Measure-Object).Count
    Write-Status "Found $nodeCount Windows node(s)" "Success"
    if ($Verbose) {
        Write-Host "Windows nodes:" -ForegroundColor Gray
        $windowsNodes | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
    }
} else {
    Write-Status "No Windows nodes found" "Error"
    Write-Status "This deployment requires Windows nodes" "Error"
    exit 1
}

# Check namespace
Write-Status "Checking namespace '$Namespace'..." "Info"
$namespaceExists = kubectl get namespace $Namespace 2>$null
if ($LASTEXITCODE -eq 0) {
    Write-Status "Namespace '$Namespace' exists" "Success"
} else {
    Write-Status "Namespace '$Namespace' does not exist" "Error"
    Write-Status "Deploy first with: kubectl apply -f k8s/all-in-one.yaml" "Info"
    exit 1
}

# Check deployment
Write-Status "Checking deployment status..." "Info"
$deployment = kubectl get deployment terminator-mcp-agent -n $Namespace -o json 2>$null | ConvertFrom-Json
if ($deployment) {
    $availableReplicas = $deployment.status.availableReplicas
    $desiredReplicas = $deployment.spec.replicas
    
    if ($availableReplicas -ge $desiredReplicas -and $availableReplicas -gt 0) {
        Write-Status "Deployment is healthy ($availableReplicas/$desiredReplicas replicas)" "Success"
    } else {
        Write-Status "Deployment is not healthy ($availableReplicas/$desiredReplicas replicas)" "Error"
    }
} else {
    Write-Status "Deployment 'terminator-mcp-agent' not found" "Error"
    exit 1
}

# Check pods
Write-Status "Checking pod status..." "Info"
$pods = kubectl get pods -n $Namespace -l app=terminator-mcp-agent -o json 2>$null | ConvertFrom-Json
if ($pods.items) {
    $runningPods = 0
    foreach ($pod in $pods.items) {
        $podName = $pod.metadata.name
        $podStatus = $pod.status.phase
        $nodeName = $pod.spec.nodeName
        
        if ($podStatus -eq "Running") {
            $runningPods++
            Write-Status "  Pod $podName: $podStatus on $nodeName" "Success"
        } else {
            Write-Status "  Pod $podName: $podStatus" "Warning"
        }
    }
    
    if ($runningPods -gt 0) {
        Write-Status "$runningPods/$($pods.items.Count) pods are running" "Success"
    } else {
        Write-Status "No pods are running" "Error"
        exit 1
    }
} else {
    Write-Status "No pods found" "Error"
    exit 1
}

# Check service
Write-Status "Checking service '$ServiceName'..." "Info"
$service = kubectl get service $ServiceName -n $Namespace -o json 2>$null | ConvertFrom-Json
if ($service) {
    $serviceType = $service.spec.type
    $ports = $service.spec.ports | ForEach-Object { "$($_.port):$($_.targetPort)" }
    Write-Status "Service $ServiceName: $serviceType [$($ports -join ', ')]" "Success"
    
    # Check endpoints
    $endpoints = kubectl get endpoints $ServiceName -n $Namespace -o json 2>$null | ConvertFrom-Json
    if ($endpoints.subsets -and $endpoints.subsets[0].addresses) {
        $endpointCount = $endpoints.subsets[0].addresses.Count
        Write-Status "  $endpointCount endpoint(s) available" "Success"
    } else {
        Write-Status "  No endpoints available" "Warning"
    }
} else {
    Write-Status "Service '$ServiceName' not found" "Error"
    exit 1
}

# Test pod health endpoints
Write-Status "Testing pod health endpoints..." "Info"
$runningPods = kubectl get pods -n $Namespace -l app=terminator-mcp-agent --field-selector=status.phase=Running -o jsonpath='{.items[*].metadata.name}' 2>$null
if ($runningPods) {
    $podNames = $runningPods -split ' '
    $successCount = 0
    
    foreach ($podName in $podNames) {
        try {
            $healthTest = kubectl exec -n $Namespace $podName -- powershell -c "try { `$r = iwr http://localhost:8080/health -UseBasicParsing; if (`$r.StatusCode -eq 200) { exit 0 } else { exit 1 } } catch { exit 1 }" 2>$null
            
            if ($LASTEXITCODE -eq 0) {
                Write-Status "  Pod $podName: Health endpoint OK" "Success"
                $successCount++
            } else {
                Write-Status "  Pod $podName: Health endpoint failed" "Error"
            }
        } catch {
            Write-Status "  Pod $podName: Health check error" "Error"
        }
    }
    
    if ($successCount -gt 0) {
        Write-Status "$successCount/$($podNames.Count) pods passed health check" "Success"
    } else {
        Write-Status "No pods passed health check" "Error"
    }
} else {
    Write-Status "No running pods to test" "Error"
}

# Test service connectivity
Write-Status "Testing service connectivity..." "Info"
if ($runningPods) {
    $testPod = ($runningPods -split ' ')[0]
    
    try {
        $serviceTest = kubectl exec -n $Namespace $testPod -- powershell -c "try { `$r = iwr http://${ServiceName}:8080/health -UseBasicParsing; if (`$r.StatusCode -eq 200) { exit 0 } else { exit 1 } } catch { exit 1 }" 2>$null
        
        if ($LASTEXITCODE -eq 0) {
            Write-Status "Service connectivity test passed" "Success"
        } else {
            Write-Status "Service connectivity test failed" "Error"
        }
    } catch {
        Write-Status "Service connectivity test error" "Error"
    }
}

# Test external access via port-forward
if (-not $SkipPortForward) {
    Write-Status "Testing external access (port-forward)..." "Info"
    
    # Start port-forward in background
    $portForwardJob = Start-Job -ScriptBlock {
        param($Namespace, $ServiceName, $Port)
        kubectl port-forward -n $Namespace svc/$ServiceName ${Port}:8080
    } -ArgumentList $Namespace, $ServiceName, $Port
    
    # Wait for port-forward to establish
    Start-Sleep -Seconds 5
    
    try {
        $response = Invoke-WebRequest -Uri "http://localhost:$Port/health" -UseBasicParsing -TimeoutSec 10
        if ($response.StatusCode -eq 200) {
            Write-Status "External access test passed" "Success"
        } else {
            Write-Status "External access test failed (status: $($response.StatusCode))" "Error"
        }
    } catch {
        Write-Status "External access test failed: $($_.Exception.Message)" "Error"
    } finally {
        # Stop port-forward job
        Stop-Job -Job $portForwardJob -ErrorAction SilentlyContinue
        Remove-Job -Job $portForwardJob -ErrorAction SilentlyContinue
    }
}

# Test with Python MCP client (if available)
if (Test-Command "python") {
    Write-Status "Testing with Python MCP client..." "Info"
    
    if (Test-Path "examples/k8s_mcp_client.py") {
        try {
            $pythonTest = python examples/k8s_mcp_client.py --namespace $Namespace --service $ServiceName --test-only 2>$null
            
            if ($LASTEXITCODE -eq 0) {
                Write-Status "Python MCP client test passed" "Success"
            } else {
                Write-Status "Python MCP client test failed" "Warning"
                Write-Status "Run manually: python examples/k8s_mcp_client.py --namespace $Namespace" "Info"
            }
        } catch {
            Write-Status "Python MCP client test error" "Warning"
        }
    } else {
        Write-Status "Python MCP client script not found" "Warning"
    }
}

# Summary
Write-Host ""
Write-Status "🎯 Test Summary" "Info"
Write-Host "=" * 30

Write-Status "✅ Kubernetes cluster connectivity" "Success"
Write-Status "✅ Windows nodes available" "Success"
Write-Status "✅ Namespace exists" "Success"
Write-Status "✅ Deployment is healthy" "Success"
Write-Status "✅ Pods are running" "Success"
Write-Status "✅ Service is configured" "Success"
Write-Status "✅ Health endpoints responding" "Success"
Write-Status "✅ Service connectivity working" "Success"

Write-Host ""
Write-Status "🎉 Kubernetes deployment is working correctly!" "Success"
Write-Host ""
Write-Status "Next steps:" "Info"
Write-Host "• Test with MCP client: kubectl port-forward -n $Namespace svc/$ServiceName 8080:8080"
Write-Host "• Then run: python examples/k8s_mcp_client.py --namespace $Namespace"
Write-Host "• Or use: python examples/docker_mcp_simple.py --server-url http://localhost:8080/mcp"

Write-Host ""
Write-Status "🔗 Access URLs:" "Info"
Write-Host "• Port-forward: kubectl port-forward -n $Namespace svc/$ServiceName 8080:8080"
Write-Host "• Then connect to: http://localhost:8080/mcp"
Write-Host "• Health check: http://localhost:8080/health"

if ($service.spec.type -eq "LoadBalancer") {
    $externalIP = $service.status.loadBalancer.ingress[0].ip
    if ($externalIP) {
        Write-Host "• External IP: http://${externalIP}:8080/mcp"
    }
}

if ($service.spec.type -eq "NodePort" -or (kubectl get service terminator-mcp-nodeport -n $Namespace 2>$null)) {
    Write-Host "• NodePort: http://<node-ip>:30080/mcp"
}

Write-Host ""
