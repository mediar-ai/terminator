# Azure VM Deployment Guide for RDP Testing

## Quick Deployment Steps

### 1. Create Azure VM

**Option A: Windows VM (Recommended for Initial Testing)**
```bash
# Create resource group
az group create --name terminator-rdp-test --location eastus

# Create Windows Server VM
az vm create \
  --resource-group terminator-rdp-test \
  --name terminator-rdp-vm \
  --image Win2022Datacenter \
  --admin-username azureuser \
  --admin-password 'YourSecurePassword123!' \
  --size Standard_D2s_v3 \
  --public-ip-sku Standard

# Open ports for RDP (3389) and MCP HTTP (3000)
az vm open-port --resource-group terminator-rdp-test --name terminator-rdp-vm --port 3389 --priority 1000
az vm open-port --resource-group terminator-rdp-test --name terminator-rdp-vm --port 3000 --priority 1001
```

**Option B: Ubuntu VM (For Production)**
```bash
# Create Ubuntu VM
az vm create \
  --resource-group terminator-rdp-test \
  --name terminator-rdp-vm \
  --image UbuntuLTS \
  --admin-username azureuser \
  --generate-ssh-keys \
  --size Standard_D2s_v3 \
  --public-ip-sku Standard

# Open ports
az vm open-port --resource-group terminator-rdp-test --name terminator-rdp-vm --port 3389 --priority 1000
az vm open-port --resource-group terminator-rdp-test --name terminator-rdp-vm --port 3000 --priority 1001
```

### 2. Get VM Public IP
```bash
az vm show --resource-group terminator-rdp-test --name terminator-rdp-vm --show-details --query publicIps -o tsv
```

### 3. Deploy on Windows VM

**Connect via RDP:**
```bash
# From local machine
mstsc /v:<VM_PUBLIC_IP>
# Login with azureuser / YourSecurePassword123!
```

**Install Build Tools on VM:**
1. Install Rust: https://rustup.rs/
2. Install Visual Studio Build Tools: https://visualstudio.microsoft.com/downloads/
3. Install CMake: https://cmake.org/download/
4. Install Git: https://git-scm.com/download/win

**Build and Run:**
```powershell
# Clone repo
git clone https://github.com/mediar-ai/terminator.git
cd terminator
git checkout feature/rdp-server-integration

# Build with RDP feature
cargo build --release --features rdp

# Run server
.\target\release\terminator-mcp-agent.exe -t http --rdp --rdp-bind 0.0.0.0:3389 --host 0.0.0.0 --port 3000
```

### 4. Deploy on Ubuntu VM

**Connect via SSH:**
```bash
ssh azureuser@<VM_PUBLIC_IP>
```

**Install Dependencies:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install build dependencies
sudo apt-get update
sudo apt-get install -y build-essential cmake pkg-config libssl-dev git

# Install X11 dependencies (required for desktop automation)
sudo apt-get install -y libx11-dev libxrandr-dev libxfixes-dev
```

**Build and Run:**
```bash
# Clone repo
git clone https://github.com/mediar-ai/terminator.git
cd terminator
git checkout feature/rdp-server-integration

# Build with RDP feature
cargo build --release --features rdp

# Run server
./target/release/terminator-mcp-agent -t http --rdp --rdp-bind 0.0.0.0:3389 --host 0.0.0.0 --port 3000
```

## Testing from Local Machine

### Test 1: HTTP MCP Server
```bash
# Test MCP server is accessible
curl http://<VM_PUBLIC_IP>:3000/health

# Expected response:
# {"status":"healthy","extension_bridge":{...},"automation":{...}}
```

### Test 2: RDP Connection Attempt
```bash
# Windows: Use Microsoft Remote Desktop
mstsc /v:<VM_PUBLIC_IP>:3389

# macOS: Use Microsoft Remote Desktop app from App Store
# Linux: Use rdesktop or freerdp
rdesktop <VM_PUBLIC_IP>:3389
# or
xfreerdp /v:<VM_PUBLIC_IP>:3389 /u:test /p:test
```

**Expected Result (Current MVP)**:
- ✅ TCP connection succeeds
- ✅ Logs show "RDP client connected from X.X.X.X"
- ✅ Logs show "RDP handshake completed"
- ✅ Logs show "Screen capture successful: monitor 0 (1920x1080 pixels, ...)"
- ❌ RDP client disconnects (missing full protocol implementation)
- ❌ No screen visible in RDP client (bitmap updates not implemented)

### Test 3: Monitor Server Logs
```bash
# On VM, check logs in real-time
# Look for:
# - "RDP server listening on 0.0.0.0:3389"
# - "RDP client connected from X.X.X.X"
# - "RDP handshake completed for X.X.X.X"
# - "Screen capture successful: monitor 0..."
```

## Firewall Configuration

If connection fails, verify firewall rules:

### Azure Network Security Group
```bash
# List current rules
az network nsg list --resource-group terminator-rdp-test --output table

