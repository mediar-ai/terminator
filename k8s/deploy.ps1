# PowerShell deployment script for Terminator MCP Agent on Kubernetes
# Usage: .\deploy.ps1 -Environment "production" -Action "deploy"

param(
    [string]$Environment = "development",
    [string]$Action = "deploy",
    [int]$Replicas = 3,
    [switch]$Verbose
)

# Configuration
$Namespace = "terminator"
$AppName = "terminator-mcp-agent"

# Logging functions
function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message" -ForegroundColor Green
}

function Write-Warning {
    param([string]$Message)
    Write-Host "[WARNING] $Message" -ForegroundColor Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

# Check prerequisites
function Test-Prerequisites {
    Write-Info "Checking prerequisites..."
    
    # Check kubectl
    try {
        $null = kubectl version --client --short 2>$null
    } catch {
        Write-Error "kubectl is not installed or not in PATH"
        exit 1
    }
    
    # Check cluster connectivity
    try {
        $null = kubectl cluster-info 2>$null
    } catch {
        Write-Error "Cannot connect to Kubernetes cluster"
        exit 1
    }
    
    # Check Windows nodes
    $windowsNodes = kubectl get nodes -l kubernetes.io/os=windows --no-headers 2>$null
    if (-not $windowsNodes) {
        Write-Error "No Windows nodes found in the cluster"
        exit 1
    }
    
    $nodeCount = ($windowsNodes | Measure-Object).Count
    Write-Success "Prerequisites check passed"
    Write-Info "Found $nodeCount Windows node(s)"
}

# Deploy function
function Invoke-Deploy {
    Write-Info "Deploying Terminator MCP Agent to $Environment environment..."
    
    # Create namespace if it doesn't exist
    kubectl create namespace $Namespace --dry-run=client -o yaml | kubectl apply -f -
    
    # Apply resources in order
    $resources = @(
        "configmap.yaml",
        "rbac.yaml", 
        "pvc.yaml",
        "deployment.yaml",
        "service.yaml",
        "hpa.yaml",
        "pdb.yaml",
        "networkpolicy.yaml",
        "ingress.yaml"
    )
    
    foreach ($resource in $resources) {
        Write-Info "Applying $resource..."
        kubectl apply -f $resource
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Failed to apply $resource"
            exit 1
        }
    }
    
    # Wait for deployment to be ready
    Write-Info "Waiting for deployment to be ready..."
    kubectl wait --for=condition=available --timeout=300s deployment/$AppName -n $Namespace
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Deployment completed successfully!"
    } else {
        Write-Error "Deployment failed or timed out"
        exit 1
    }
}

# Status function
function Get-Status {
    Write-Info "Checking deployment status..."
    
    Write-Host ""
    Write-Info "Namespace:"
    kubectl get namespace $Namespace
    
    Write-Host ""
    Write-Info "Pods:"
    kubectl get pods -n $Namespace -o wide
    
    Write-Host ""
    Write-Info "Services:"
    kubectl get svc -n $Namespace
    
    Write-Host ""
    Write-Info "Ingress:"
    kubectl get ingress -n $Namespace
    
    Write-Host ""
    Write-Info "HPA:"
    kubectl get hpa -n $Namespace
    
    Write-Host ""
    Write-Info "Recent Events:"
    kubectl get events -n $Namespace --sort-by='.lastTimestamp' | Select-Object -Last 10
}

