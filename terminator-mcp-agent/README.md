# Terminator MCP Agent

This directory contains the Model Context Protocol (MCP) agent that allows AI assistants, like Cursor and Claude desktop to interact with your desktop using the Terminator UI automation library.

<img width="1512" alt="Screenshot 2025-04-16 at 9 29 42 AM" src="https://github.com/user-attachments/assets/457ebaf2-640c-4f21-a236-fcb2b92748ab" />

MCP is useful to test out the `terminator` lib and see what you can do. You can use any model.

<br>

## For configuring Terminator MCP in desktop apps like [Cursor](https://www.cursor.com/) and [Claude](https://claude.ai/download):
Open terminal and run this command:

```ps1
iwr -useb https://raw.githubusercontent.com/mediar-ai/terminator/refs/heads/main/terminator-mcp-agent/install.ps1 | iex
```
> it'll guide through the configuration, only for [Cursor](https://www.cursor.com/) and [Claude](https://claude.ai/download) desktop app


<br>

<details>

<summary> 

## Manual Installation & Setup 

</summary>

Open a new terminal (like PowerShell or Command Prompt)

1.  **Clone the Terminator locally**
    If you haven't already, clone the repository:
    ```bash
    git clone https://github.com/mediar-ai/terminator
    ```
    Then build the terminator-mcp-agent:
    ```
    cargo build -p terminator-mcp-agent --release
    ```

2.  **Configure Cursor:**
    You need to tell Cursor how to run this agent. Create a file named `mcp.json` in your Cursor configuration directory (`~/.cursor` on macOS/Linux, `%USERPROFILE%\.cursor` on Windows).

    **macOS / Linux:**

    ```bash
    # Run this command inside the terminator/mcp directory
    MCP_PATH="$(pwd)/target/release/terminator-mcp-agent.exe"
    JSON_CONTENT=$(cat <<EOF
    {
      "mcpServers": {
        "terminator-mcp-agent": {
          "command": "$MCP_PATH",
          "args": []
        }
      }
    }
    EOF
    )
    echo "--- Copy the JSON below and save it as mcp.json in your ~/.cursor directory ---"
    echo "$JSON_CONTENT"
    echo "------------------------------------------------------------------------------------------"
    mkdir -p "$HOME/.cursor"
    ```

    **Windows (PowerShell):**

    You can use this PowerShell command **while inside the `mcp` directory** to generate the correct JSON content:

    ```powershell
    # Run this command inside the terminator/mcp directory
    $mcpPath = ($pwd).Path.Replace('\', '\\') + '\\target\\release\\terminator-mcp-agent.exe'
    $jsonContent = @"
    {
      "mcpServers": {
        "terminator-mcp-agent": {
          "command": "$mcpPath",
          "args": []
        }
      }
    }
    "@
    Write-Host "--- Copy the JSON below and save it as mcp.json in your %USERPROFILE%\.cursor directory ---"
    Write-Host $jsonContent
    Write-Host "------------------------------------------------------------------------------------------"
    # Optional: Try to automatically open the directory
    Start-Process "$env:USERPROFILE\.cursor" -ErrorAction SilentlyContinue
    ```

    *   Run the appropriate command for your OS (PowerShell for Windows, Bash for macOS/Linux).
    *   Copy the JSON output (starting with `{` and ending with `}`).
    *   Create the `%USERPROFILE%\.cursor` (Windows) or `~/.cursor` (macOS/Linux) directory if it doesn't exist.
    *   Create a new file named `mcp.json` inside that directory.
    *   Paste the copied JSON content into `mcp.json` and save it.


###  **Configure Claude Desktop app:**

open the claude app and search for developer options then MCP. when you click on configure MCP button it'll open a json file where you have to edit a `claude_desktop_config.json` file

```
{
  "mcpServers": {
    "terminator-mcp-agent": {
      "command": "path_to_terminator-mcp-agent.exe",
      "args": []
    }
  }
}
```
remember to replace `path_to_terminator` exe with actual path of terminator-mcp-agent binary, you can find the binary of terminator mcp in the target directory where you've build the project!
</details>


