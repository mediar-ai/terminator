# Terminator MCP Agent - Kubernetes Windows Deployment

This guide explains how to deploy the Terminator MCP Agent on Kubernetes clusters with Windows nodes, providing scalable desktop automation capabilities through the Model Context Protocol (MCP) over HTTP.

## 🎯 Overview

The Kubernetes deployment enables:
- **Scalable Deployment**: Run multiple MCP agent instances with auto-scaling
- **High Availability**: Load balancing and pod distribution across Windows nodes
- **Cloud Native**: Full Kubernetes integration with health checks, monitoring, and RBAC
- **HTTP Transport**: External MCP client access without RDP/VNC requirements
- **Enterprise Ready**: Production-grade configuration with security and observability

## 📋 Prerequisites

### Cluster Requirements
- **Kubernetes Version**: 1.24+ with Windows node support
- **Windows Nodes**: Windows Server 2019/2022 with containerd runtime
- **Container Runtime**: containerd 1.6+ or Docker 20.10+
- **CNI Plugin**: Compatible with Windows (Calico, Flannel, or Azure CNI)
- **Storage**: CSI driver supporting Windows (Azure Disk, AWS EBS, etc.)

### Tools Required
- `kubectl` 1.24+
- `helm` 3.8+ (optional, for Helm deployment)
- `kustomize` 4.5+ (optional, for Kustomize deployment)

### Cluster Setup Verification
```bash
# Check Windows nodes are available
kubectl get nodes -l kubernetes.io/os=windows

# Verify Windows node readiness
kubectl describe nodes -l kubernetes.io/os=windows

# Check storage classes
kubectl get storageclass
```

## 🚀 Quick Start Deployment

### Method 1: Direct kubectl Apply

```bash
# Clone the repository
git clone https://github.com/mediar-ai/terminator.git
cd terminator/k8s

# Deploy all resources
kubectl apply -f namespace.yaml
kubectl apply -f configmap.yaml
kubectl apply -f rbac.yaml
kubectl apply -f pvc.yaml
kubectl apply -f deployment.yaml
kubectl apply -f service.yaml
kubectl apply -f hpa.yaml
kubectl apply -f pdb.yaml
kubectl apply -f networkpolicy.yaml
kubectl apply -f ingress.yaml
```

### Method 2: Kustomize Deployment

```bash
# Deploy using Kustomize
kubectl apply -k k8s/

# Or with custom overlays
kubectl apply -k k8s/overlays/production/
```

### Method 3: Single Command Deployment

```bash
# Deploy everything at once
kubectl apply -f https://raw.githubusercontent.com/mediar-ai/terminator/main/k8s/all-in-one.yaml
```

## 🔍 Verification and Testing

### Check Deployment Status

```bash
# Check namespace
kubectl get namespace terminator

# Check all resources
kubectl get all -n terminator

# Check pod status
kubectl get pods -n terminator -o wide

# Check services
kubectl get svc -n terminator

# Check ingress
kubectl get ingress -n terminator
```

### Verify Pod Health

```bash
# Check pod logs
kubectl logs -n terminator deployment/terminator-mcp-agent

# Check pod events
kubectl describe pods -n terminator

# Check health endpoints
kubectl port-forward -n terminator svc/terminator-mcp-service 8080:8080
curl http://localhost:8080/health
```

### Test MCP Connectivity

```bash
# Port forward to test locally
kubectl port-forward -n terminator svc/terminator-mcp-service 8080:8080

# Test with Python client
python examples/docker_mcp_simple.py --server-url http://localhost:8080/mcp

# Test health endpoint
curl http://localhost:8080/health
```

## 🔧 Configuration and Customization

### Environment Variables

Modify the ConfigMap to customize behavior:

```yaml
# k8s/configmap.yaml
data:
  RUST_LOG: "debug"          # Logging level
  NODE_ENV: "production"     # Environment
  MCP_HOST: "0.0.0.0"       # Bind address
  MCP_PORT: "8080"          # Port number
  MCP_CORS_ENABLED: "true"  # CORS support
```

