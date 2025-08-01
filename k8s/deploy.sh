#!/bin/bash
# Deployment script for Terminator MCP Agent on Kubernetes
# Usage: ./deploy.sh [environment] [action]
# Example: ./deploy.sh production deploy

set -e

# Configuration
NAMESPACE="terminator"
APP_NAME="terminator-mcp-agent"
ENVIRONMENT=${1:-"development"}
ACTION=${2:-"deploy"}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check kubectl
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl is not installed or not in PATH"
        exit 1
    fi
    
    # Check cluster connectivity
    if ! kubectl cluster-info &> /dev/null; then
        log_error "Cannot connect to Kubernetes cluster"
        exit 1
    fi
    
    # Check Windows nodes
    WINDOWS_NODES=$(kubectl get nodes -l kubernetes.io/os=windows --no-headers | wc -l)
    if [ "$WINDOWS_NODES" -eq 0 ]; then
        log_error "No Windows nodes found in the cluster"
        exit 1
    fi
    
    log_success "Prerequisites check passed"
    log_info "Found $WINDOWS_NODES Windows node(s)"
}

# Deploy function
deploy() {
    log_info "Deploying Terminator MCP Agent to $ENVIRONMENT environment..."
    
    # Create namespace if it doesn't exist
    kubectl create namespace $NAMESPACE --dry-run=client -o yaml | kubectl apply -f -
    
    # Apply resources in order
    log_info "Applying ConfigMap..."
    kubectl apply -f configmap.yaml
    
    log_info "Applying RBAC..."
    kubectl apply -f rbac.yaml
    
    log_info "Applying PVC..."
    kubectl apply -f pvc.yaml
    
    log_info "Applying Deployment..."
    kubectl apply -f deployment.yaml
    
    log_info "Applying Services..."
    kubectl apply -f service.yaml
    
    log_info "Applying HPA..."
    kubectl apply -f hpa.yaml
    
    log_info "Applying PDB..."
    kubectl apply -f pdb.yaml
    
    log_info "Applying Network Policy..."
    kubectl apply -f networkpolicy.yaml
    
    log_info "Applying Ingress..."
    kubectl apply -f ingress.yaml
    
    # Wait for deployment to be ready
    log_info "Waiting for deployment to be ready..."
    kubectl wait --for=condition=available --timeout=300s deployment/$APP_NAME -n $NAMESPACE
    
    log_success "Deployment completed successfully!"
}

# Status function
status() {
    log_info "Checking deployment status..."
    
    echo ""
    log_info "Namespace:"
    kubectl get namespace $NAMESPACE
    
    echo ""
    log_info "Pods:"
    kubectl get pods -n $NAMESPACE -o wide
    
    echo ""
    log_info "Services:"
    kubectl get svc -n $NAMESPACE
    
    echo ""
    log_info "Ingress:"
    kubectl get ingress -n $NAMESPACE
    
    echo ""
    log_info "HPA:"
    kubectl get hpa -n $NAMESPACE
    
    echo ""
    log_info "Recent Events:"
    kubectl get events -n $NAMESPACE --sort-by='.lastTimestamp' | tail -10
}

# Test function
test() {
    log_info "Testing deployment..."
    
    # Check if pods are running
    RUNNING_PODS=$(kubectl get pods -n $NAMESPACE -l app=$APP_NAME --field-selector=status.phase=Running --no-headers | wc -l)
    if [ "$RUNNING_PODS" -eq 0 ]; then
        log_error "No running pods found"
        return 1
    fi
    
    log_success "$RUNNING_PODS pod(s) are running"
    
    # Test health endpoint
    log_info "Testing health endpoint..."
    POD_NAME=$(kubectl get pods -n $NAMESPACE -l app=$APP_NAME -o jsonpath='{.items[0].metadata.name}')
    
    if kubectl exec -n $NAMESPACE $POD_NAME -- powershell -c "try { \$r = iwr http://localhost:8080/health -UseBasicParsing; if (\$r.StatusCode -eq 200) { exit 0 } else { exit 1 } } catch { exit 1 }" &> /dev/null; then
        log_success "Health endpoint is responding"
    else
        log_error "Health endpoint is not responding"
        return 1
    fi
    
    # Test service connectivity
    log_info "Testing service connectivity..."
    if kubectl exec -n $NAMESPACE $POD_NAME -- powershell -c "try { \$r = iwr http://terminator-mcp-service:8080/health -UseBasicParsing; if (\$r.StatusCode -eq 200) { exit 0 } else { exit 1 } } catch { exit 1 }" &> /dev/null; then
        log_success "Service connectivity is working"
    else
        log_error "Service connectivity is not working"
        return 1
    fi
    
    log_success "All tests passed!"
}

# Cleanup function
cleanup() {
    log_warning "Cleaning up deployment..."
    
    kubectl delete -f ingress.yaml --ignore-not-found=true
    kubectl delete -f networkpolicy.yaml --ignore-not-found=true
    kubectl delete -f pdb.yaml --ignore-not-found=true
    kubectl delete -f hpa.yaml --ignore-not-found=true
    kubectl delete -f service.yaml --ignore-not-found=true
    kubectl delete -f deployment.yaml --ignore-not-found=true
    kubectl delete -f pvc.yaml --ignore-not-found=true
    kubectl delete -f rbac.yaml --ignore-not-found=true
    kubectl delete -f configmap.yaml --ignore-not-found=true
    
    # Optionally delete namespace
    read -p "Delete namespace '$NAMESPACE'? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        kubectl delete namespace $NAMESPACE --ignore-not-found=true
        log_success "Namespace deleted"
    fi
    
    log_success "Cleanup completed"
}

# Logs function
logs() {
    log_info "Showing logs for $APP_NAME..."
    kubectl logs -n $NAMESPACE -l app=$APP_NAME --tail=100 -f
}

# Scale function
scale() {
    REPLICAS=${3:-3}
    log_info "Scaling deployment to $REPLICAS replicas..."
    kubectl scale deployment/$APP_NAME -n $NAMESPACE --replicas=$REPLICAS
    kubectl wait --for=condition=available --timeout=300s deployment/$APP_NAME -n $NAMESPACE
    log_success "Scaled to $REPLICAS replicas"
}

# Main execution
case $ACTION in
    "deploy")
        check_prerequisites
        deploy
        status
        ;;
    "status")
        status
        ;;
    "test")
        test
        ;;
    "cleanup")
        cleanup
        ;;
    "logs")
        logs
        ;;
    "scale")
        scale
        ;;
    *)
        echo "Usage: $0 [environment] [action]"
        echo "Actions: deploy, status, test, cleanup, logs, scale"
        echo "Example: $0 production deploy"
        exit 1
        ;;
esac
