---
name: remote-mcp
description: Control remote machines via MCP using terminator CLI. Auto-activates when user says "remote MCP", "connect to machine", "execute on remote", or wants to run commands on remote VMs.
allowed-tools: Bash, Read, Write
---

# Remote MCP Control Skill

Control remote MCP-enabled machines using the terminator CLI.

## Prerequisites

1. **Terminator CLI** installed: `cargo install terminator-cli` or `npx @mediar-ai/cli`
2. **MCP endpoint URL** from target machine (e.g., `http://<IP>:8080/mcp`)
3. **Auth token** (if required): Set via `MCP_AUTH_TOKEN` env var

---

## Basic Usage

```bash
# Set auth token if required
export MCP_AUTH_TOKEN="your-token-here"

# Execute a command on remote machine
terminator mcp exec --url "http://<IP>:8080/mcp" run_command '{
  "run": "node -e \"console.log(JSON.stringify({cwd: process.cwd()}))\"",
  "timeout_seconds": 10
}'
```

---

## Common Commands

### 1. Run Shell Command

```bash
terminator mcp exec --url "http://<IP>:8080/mcp" run_command '{
  "run": "tasklist /FI \"IMAGENAME eq chrome.exe\" /FO TABLE",
  "timeout_seconds": 10
}'
```

### 2. Run JavaScript

```bash
terminator mcp exec --url "http://<IP>:8080/mcp" run_command '{
  "run": "console.log(JSON.stringify({user: process.env.USERNAME, cwd: process.cwd()}))",
  "engine": "javascript",
  "timeout_seconds": 10
}'
```

### 3. Check File Exists

```bash
terminator mcp exec --url "http://<IP>:8080/mcp" run_command '{
  "run": "dir \"C:\\Users\\vmuser\\Desktop\"",
  "timeout_seconds": 5
}'
```

### 4. Get Desktop UI Tree

```bash
terminator mcp exec --url "http://<IP>:8080/mcp" get_desktop_elements '{
  "depth": 1
}'
```

### 5. Get Process-Specific UI Tree

```bash
# Chrome UI tree
terminator mcp exec --url "http://<IP>:8080/mcp" get_desktop_elements '{
  "selector": "process:chrome",
  "depth": 3
}'
```

### 6. Take Screenshot

```bash
terminator mcp exec --url "http://<IP>:8080/mcp" take_screenshot '{}'
```

### 7. Run PowerShell Script

```bash
terminator mcp exec --url "http://<IP>:8080/mcp" run_command '{
  "run": "powershell -Command \"Get-Process | Select-Object ProcessName, Id | ConvertTo-Json\"",
  "timeout_seconds": 15
}'
```

---

## MCP Tool Reference

| Tool | Description | Args |
|------|-------------|------|
| `run_command` | Execute shell/JS/Python | `{"run": "...", "engine": "javascript", "timeout_seconds": N}` |
| `get_desktop_elements` | Get UI tree | `{"selector": "...", "depth": N}` |
| `take_screenshot` | Capture screen | `{"selector": "..."}` (optional) |
| `click_element` | Click UI element | `{"selector": "process:App >> role:Button && name:OK"}` |
| `type_text` | Type into element | `{"selector": "...", "text": "..."}` |
| `press_key` | Press keyboard key | `{"key": "enter"}` or `{"key": "ctrl+a"}` |
| `wait_for_element` | Wait for element | `{"selector": "...", "timeout_seconds": N}` |
| `open_application` | Launch app | `{"path": "C:\\path\\to\\app.exe"}` |

---

## run_command Args

| Arg | Description |
|-----|-------------|
| `run` | Command/code to execute (required unless script_file used) |
| `script_file` | Path to script file to execute |
| `engine` | Script engine: `javascript`, `typescript`, `python`, `node`, `bun` |
| `shell` | Shell to use: `bash`, `sh`, `cmd`, `powershell`, `pwsh` |
| `env` | Environment variables as JSON object |
| `timeout_seconds` | Execution timeout |

---

## Selector Syntax

**Operators:**
- `>>` - Chain/descendant (scope into process, then find element)
- `&&` - AND (element must match ALL conditions)
- `||` - OR (element matches ANY condition)
- `!` - NOT (element must NOT match)
- `()` - Grouping

**Examples:**
```
# Scope to process, then find button with specific name
process:chrome >> role:Button && name:Submit

# Window matching both role AND name
role:Window && name:Settings

# Multiple conditions
process:notepad >> role:Edit && visible:true

# OR conditions
role:Button && (name:OK || name:Cancel)

# Complex chain
process:explorer >> role:Window && name:Downloads >> role:ListItem
```

**Selector Types:**
- `process:NAME` - Target specific application (use as chain start)
- `role:TYPE` - UI Automation role (Button, Edit, Window, Text, ListItem)
- `name:TEXT` - Element name (supports wildcards: `name:*partial*`)
- `nativeid:ID` - Native automation ID
- `classname:CLASS` - Window class name
- `visible:true/false` - Filter by visibility
- `nth:N` - Select nth match (0-indexed)

**CRITICAL**: For actions (click, type), always scope with `process:` first!

---

## Troubleshooting

### "401 Unauthorized"
- Set `MCP_AUTH_TOKEN` environment variable
- Verify token matches server configuration

### "Element not found"
- Element not visible on screen
- Wrong process name (case-sensitive!)
- Take screenshot to see current state

### "Connection refused"
- MCP server not running on VM
- Check health: `curl http://<IP>:8080/health`

---

## Example Session

```bash
# Set up
export MCP_AUTH_TOKEN="your-token"
export MCP_URL="http://<IP>:8080/mcp"

# 1. Check what's running
terminator mcp exec --url "$MCP_URL" run_command '{"run": "tasklist", "timeout_seconds": 5}'

# 2. Get desktop UI tree
terminator mcp exec --url "$MCP_URL" get_desktop_elements '{"depth": 1}'

# 3. Take screenshot
terminator mcp exec --url "$MCP_URL" take_screenshot '{}'

# 4. Launch an app
terminator mcp exec --url "$MCP_URL" open_application '{"path": "notepad.exe"}'

# 5. Wait for window
terminator mcp exec --url "$MCP_URL" wait_for_element '{"selector": "process:notepad >> role:Window", "timeout_seconds": 10}'

# 6. Type text
terminator mcp exec --url "$MCP_URL" type_text '{"selector": "process:notepad >> role:Edit", "text": "Hello from remote!"}'

# 7. Click a button
terminator mcp exec --url "$MCP_URL" click_element '{"selector": "process:notepad >> role:MenuItem && name:File"}'
```