### Resource Limits

Adjust resources in the Deployment:

```yaml
# k8s/deployment.yaml
resources:
  requests:
    memory: "2Gi"    # Increase for heavy workloads
    cpu: "1000m"     # Increase for better performance
  limits:
    memory: "4Gi"    # Maximum memory
    cpu: "2000m"     # Maximum CPU
```

### Scaling Configuration

Modify HPA for auto-scaling:

```yaml
# k8s/hpa.yaml
spec:
  minReplicas: 3      # Minimum pods
  maxReplicas: 20     # Maximum pods
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 60  # Scale at 60% CPU
```

## 🌐 External Access Configuration

### LoadBalancer Service

For cloud environments with load balancer support:

```bash
# Get external IP
kubectl get svc -n terminator terminator-mcp-service

# Connect using external IP
python examples/docker_mcp_simple.py --server-url http://<EXTERNAL-IP>:8080/mcp
```

### Ingress Configuration

For domain-based access:

```yaml
# Update k8s/ingress.yaml
spec:
  rules:
  - host: terminator-mcp.yourdomain.com
    http:
      paths:
      - path: /mcp
        pathType: Prefix
        backend:
          service:
            name: terminator-mcp-service
            port:
              number: 8080
```

### NodePort Access

For on-premises clusters:

```bash
# Get node IP and port
kubectl get nodes -o wide
kubectl get svc -n terminator terminator-mcp-nodeport

# Connect using node IP and NodePort
python examples/docker_mcp_simple.py --server-url http://<NODE-IP>:30080/mcp
```

## 📊 Monitoring and Observability

### Health Checks

The deployment includes comprehensive health checks:

- **Liveness Probe**: Ensures pods are healthy
- **Readiness Probe**: Controls traffic routing
- **Startup Probe**: Handles slow container startup

### Prometheus Monitoring

Enable Prometheus scraping:

```yaml
# Add to pod annotations
annotations:
  prometheus.io/scrape: "true"
  prometheus.io/port: "8080"
  prometheus.io/path: "/health"
```

### Logging

View logs from all pods:

```bash
# All pods
kubectl logs -n terminator -l app=terminator-mcp-agent

# Specific pod
kubectl logs -n terminator <pod-name>

# Follow logs
kubectl logs -n terminator -f deployment/terminator-mcp-agent
```

## 🔒 Security Configuration

### RBAC

The deployment includes minimal RBAC permissions:

- Read ConfigMaps and Secrets
- Read own Pod information
- Read Services for discovery
- Read Events for debugging

### Network Policies

Control network traffic:

```bash
# Apply network policies
kubectl apply -f k8s/networkpolicy.yaml

# Test connectivity
kubectl exec -n terminator <pod-name> -- curl http://terminator-mcp-service:8080/health
```

### Pod Security

Windows-specific security context:

```yaml
securityContext:
  windowsOptions:
    runAsUserName: "ContainerUser"
  allowPrivilegeEscalation: false
  readOnlyRootFilesystem: false
```

## 🛠️ Management Operations

### Scaling

```bash
# Manual scaling
kubectl scale -n terminator deployment/terminator-mcp-agent --replicas=5

# Check HPA status
kubectl get hpa -n terminator

# View HPA events
kubectl describe hpa -n terminator terminator-mcp-hpa
```

### Updates

```bash
# Update image
kubectl set image -n terminator deployment/terminator-mcp-agent \
  terminator-mcp-agent=ghcr.io/mediar-ai/terminator-mcp-agent-windows:v1.1.0

# Check rollout status
kubectl rollout status -n terminator deployment/terminator-mcp-agent

# Rollback if needed
kubectl rollout undo -n terminator deployment/terminator-mcp-agent
```

### Configuration Updates