# Test function
function Invoke-Test {
    Write-Info "Testing deployment..."
    
    # Check if pods are running
    $runningPods = kubectl get pods -n $Namespace -l app=$AppName --field-selector=status.phase=Running --no-headers 2>$null
    if (-not $runningPods) {
        Write-Error "No running pods found"
        return $false
    }
    
    $podCount = ($runningPods | Measure-Object).Count
    Write-Success "$podCount pod(s) are running"
    
    # Get first pod name
    $podName = kubectl get pods -n $Namespace -l app=$AppName -o jsonpath='{.items[0].metadata.name}' 2>$null
    if (-not $podName) {
        Write-Error "Could not get pod name"
        return $false
    }
    
    # Test health endpoint
    Write-Info "Testing health endpoint..."
    $healthTest = kubectl exec -n $Namespace $podName -- powershell -c "try { `$r = iwr http://localhost:8080/health -UseBasicParsing; if (`$r.StatusCode -eq 200) { exit 0 } else { exit 1 } } catch { exit 1 }" 2>$null
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Health endpoint is responding"
    } else {
        Write-Error "Health endpoint is not responding"
        return $false
    }
    
    # Test service connectivity
    Write-Info "Testing service connectivity..."
    $serviceTest = kubectl exec -n $Namespace $podName -- powershell -c "try { `$r = iwr http://terminator-mcp-service:8080/health -UseBasicParsing; if (`$r.StatusCode -eq 200) { exit 0 } else { exit 1 } } catch { exit 1 }" 2>$null
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Service connectivity is working"
    } else {
        Write-Error "Service connectivity is not working"
        return $false
    }
    
    Write-Success "All tests passed!"
    return $true
}

# Cleanup function
function Invoke-Cleanup {
    Write-Warning "Cleaning up deployment..."
    
    $resources = @(
        "ingress.yaml",
        "networkpolicy.yaml",
        "pdb.yaml",
        "hpa.yaml",
        "service.yaml",
        "deployment.yaml",
        "pvc.yaml",
        "rbac.yaml",
        "configmap.yaml"
    )
    
    foreach ($resource in $resources) {
        Write-Info "Deleting $resource..."
        kubectl delete -f $resource --ignore-not-found=true
    }
    
    # Optionally delete namespace
    $deleteNamespace = Read-Host "Delete namespace '$Namespace'? (y/N)"
    if ($deleteNamespace -eq 'y' -or $deleteNamespace -eq 'Y') {
        kubectl delete namespace $Namespace --ignore-not-found=true
        Write-Success "Namespace deleted"
    }
    
    Write-Success "Cleanup completed"
}

# Logs function
function Get-Logs {
    Write-Info "Showing logs for $AppName..."
    kubectl logs -n $Namespace -l app=$AppName --tail=100 -f
}

# Scale function
function Set-Scale {
    Write-Info "Scaling deployment to $Replicas replicas..."
    kubectl scale deployment/$AppName -n $Namespace --replicas=$Replicas
    kubectl wait --for=condition=available --timeout=300s deployment/$AppName -n $Namespace
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Scaled to $Replicas replicas"
    } else {
        Write-Error "Scaling failed"
        exit 1
    }
}

# Port forward function
function Start-PortForward {
    Write-Info "Starting port forward to MCP service..."
    Write-Info "MCP endpoint will be available at: http://localhost:8080/mcp"
    Write-Info "Health endpoint will be available at: http://localhost:8080/health"
    Write-Info "Press Ctrl+C to stop port forwarding"
    
    kubectl port-forward -n $Namespace svc/terminator-mcp-service 8080:8080
}

# Main execution
switch ($Action.ToLower()) {
    "deploy" {
        Test-Prerequisites
        Invoke-Deploy
        Get-Status
    }
    "status" {
        Get-Status
    }
    "test" {
        $testResult = Invoke-Test
        if (-not $testResult) {
            exit 1
        }
    }
    "cleanup" {
        Invoke-Cleanup
    }
    "logs" {
        Get-Logs
    }
    "scale" {
        Set-Scale
    }
    "port-forward" {
        Start-PortForward
    }
    default {
        Write-Host "Usage: .\deploy.ps1 -Environment [environment] -Action [action]"
        Write-Host "Actions: deploy, status, test, cleanup, logs, scale, port-forward"
        Write-Host "Example: .\deploy.ps1 -Environment production -Action deploy"
        exit 1
    }
}
