# Terminator MCP Agent - Docker Windows Setup

This directory contains the Docker configuration for running the Terminator MCP Agent in a Windows container.

## Prerequisites

1. **Windows Host**: You need a Windows machine with Docker Desktop configured for Windows containers
2. **Docker Desktop**: Install Docker Desktop and switch to Windows containers mode
3. **Windows Container Support**: Ensure your Windows version supports Windows containers

## Quick Start

### Option 1: Using Docker Compose (Recommended)

```powershell
# Navigate to the docker directory
cd docker

# Build and start the container
docker-compose -f docker-compose.windows.yml up --build

# Or run in detached mode
docker-compose -f docker-compose.windows.yml up --build -d
```

### Option 2: Using Docker directly

```powershell
# Build the image
docker build -t terminator-mcp-windows -f docker/Dockerfile.windows .

# Run the container
docker run -d --name terminator-mcp-windows -p 8080:8080 terminator-mcp-windows
```

## Accessing the MCP Agent

Once the container is running, the MCP agent will be available at:

- **MCP Endpoint**: `http://localhost:8080/mcp`
- **Health Check**: `http://localhost:8080/health`

## Testing the Setup

### 1. Health Check

```powershell
# Test the health endpoint
curl http://localhost:8080/health
```

Expected response:
```json
{"status":"ok"}
```

### 2. Using Python MCP Client

See the example Python client in `examples/docker_mcp_client.py` for connecting to the containerized MCP agent.

## Configuration

### Environment Variables

You can customize the container behavior using environment variables:

- `RUST_LOG`: Set logging level (default: `info`)
- `NODE_ENV`: Node.js environment (default: `production`)

### Port Configuration

The default port is 8080, but you can change it by modifying the docker-compose.yml file:

```yaml
ports:
  - "9000:8080"  # Maps host port 9000 to container port 8080
```

## Troubleshooting

### Container Won't Start

1. **Check Docker is in Windows container mode**:
   ```powershell
   docker version
   ```
   Look for "OS/Arch: windows/amd64" in the Server section.

2. **Switch to Windows containers** (if needed):
   ```powershell
   & "C:\Program Files\Docker\Docker\DockerCli.exe" -SwitchDaemon
   ```

### MCP Agent Not Responding

1. **Check container logs**:
   ```powershell
   docker logs terminator-mcp-windows
   ```

2. **Check container health**:
   ```powershell
   docker ps
   ```
   Look for "healthy" status.

3. **Test from inside container**:
   ```powershell
   docker exec -it terminator-mcp-windows powershell
   # Inside container:
   curl http://localhost:8080/health
   ```

### Windows UI Automation Issues

The container runs on Windows Server Core, which has limited UI capabilities compared to full Windows. Some limitations:

- No desktop environment (headless)
- Limited application support
- Some UI automation features may not work

For full UI automation capabilities, consider using the agent on a full Windows installation.

## Building from Source

If you want to build the MCP agent from source within the container:

1. Modify the Dockerfile to include Rust toolchain
2. Copy the entire source code
3. Build the Rust binary during container build

See `Dockerfile.windows.dev` for a development version with full build capabilities.

## Security Considerations

- The container runs with default Windows container security
- CORS is enabled for web access - restrict in production
- Consider using HTTPS in production environments
- Limit network access as needed

## Performance

- Default memory limit: 2GB (configurable in docker-compose.yml)
- CPU usage depends on automation workload
- Consider SSD storage for better I/O performance

## Support

For issues specific to Docker Windows setup:
1. Check the container logs
2. Verify Windows container compatibility
3. Test with the provided example clients
4. Open an issue on the GitHub repository with container logs