```bash
# Update ConfigMap
kubectl patch configmap -n terminator terminator-mcp-config \
  --patch '{"data":{"RUST_LOG":"debug"}}'

# Restart deployment to pick up changes
kubectl rollout restart -n terminator deployment/terminator-mcp-agent
```

## 🔍 Troubleshooting

### Common Issues

**1. Pods Stuck in Pending**
```bash
# Check node selector and tolerations
kubectl describe pods -n terminator

# Verify Windows nodes are available
kubectl get nodes -l kubernetes.io/os=windows

# Check resource availability
kubectl describe nodes -l kubernetes.io/os=windows
```

**2. Image Pull Errors**
```bash
# Check image exists
docker pull ghcr.io/mediar-ai/terminator-mcp-agent-windows:latest

# Verify image pull secrets
kubectl get secrets -n terminator

# Check node access to registry
kubectl debug node/<windows-node-name> -it --image=mcr.microsoft.com/windows/nanoserver:ltsc2022
```

**3. Health Check Failures**
```bash
# Check pod logs
kubectl logs -n terminator <pod-name>

# Test health endpoint directly
kubectl exec -n terminator <pod-name> -- powershell -c "iwr http://localhost:8080/health"

# Check port binding
kubectl exec -n terminator <pod-name> -- netstat -an | findstr :8080
```

**4. Service Connectivity Issues**
```bash
# Test service from another pod
kubectl run test-pod --image=mcr.microsoft.com/windows/nanoserver:ltsc2022 -it --rm
# Inside pod: curl http://terminator-mcp-service.terminator.svc.cluster.local:8080/health

# Check service endpoints
kubectl get endpoints -n terminator terminator-mcp-service

# Verify network policies
kubectl describe networkpolicy -n terminator
```

### Debug Commands

```bash
# Get detailed pod information
kubectl describe pods -n terminator -l app=terminator-mcp-agent

# Check events
kubectl get events -n terminator --sort-by='.lastTimestamp'

# Debug networking
kubectl exec -n terminator <pod-name> -- nslookup terminator-mcp-service

# Check resource usage
kubectl top pods -n terminator
kubectl top nodes -l kubernetes.io/os=windows
```

## 🧪 Testing with MCP Clients

### Python Client Testing

```bash
# Port forward for local testing
kubectl port-forward -n terminator svc/terminator-mcp-service 8080:8080

# Test with simple client
python examples/docker_mcp_simple.py --server-url http://localhost:8080/mcp --demo

# Test with AI client
python examples/docker_mcp_client.py --server-url http://localhost:8080/mcp
```

### External Client Testing

```bash
# Get external access URL
kubectl get ingress -n terminator terminator-mcp-ingress

# Test external access
python examples/docker_mcp_simple.py --server-url https://terminator-mcp.yourdomain.com/mcp
```

## 🚀 Production Deployment Checklist

- [ ] Windows nodes are properly configured and ready
- [ ] Storage class supports Windows containers
- [ ] Image registry is accessible from Windows nodes
- [ ] Resource limits are appropriate for workload
- [ ] Health checks are configured and working
- [ ] Monitoring and logging are set up
- [ ] Network policies are applied
- [ ] RBAC permissions are minimal and appropriate
- [ ] TLS certificates are configured (if using HTTPS)
- [ ] Backup and disaster recovery plans are in place

## 📈 Performance Tuning

### Resource Optimization

```yaml
# Optimized resource configuration
resources:
  requests:
    memory: "1.5Gi"
    cpu: "750m"
  limits:
    memory: "3Gi"
    cpu: "1500m"
```

### Node Affinity

```yaml
# Prefer specific Windows node types
affinity:
  nodeAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
    - weight: 100
      preference:
        matchExpressions:
        - key: node.kubernetes.io/instance-type
          operator: In
          values:
          - Standard_D4s_v3  # Azure example
```

This Kubernetes deployment provides a production-ready, scalable solution for running the Terminator MCP Agent on Windows nodes while maintaining all the HTTP transport and terminal-only operation benefits of the Docker implementation.