# Ensure RDP and HTTP ports are open
az network nsg rule list --resource-group terminator-rdp-test --nsg-name terminator-rdp-vmNSG --output table
```

### Windows VM Firewall
```powershell
# On Windows VM, check firewall
netsh advfirewall firewall show rule name=all | findstr 3389
netsh advfirewall firewall show rule name=all | findstr 3000

# Add rules if missing
netsh advfirewall firewall add rule name="RDP Server" dir=in action=allow protocol=TCP localport=3389
netsh advfirewall firewall add rule name="MCP HTTP" dir=in action=allow protocol=TCP localport=3000
```

### Ubuntu VM Firewall
```bash
# Check UFW status
sudo ufw status

# Allow ports
sudo ufw allow 3389/tcp
sudo ufw allow 3000/tcp
```

## Monitoring and Debugging

### Check if Ports are Listening
**Windows:**
```powershell
netstat -an | findstr 3389
netstat -an | findstr 3000
```

**Ubuntu:**
```bash
ss -tuln | grep 3389
ss -tuln | grep 3000
```

### View Real-Time Logs
The terminator-mcp-agent outputs logs to stderr. You should see:
```
RDP server listening on 0.0.0.0:3389 (fps: 15, input control: true)
Streamable HTTP server running on http://0.0.0.0:3000
```

When client connects:
```
RDP client connected from 1.2.3.4
Handling RDP client from 1.2.3.4
Received X.224 Connection Request
Sent X.224 Connection Confirm
Received MCS Connect Initial
Sent MCS Connect Response
RDP handshake completed for 1.2.3.4
Screen capture successful: monitor 0 (1920x1080 pixels, 8294400 bytes)
```

## Cost Optimization

### Stop VM When Not Testing
```bash
# Stop VM (keeps disk, no compute charges)
az vm deallocate --resource-group terminator-rdp-test --name terminator-rdp-vm

# Start VM when needed
az vm start --resource-group terminator-rdp-test --name terminator-rdp-vm
```

### Delete Everything When Done
```bash
# Delete entire resource group
az group delete --name terminator-rdp-test --yes --no-wait
```

## Alternative: Local Testing

If you don't want to use Azure, test locally:

```bash
# Build and run locally
cargo build --release --features rdp
./target/release/terminator-mcp-agent -t http --rdp --rdp-bind 127.0.0.1:3389 --host 127.0.0.1 --port 3000

# Connect from same machine
# Windows: mstsc /v:localhost:3389
# macOS/Linux: Use local RDP client pointing to localhost:3389
```

## Troubleshooting

### "RDP server error: Failed to bind RDP server"
- Port 3389 may already be in use (Windows RDP service)
- Solution: Use different port `--rdp-bind 0.0.0.0:3390`
- Or stop Windows RDP: `net stop termservice` (requires admin)

### "Failed to initialize desktop wrapper"
- UI Automation not available
- On Linux: Install X11 dependencies
- On Windows: Should work out of box

### RDP Client Shows "Connection Failed"
- Check firewall rules (Azure NSG + VM firewall)
- Verify server is listening: `netstat -an | findstr 3389`
- Check logs for "RDP server listening" message

### CMake Not Found
- Build error: "Missing dependency: cmake"
- Install CMake before building with --features rdp
- Windows: https://cmake.org/download/
- Ubuntu: `sudo apt-get install cmake`
- macOS: `brew install cmake`

## Success Criteria

### MVP Testing (Current State)
- ✅ HTTP MCP server responds to /health
- ✅ RDP TCP connection established
- ✅ Logs show handshake completed
- ✅ Logs show screen capture working
- ✅ Server doesn't crash

### Full Implementation (Future)
- ✅ RDP client stays connected
- ✅ Screen visible in RDP client
- ✅ Mouse control works
- ✅ Keyboard control works
- ✅ Multiple clients can connect

## Next Steps After Deployment

1. **Verify Infrastructure**: Confirm HTTP and RDP servers are running
2. **Document Logs**: Capture what's working vs what's failing
3. **Decide on Path Forward**:
   - Complete full RDP implementation (~2-3 days)
   - Switch to VNC protocol (~4-6 hours)
   - Defer remote viewing feature

See `RDP_IMPLEMENTATION_STATUS.md` for detailed technical analysis.
