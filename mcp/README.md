# Terminator MCP Agent

This directory contains the Model Context Protocol (MCP) agent that allows AI assistants (like Cursor) to interact with your desktop using the Terminator UI automation library.

<img width="1512" alt="Screenshot 2025-04-16 at 9 29 42 AM" src="https://github.com/user-attachments/assets/457ebaf2-640c-4f21-a236-fcb2b92748ab" />

MCP is useful to test out the `terminator` lib and see what you can do. You can use any model.

## Prerequisites

1.  **Node.js:** You need Node.js installed. Download it from [nodejs.org](https://nodejs.org/).
2.  **Git:** You need Git installed. Download it from [git-scm.com](https://git-scm.com/).
3.  **Running Terminator Server:** The main Terminator server must be running. Follow the steps in the main project [Quick Start guide](../README.md#quick-start) (steps 1-3) to clone the repo, download the server, and run it. **Leave the server running in its terminal.**

## Installation & Setup

Open a new terminal (like PowerShell or Command Prompt) **separate from the one running the Terminator server**:

1.  **Navigate to the MCP directory:**
    If you haven't already, clone the repository and change to the `mcp` directory:
    ```bash
    git clone https://github.com/mediar-ai/terminator
    cd terminator/mcp
    ```
    If you already cloned it, just navigate to the `mcp` sub-directory within your `terminator` folder.

2.  **Install dependencies and build the agent:**
    ```bash
    npm install && npm run build
    ```
    *(This assumes a `build` script exists in `package.json`. If it fails, check `package.json` for the correct build command, e.g., `tsc`).*

3.  **Configure Cursor:**
    You need to tell Cursor how to run this agent. Create a file named `mcp.json` in your Cursor configuration directory (`~/.cursor` on macOS/Linux, `%USERPROFILE%\.cursor` on Windows).

    **macOS / Linux:**

    ```bash
    # Run this command inside the terminator/mcp directory
    MCP_PATH="$(pwd)/dist/index.js"
    JSON_CONTENT=$(cat <<EOF
    {
      "mcpServers": {
        "terminator-mcp-agent": {
          "command": "node",
          "args": [
              "$MCP_PATH"
          ]
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
    $mcpPath = ($pwd).Path.Replace('\', '\\') + '\\dist\\index.js'
    $jsonContent = @"
    {
      "mcpServers": {
        "terminator-mcp-agent": {
          "command": "node",
          "args": [
              "$mcpPath"
          ]
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

## Running

1.  Ensure the main Terminator server (e.g., `server.exe`) is still running (from the Prerequisites step).
2.  Restart Cursor if it was already running.
3.  Cursor should now automatically detect and use the `terminator-mcp-agent` when you invoke its capabilities. You don't need to manually run the `node dist/index.js` command; Cursor handles it based on the `mcp.json` configuration.

## Other examples / showcases

scrapping your whole desktop including background windows:

<img width="505" alt="image" src="https://github.com/user-attachments/assets/b21c9f85-1a34-488f-8779-d912071ec273" />

vibe working

<img width="1512" alt="Screenshot 2025-04-16 at 9 57 14 AM" src="https://github.com/user-attachments/assets/9eb40279-9a99-4498-8233-28b66f89ab92" />


