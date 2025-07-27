# Terminator MCP Agent - Docker Windows Setup

This guide explains how to run the Terminator MCP Agent in a Docker Windows container, enabling desktop automation capabilities through the Model Context Protocol (MCP) over HTTP.

## 🎯 Overview

The Docker Windows setup allows you to:
- Run Terminator MCP Agent in an isolated Windows container
- Access desktop automation via HTTP (no RDP/VNC required)
- Connect from external MCP clients using the HTTP transport
- Maintain security through containerization

## 📋 Prerequisites

### System Requirements
- **Windows Host**: Windows 10/11 or Windows Server 2019/2022
- **Docker Desktop**: Latest version with Windows container support
- **Memory**: At least 4GB RAM (2GB allocated to container)
- **Storage**: 10GB free space for container images

### Software Dependencies
- Docker Desktop for Windows
- PowerShell 5.1 or later
- Python 3.8+ (for client examples)

## 🚀 Quick Start

### 1. Switch to Windows Containers

Ensure Docker Desktop is running Windows containers:

```powershell
# Check current mode
docker version

# Switch to Windows containers if needed
& "C:\Program Files\Docker\Docker\DockerCli.exe" -SwitchDaemon
```

### 2. Build and Run the Container

```powershell
# Clone the repository (if not already done)
git clone https://github.com/mediar-ai/terminator.git
cd terminator

# Build and start the container using Docker Compose
cd docker
docker-compose -f docker-compose.windows.yml up --build

# Or run in detached mode
docker-compose -f docker-compose.windows.yml up --build -d
```

### 3. Verify the Setup

```powershell
# Run the test script
.\test-docker-setup.ps1

# Or test manually
curl http://localhost:8080/health
```

Expected response:
```json
{"status":"ok"}
```

## 🔧 Configuration

### Environment Variables

Customize the container behavior in `docker-compose.windows.yml`:

```yaml
environment:
  - RUST_LOG=info          # Logging level (debug, info, warn, error)
  - NODE_ENV=production     # Node.js environment
```

### Port Configuration

Change the exposed port by modifying the docker-compose file:

```yaml
ports:
  - "9000:8080"  # Maps host port 9000 to container port 8080
```

### Resource Limits

Adjust memory and CPU limits:

```yaml
deploy:
  resources:
    limits:
      memory: 4G      # Increase if needed
      cpus: '2.0'     # CPU limit
    reservations:
      memory: 2G      # Minimum memory
```

## 🧪 Testing the Setup

### Automated Testing

Use the provided test scripts:

```powershell
# PowerShell test script (Windows-specific checks)
.\docker\test-docker-setup.ps1

# Verbose output
.\docker\test-docker-setup.ps1 -Verbose

# Test different port
.\docker\test-docker-setup.ps1 -ContainerUrl "http://localhost:9000"
```

```bash
# Python validation script (cross-platform)
pip install requests
python docker/validate-setup.py

# Test different URL
python docker/validate-setup.py --container-url http://localhost:9000
```

### Manual Testing

1. **Health Check**:
   ```powershell
   curl http://localhost:8080/health
   ```

2. **Container Status**:
   ```powershell
   docker ps --filter "name=terminator-mcp-windows"
   ```

3. **Container Logs**:
   ```powershell
   docker logs terminator-mcp-windows
   ```

### Python Client Testing

Test with the provided Python clients:

```bash
# Install dependencies
pip install mcp

# Simple test (no AI required)
python examples/docker_mcp_simple.py --demo

# Interactive mode
python examples/docker_mcp_simple.py

# AI-powered client (requires ANTHROPIC_API_KEY)
pip install anthropic python-dotenv
python examples/docker_mcp_client.py
```

## 🔌 Connecting MCP Clients

### HTTP Transport Configuration

The container exposes the MCP server via HTTP at:
- **MCP Endpoint**: `http://localhost:8080/mcp`
- **Health Check**: `http://localhost:8080/health`

### Client Configuration Examples

**VS Code/Cursor MCP Settings**:
```json
{
  "mcpServers": {
    "terminator-docker": {
      "command": "python",
      "args": ["examples/docker_mcp_client.py", "--server-url", "http://localhost:8080/mcp"]
    }
  }
}
```

