# Remote UI Automation - Deployment Guide

## Table of Contents
1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Deployment Scenarios](#deployment-scenarios)
4. [Setup Instructions](#setup-instructions)
5. [Configuration](#configuration)
6. [Security](#security)
7. [Monitoring](#monitoring)
8. [Troubleshooting](#troubleshooting)

## Overview

The Remote UI Automation system enables Windows UI automation across different environments:
- **Local VMs** (Hyper-V, VMware, VirtualBox)
- **Azure VMs** (with various connection methods)
- **Remote machines** (via HTTP API)

### Key Features
- Clean abstraction layer preventing code duplication
- Support for multiple hypervisors and cloud providers
- Secure API with optional authentication
- Automatic VM management (start/stop/restart)
- Agent deployment automation

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Client App    │────▶│  VM Connector    │────▶│    Target VM    │
│                 │     │  (Abstraction)   │     │                 │
└─────────────────┘     └──────────────────┘     └─────────────────┘
                               │                          │
                               ▼                          ▼
                        ┌─────────────┐           ┌──────────────┐
                        │  Local VM   │           │ Remote Agent │
                        │  Connector  │           │   (HTTP)     │
                        └─────────────┘           └──────────────┘
                               │                          │
                               ▼                          ▼
                        ┌─────────────┐           ┌──────────────┐
                        │  Azure VM   │           │ UI Automation│
                        │  Connector  │           │   Engine     │
                        └─────────────┘           └──────────────┘
```

### Components

1. **VM Connector** (`vm_connector.rs`)
   - Abstract interface for VM operations
   - Implementations for different VM types
   - Connection management

2. **Remote Server** (`remote_server.rs`)
   - HTTP API server
   - Executes UI automation commands
   - Session management

3. **Remote Client** (`remote_client.rs`)
   - Client library for remote connections
   - Request/response handling
   - Retry logic

4. **UI Automation** (`remote_automation.rs`)
   - Abstract UI automation interface
   - Local and remote implementations
   - Async/sync bridge

## Deployment Scenarios

### Scenario 1: Local Hyper-V VM

```powershell
# Set environment variables
$env:VM_TYPE = "local"
$env:HYPERVISOR = "hyperv"
$env:VM_NAME = "Windows11-Dev"

# Run the automation
cargo run --example remote_automation_example
```

### Scenario 2: Azure VM with Remote Agent

```powershell
# Deploy agent to Azure VM
az vm extension set `
  --resource-group AVD-TERMINATOR-RG `
  --vm-name mcp-test-vm `
  --name CustomScriptExtension `
  --publisher Microsoft.Compute `
  --settings '{"fileUris": ["https://storage.blob.core.windows.net/scripts/deploy-agent.ps1"]}' `
  --protected-settings '{"commandToExecute": "powershell -ExecutionPolicy Unrestricted -File deploy-agent.ps1"}'

# Set environment variables
$env:VM_TYPE = "azure"
$env:AZURE_SUBSCRIPTION_ID = "5c0a60d0-92cf-47ca-9430-b462bc2fe194"
$env:AZURE_RESOURCE_GROUP = "AVD-TERMINATOR-RG"
$env:AZURE_VM_NAME = "mcp-test-vm"

# Run the automation
cargo run --example remote_automation_example
```

### Scenario 3: Remote HTTP Agent

```powershell
# On the remote machine, start the agent
cd C:\remote-agent
.\remote-ui-agent.exe --port 8080 --api-key "secret-key"

# On the client machine
$env:VM_TYPE = "remote"
$env:REMOTE_HOST = "192.168.1.100"
$env:REMOTE_PORT = "8080"
$env:REMOTE_API_KEY = "secret-key"

cargo run --example remote_automation_example
```

## Setup Instructions

### Prerequisites

1. **Windows Machine** (for UI automation)
2. **Rust** (latest stable version)
3. **Azure CLI** (for Azure VMs)
4. **PowerShell** (for Hyper-V operations)

### Installation

1. **Clone the repository**
```bash
git clone https://github.com/your-org/terminator.git
cd terminator
git checkout feature/remote-ui-automation
```

2. **Build the project**
```bash
cargo build --release
```

3. **Build the remote agent**
```bash
cd terminator-mcp-agent
cargo build --release --bin remote-ui-agent
```

### Deploy Agent to Target VM

#### Local VM (Hyper-V)
```powershell
# Get VM IP
$vmIP = (Get-VM -Name "Windows11-Dev" | Get-VMNetworkAdapter).IPAddresses[0]

# Copy agent to VM
Copy-Item -Path ".\target\release\remote-ui-agent.exe" `
          -Destination "\\$vmIP\c$\remote-agent\" `
          -Force

# Start agent via PowerShell remoting
Invoke-Command -ComputerName $vmIP -ScriptBlock {
    Start-Process "C:\remote-agent\remote-ui-agent.exe" -ArgumentList "--port", "8080"
}
```

#### Azure VM
```bash
# Upload agent to Azure Storage
az storage blob upload \
  --account-name mystorageaccount \
  --container-name agents \
  --name remote-ui-agent.exe \
  --file ./target/release/remote-ui-agent.exe

# Deploy via Custom Script Extension
az vm extension set \
  --resource-group AVD-TERMINATOR-RG \
  --vm-name mcp-test-vm \
  --name CustomScriptExtension \
  --publisher Microsoft.Compute \
  --settings '{"fileUris": ["https://mystorageaccount.blob.core.windows.net/agents/remote-ui-agent.exe"]}' \
  --protected-settings '{"commandToExecute": "remote-ui-agent.exe --port 8080"}'
```

## Configuration

### Configuration File (`config/remote_automation.toml`)

```toml
[default]
type = "local"
timeout_ms = 30000
retry_count = 3

[local.hyperv]
hypervisor = "hyperv"
vm_name = "Windows11-Dev"
connection_method = "vmconnect"

[azure.production]
subscription_id = "${AZURE_SUBSCRIPTION_ID}"
resource_group = "PROD-RG"
vm_name = "prod-automation-vm"
connection_method = { type = "bastion", bastion_host = "prod-bastion" }

[security]
use_tls = true
verify_certificates = true
api_key_header = "X-API-Key"
```

### Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `VM_TYPE` | Type of VM connection | `local`, `azure`, `remote` |
| `HYPERVISOR` | Local hypervisor type | `hyperv`, `vmware`, `virtualbox` |
| `VM_NAME` | Local VM name | `Windows11-Dev` |
| `AZURE_SUBSCRIPTION_ID` | Azure subscription | `5c0a60d0-92cf-47ca-9430-b462bc2fe194` |
| `AZURE_RESOURCE_GROUP` | Azure resource group | `AVD-TERMINATOR-RG` |
| `AZURE_VM_NAME` | Azure VM name | `mcp-test-vm` |
| `REMOTE_HOST` | Remote agent host | `192.168.1.100` |
| `REMOTE_PORT` | Remote agent port | `8080` |
| `REMOTE_API_KEY` | API key for authentication | `secret-key` |

## Security

### Authentication Methods

1. **API Key Authentication**
   - Set `REMOTE_API_KEY` environment variable
   - Pass in `X-API-Key` header

2. **Azure AD Authentication** (for Azure VMs)
   - Use Azure CLI authentication
   - Service Principal support

3. **Windows Authentication** (for local VMs)
   - Integrated Windows authentication
   - Credential pass-through

### Best Practices

1. **Use TLS/HTTPS** in production
2. **Rotate API keys** regularly
3. **Implement IP whitelisting**
4. **Enable audit logging**
5. **Use Azure Key Vault** for secrets
6. **Apply principle of least privilege**

## Monitoring

### Health Checks

```bash
# Check agent health
curl http://remote-host:8080/health

# Check Azure VM status
az vm get-instance-view \
  --resource-group AVD-TERMINATOR-RG \
  --name mcp-test-vm \
  --query "instanceView.statuses[1]"
```

### Logging

Configure logging in the agent:
```toml
[logging]
level = "info"
file = "C:\\logs\\remote-agent.log"
max_size_mb = 100
max_files = 10
```

### Metrics

Monitor these key metrics:
- Request latency
- Success/failure rate
- VM availability
- Agent memory usage
- Network throughput

## Troubleshooting

### Common Issues

#### 1. Cannot connect to local VM
```powershell
# Check VM network adapter
Get-VM "Windows11-Dev" | Get-VMNetworkAdapter

# Enable VM integration services
Enable-VMIntegrationService -VMName "Windows11-Dev" -Name "Guest Service Interface"
```

#### 2. Azure VM connection timeout
```bash
# Check NSG rules
az network nsg rule list \
  --resource-group AVD-TERMINATOR-RG \
  --nsg-name vm-nsg

# Add inbound rule for agent port
az network nsg rule create \
  --resource-group AVD-TERMINATOR-RG \
  --nsg-name vm-nsg \
  --name AllowRemoteAgent \
  --priority 1000 \
  --source-address-prefixes "*" \
  --destination-port-ranges 8080 \
  --access Allow \
  --protocol Tcp
```

#### 3. Agent not starting
```powershell
# Check Windows Defender Firewall
New-NetFirewallRule -DisplayName "Remote UI Agent" `
                    -Direction Inbound `
                    -Protocol TCP `
                    -LocalPort 8080 `
                    -Action Allow

# Check if port is in use
netstat -an | findstr :8080
```

### Debug Mode

Enable debug logging:
```bash
# Set environment variable
export RUST_LOG=debug

# Run with verbose output
cargo run --example remote_automation_example -- --verbose
```

### Performance Tuning

1. **Connection Pooling**
```toml
[performance]
connection_pool_size = 10
keepalive_interval_ms = 5000
```

2. **Retry Configuration**
```toml
[retry]
max_attempts = 3
backoff_ms = 1000
max_backoff_ms = 30000
```

3. **Timeout Settings**
```toml
[timeouts]
connect_ms = 5000
request_ms = 30000
total_ms = 300000
```

## Testing

### Run Unit Tests
```bash
cargo test
```

### Run Integration Tests
```bash
# Requires VMs to be available
cargo test --ignored
```

### Run Specific Test Scenarios
```bash
# Test local Hyper-V
cargo test --ignored test_local_hyperv_connection

# Test Azure VM
cargo test --ignored test_azure_vm_connection

# Test full automation flow
cargo test --ignored test_full_automation_flow
```

## CI/CD Pipeline

### GitHub Actions Example
```yaml
name: Remote UI Automation Tests

on:
  push:
    branches: [ feature/remote-ui-automation ]

jobs:
  test:
    runs-on: windows-latest
    steps:
    - uses: actions/checkout@v2

    - name: Setup Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable

    - name: Build
      run: cargo build --release

    - name: Run Tests
      run: cargo test

    - name: Deploy Agent to Test VM
      env:
        AZURE_CREDENTIALS: ${{ secrets.AZURE_CREDENTIALS }}
      run: |
        az login --service-principal
        ./scripts/deploy-agent.ps1

    - name: Run Integration Tests
      run: cargo test --ignored
```

## Support

For issues and questions:
- GitHub Issues: https://github.com/your-org/terminator/issues
- Documentation: https://docs.your-org.com/remote-automation
- Email: support@your-org.com