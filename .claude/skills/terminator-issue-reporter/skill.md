# Terminator Issue Reporter

**Auto-activates when user mentions:**
- "terminator issue"
- "terminator bug"
- "terminator error"
- "terminator not working"
- "create issue"
- "report bug"

## Goal
Create detailed GitHub issue for terminator problems with full context, logs, and reproduction steps.

## Instructions

### 1. Gather Issue Context
Ask user:
- What were you trying to do?
- What happened (actual behavior)?
- What did you expect to happen?
- Any error messages?

### 2. Collect System Info
```bash
# Get versions
node --version
git rev-parse HEAD  # Current commit
uname -a  # OS info (Linux/macOS)
# or on Windows:
systeminfo | findstr /B /C:"OS Name" /C:"OS Version"
```

### 3. Find and Extract Logs
**MCP Server Logs Location:**
- **Windows:** `%LOCALAPPDATA%\claude-cli-nodejs\Cache\*\mcp-logs-terminator-mcp-agent\*.txt`
- **macOS/Linux:** `~/.local/share/claude-cli-nodejs/Cache/*/mcp-logs-terminator-mcp-agent/*.txt`

**PowerShell command to get latest logs:**
```powershell
Get-ChildItem (Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'claude-cli-nodejs\Cache\*\mcp-logs-terminator-mcp-agent\*.txt') | Sort-Object LastWriteTime -Descending | Select-Object -First 1 | Get-Content -Tail 100
```

**Extract:**
- Last 100 lines of most recent log
- Look for: error stack traces, EVAL_ERROR, NULL_RESULT, ElementNotFound, timeout errors
- Tool call failures and their arguments

### 4. Identify Issue Category
- **Selector Issues:** Element not found, wrong element clicked
- **Browser Automation:** execute_browser_script errors, Chrome extension issues
- **Workflow Execution:** Step failures, conditional logic, jumps
- **MCP Server:** Connection issues, tool registration, binary crashes
- **Performance:** Slow execution, timeouts, memory issues

### 5. Create Minimal Reproduction
- Simplify workflow to minimal failing case
- Remove sensitive data (passwords, API keys, personal info)
- Include only relevant steps
- Test if reproduction still fails

### 6. Generate GitHub Issue

```markdown
## Description
[Clear 2-3 sentence description of the problem]

## Environment
- **OS:** [Windows 11/macOS 14.5/Ubuntu 22.04]
- **Node.js:** [v20.10.0]
- **Terminator Commit:** [git rev-parse HEAD output]
- **Browser:** [Chrome 120.0.6099.109] (if applicable)
- **MCP Client:** [Claude Desktop / claude-cli]

## Expected Behavior
[What should happen]

## Actual Behavior
[What actually happens, including error message]

## Reproduction Steps
1. [Step 1]
2. [Step 2]
3. Error occurs: [exact error]

## Minimal Reproduction Workflow
```yaml
steps:
  - tool_name: [tool]
    arguments:
      [minimal args]
# Only include steps needed to reproduce
```

## Error Logs
```
[Last 50-100 lines from MCP logs around the error]
```

## Screenshots
[If UI-related issue, include screenshots]

## Attempted Solutions
- [x] Restarted MCP server
- [x] Cleared cache
- [ ] Tried alternative selector
- [ ] etc.

## Possible Cause / Suggestion
[Any insights into what might be causing this]

## Additional Context
[Related issues, workarounds, similar problems]
```

### 7. Provide Next Steps
1. Show formatted issue to user
2. Ask user to review and confirm
3. Provide GitHub link: `https://github.com/louis030195/terminator/issues/new`
4. Offer to copy issue body to clipboard
5. Suggest immediate workarounds based on error type

## Common Issue Patterns & Solutions

### ElementNotFound / Selector Issues
**Workarounds:**
- Use `get_window_tree` to inspect actual UI structure
- Try numeric ID selector instead of role|name
- Use `validate_element` first to check existence
- Check if element is in different window

### Browser Script Errors
**Common causes:**
- Missing IIFE wrapper: `(function() { ... })()`
- Missing `typeof` checks for env variables
- Top-level return without IIFE
- NULL_RESULT from missing return in Promise handlers

**Solutions:**
- Wrap script in IIFE
- Add typeof checks: `const x = (typeof x !== 'undefined') ? x : default;`
- Return values in .then() and .catch() handlers

### MCP Connection Issues
**Workarounds:**
- Restart Claude Desktop
- Check MCP config in `%APPDATA%\Claude\claude_desktop_config.json`
- Verify binary path and permissions
- Check for port conflicts

### Workflow State Issues
**Solutions:**
- Clear workflow state: delete `.workflow_state/` directory
- Use `start_from_step` to resume from specific step
- Check jump conditions syntax
- Verify step IDs are unique

## Example Usage

**User:** "I'm getting NULL_RESULT when running a browser script"

**Skill Response:**
1. Retrieve latest MCP logs
2. Identify it's a browser script issue
3. Check for common patterns (missing IIFE, no return, etc.)
4. Create issue with:
   - Error logs showing NULL_RESULT
   - Browser script code
   - Suggestion about IIFE wrapper or return statements
5. Provide workaround: wrap script in `(function() { ... })()`

## Output Format

After gathering all info, show user:

```
📋 **GitHub Issue Ready**

Title: [Auto-generated concise title]

[Full formatted issue body]

🔗 **Next Steps:**
1. Review issue above
2. Click to create: https://github.com/mediar-ai/terminator/issues/new
3. Copy issue body (I can do this for you)

💡 **Immediate Workaround:**
[Specific suggestion based on error type]
```