**Direct HTTP Client**:
```python
from mcp.client.streamable_http import streamablehttp_client

# Connect to Docker container
transport = await streamablehttp_client("http://localhost:8080/mcp")
```

## 🛠️ Development and Customization

### Building from Source

To build the MCP agent from source within the container:

1. Create a development Dockerfile:
   ```dockerfile
   # Add Rust toolchain
   RUN powershell -Command "Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile 'rustup-init.exe'; ./rustup-init.exe -y"
   
   # Copy source code
   COPY . .
   
   # Build the agent
   RUN cargo build --release -p terminator-mcp-agent
   ```

2. Use the local binary instead of npm package

### Custom Configuration

Create a custom startup script:

```powershell
# custom-start.ps1
Write-Host "🚀 Starting custom Terminator MCP Agent..."

# Set custom environment variables
$env:RUST_LOG = "debug"
$env:CUSTOM_CONFIG = "enabled"

# Start with custom parameters
node index.js -t http --host 0.0.0.0 --port 8080 --cors
```

## 🔍 Troubleshooting

### Common Issues

**1. Container Won't Start**
```powershell
# Check Docker mode
docker version
# Look for "OS/Arch: windows/amd64" in Server section

# Check container logs
docker logs terminator-mcp-windows

# Rebuild container
docker-compose -f docker-compose.windows.yml up --build --force-recreate
```

**2. MCP Agent Not Responding**
```powershell
# Test from inside container
docker exec -it terminator-mcp-windows powershell
curl http://localhost:8080/health

# Check if port is bound
netstat -an | findstr :8080
```

**3. Connection Refused from Client**
- Ensure container is running: `docker ps`
- Check port mapping: `docker port terminator-mcp-windows`
- Verify Windows Firewall settings
- Test with `curl` or browser first

**4. Windows UI Automation Issues**
- Container runs on Windows Server Core (headless)
- Limited desktop applications available
- Some UI automation features may not work without full desktop

### Performance Issues

**High Memory Usage**:
- Increase container memory limit
- Monitor with: `docker stats terminator-mcp-windows`
- Check for memory leaks in logs

**Slow Response Times**:
- Ensure SSD storage for Docker
- Increase CPU allocation
- Check network latency

### Debugging

**Enable Debug Logging**:
```yaml
environment:
  - RUST_LOG=debug
  - NODE_ENV=development
```

**Access Container Shell**:
```powershell
docker exec -it terminator-mcp-windows powershell
```

**Monitor Resource Usage**:
```powershell
docker stats terminator-mcp-windows
```

## 🔒 Security Considerations

### Network Security
- Container exposes port 8080 by default
- CORS is enabled for web access
- Consider using reverse proxy for production
- Implement authentication if needed

### Container Security
- Runs with default Windows container security
- No privileged access required
- Isolated from host system
- Consider read-only filesystem for production

### Production Deployment
- Use HTTPS with proper certificates
- Implement rate limiting
- Monitor container logs
- Regular security updates

## 📚 Additional Resources

- [Terminator MCP Agent Documentation](terminator-mcp-agent/README.md)
- [Docker Windows Containers Guide](https://docs.docker.com/desktop/windows/)
- [Model Context Protocol Specification](https://modelcontextprotocol.io/)
- [Example Python Clients](examples/)

## 🆘 Getting Help

If you encounter issues:

1. **Check the logs**: `docker logs terminator-mcp-windows`
2. **Run the test script**: `.\docker\test-docker-setup.ps1 -Verbose`
3. **Test with simple client**: `python examples/docker_mcp_simple.py --demo`
4. **Open an issue** on GitHub with:
   - Container logs
   - Test script output
   - System information (`docker version`, `docker info`)

## 🎉 Success Criteria

Your Docker Windows setup is working correctly when:

✅ Container starts without errors  
✅ Health check returns `{"status":"ok"}`  
✅ MCP endpoint responds at `/mcp`  
✅ Python client can connect and list tools  
✅ Basic automation commands work  

**Ready to automate!** 🤖
