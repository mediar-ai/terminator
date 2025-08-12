## Terminator MCP Agent

<!-- BADGES:START -->

[<img alt="Install in VS Code" src="https://img.shields.io/badge/VS_Code-VS_Code?style=flat-square&label=Install%20Server&color=0098FF">](https://insiders.vscode.dev/redirect?url=vscode%3Amcp%2Finstall%3F%257B%2522terminator-mcp-agent%2522%253A%257B%2522command%2522%253A%2522npx%2522%252C%2522args%2522%253A%255B%2522-y%2522%252C%2522terminator-mcp-agent%2540latest%2522%255D%257D%257D)
[<img alt="Install in VS Code Insiders" src="https://img.shields.io/badge/VS_Code_Insiders-VS_Code_Insiders?style=flat-square&label=Install%20Server&color=24bfa5">](https://insiders.vscode.dev/redirect?url=vscode-insiders%3Amcp%2Finstall%3F%257B%2522terminator-mcp-agent%2522%253A%257B%2522command%2522%253A%2522npx%2522%252C%2522args%2522%253A%255B%2522-y%2522%252C%2522terminator-mcp-agent%2540latest%2522%255D%257D%257D)
[<img alt="Install in Cursor" src="https://img.shields.io/badge/Cursor-Cursor?style=flat-square&label=Install%20Server&color=22272e">](https://cursor.com/install-mcp?name=terminator-mcp-agent&config=eyJjb21tYW5kIjoibnB4IiwiYXJncyI6WyIteSIsInRlcm1pbmF0b3ItbWNwLWFnZW50QGxhdGVzdCJdfQ%3D%3D)

<!-- BADGES:END -->

A Model Context Protocol (MCP) server that provides desktop GUI automation capabilities using the [Terminator](https://github.com/mediar-ai/terminator) library. This server enables LLMs and agentic clients to interact with Windows, macOS, and Linux applications through structured accessibility APIs—no vision models or screenshots required.

### HTTP Endpoints (when running with `-t http`)

- `GET /health`: Always returns 200 while the process is alive.
- `GET /status`: Busy-aware probe for load balancers. Returns JSON and appropriate status:
  - 200 when idle: `{ "busy": false, "activeRequests": 0, "maxConcurrent": 1, "lastActivity": "<ISO-8601>" }`
  - 503 when busy: `{ "busy": true, "activeRequests": 1, "maxConcurrent": 1, "lastActivity": "<ISO-8601>" }`
  - Content-Type is `application/json`.
- `POST /mcp`: MCP execution endpoint. Enforces single-request concurrency per machine by default.

Concurrency is controlled by the `MCP_MAX_CONCURRENT` environment variable (default `1`). Only accepted `POST /mcp` requests are counted toward `activeRequests`. If the server is at capacity, new `POST /mcp` requests return 503 immediately. This 503 behavior is intentional so an Azure Load Balancer probing `GET /status` can take a busy VM out of rotation and route traffic elsewhere.

### Getting Started

The easiest way to get started is to use the one-click install buttons above for your specific editor (VS Code, Cursor, etc.).

Alternatively, you can install and configure the agent from your command line.

**1. Install & Configure Automatically**
Run the following command and select your MCP client from the list:

```sh
npx -y terminator-mcp-agent@latest --add-to-app
```

**2. Manual Configuration**
If you prefer, you can add the following to your MCP client's settings file:

```json
{
  "mcpServers": {
    "terminator-mcp-agent": {
      "command": "npx",
      "args": ["-y", "terminator-mcp-agent@latest"]
    }
  }
}
```

### Command Line Interface (CLI) Execution

For automation workflows and CI/CD pipelines, you can execute workflows directly from the command line using the [Terminator CLI](../terminator-cli/README.md):

**Quick Start:**

```bash
# Execute a workflow file
terminator mcp run workflow.yml

# With verbose logging
terminator mcp run workflow.yml --verbose

# Dry run (validate without executing)
terminator mcp run workflow.yml --dry-run

# Use specific MCP server version
terminator mcp run workflow.yml --command "npx -y terminator-mcp-agent@latest"
```

**Workflow File Formats:**

Direct workflow format (`workflow.yml`):

```yaml
steps:
  - tool_name: navigate_browser
    arguments:
      url: "https://example.com"
  - tool_name: click_element
    arguments:
      selector: "role:Button|name:Submit"
stop_on_error: true
include_detailed_results: true
```

Tool call wrapper format (`workflow.json`):

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "steps": [
      {
        "tool_name": "navigate_browser",
        "arguments": {
          "url": "https://example.com"
        }
      }
    ]
  }
}
```

**JavaScript Execution in Workflows:**

Execute custom JavaScript code with access to desktop automation APIs:

```yaml
steps:
  - tool_name: run_javascript
    arguments:
      engine: "nodejs"
      script: |
        // Access desktop automation APIs
        const elements = await desktop.locator('role:button').all();
        log(`Found ${elements.length} buttons`);

        // Conditional logic and bulk operations
        for (const element of elements) {
          const name = await element.name();
          if (name.includes('Submit')) {
            await element.click();
            break;
          }
        }

        return {
          buttons_found: elements.length,
          action: 'clicked_submit'
        };
```

For complete CLI documentation, see [Terminator CLI README](../terminator-cli/README.md).

### Core Workflows: From Interaction to Structured Data

The Terminator MCP agent offers two primary workflows for automating desktop tasks. Both paths lead to the same goal: creating a >95% accuracy, 10000x faster than humans, automation.

#### 1. Iterative Development with `execute_sequence`

This is the most powerful and flexible method. You build a workflow step-by-step, using MCP tools to inspect the UI and refine your actions.

1.  **Inspect the UI**: Start by using `get_focused_window_tree` to understand the structure of your target application. This gives you the roles, names, and IDs of all elements.
2.  **Build a Sequence**: Create an `execute_sequence` tool call with a series of actions (`click_element`, `type_into_element`, etc.). Use robust selectors (like `role|name` or stable `properties:AutomationId:value` selectors) whenever possible.
3.  **Capture the Final State**: Ensure the last step in your sequence is an action that returns a UI tree. The `wait_for_element` tool with `include_tree: true` is perfect for this, as it captures the application's state after your automation has run.
4.  **Extract Structured Data with `output_parser`**: Add the `output_parser` argument to your `execute_sequence` call. Write JavaScript code to parse the final UI tree and extract structured data. If successful, the tool result will contain a `parsed_output` field with your clean JSON data.

Here is an example of an `output_parser` that extracts insurance quote data from a web page:

```yaml
output_parser:
  ui_tree_source_step_id: capture_quotes_tree
  javascript_code: |
    // Find all quote groups with Image and Text children
    const results = [];

    function findElementsRecursively(element) {
        if (element.attributes && element.attributes.role === 'Group') {
            const children = element.children || [];
            const hasImage = children.some(child => 
                child.attributes && child.attributes.role === 'Image'
            );
            const hasText = children.some(child => 
                child.attributes && child.attributes.role === 'Text'
            );
            
            if (hasImage && hasText) {
                const textElements = children.filter(child => 
                    child.attributes && child.attributes.role === 'Text' && child.attributes.name
                );
                
                let carrierProduct = '';
                let monthlyPrice = '';
                
                for (const textEl of textElements) {
                    const text = textEl.attributes.name;
                    if (text.includes(':')) {
                        carrierProduct = text;
                    }
                    if (text.startsWith('$')) {
                        monthlyPrice = text;
                    }
                }
                
                if (carrierProduct && monthlyPrice) {
                    results.push({
                        carrierProduct: carrierProduct,
                        monthlyPrice: monthlyPrice
                    });
                }
            }
        }
        
        if (element.children) {
            for (const child of element.children) {
                findElementsRecursively(child);
            }
        }
    }

    findElementsRecursively(tree);
    return results;
```

#### 2. Recording Human Actions with `record_workflow`

For simpler tasks, you can record your own actions to generate a baseline workflow.

1.  **Start Recording**: Call `record_workflow` with `action: "start"`.
2.  **Perform the Task**: Manually perform the clicks, typing, and other interactions in the target application.
3.  **Stop and Save**: Call `record_workflow` with `action: "stop"`. This returns a complete workflow JSON file containing all your recorded actions.
4.  **Refine and Parse**: The recorded workflow is a great starting point. You can then refine the selectors for robustness, add a final step to capture the UI tree, and attach an `output_parser` to extract structured data, just as you would in the iterative workflow.

## Local Development

To build and test the agent from the source code:

```sh
# 1. Clone the entire Terminator repository
git clone https://github.com/mediar-ai/terminator

# 2. Navigate to the agent's directory
cd terminator/terminator-mcp-agent

# 3. Install Node.js dependencies
npm install

# 4. Build the Rust binary and Node.js wrapper
npm run build

# 5. To use your local build in your MCP client, link it globally
npm install --global .
```

Now, when your MCP client runs `terminator-mcp-agent`, it will use your local build instead of the published `npm` version.

---

## Troubleshooting

- Make sure you have Node.js installed (v16+ recommended).
- For VS Code/Insiders, ensure the CLI (`code` or `code-insiders`) is available in your PATH.
- If you encounter issues, try running with elevated permissions.

### Version Compatibility Issues

**Problem**: "missing field `items`" or schema mismatch errors

**Solution**: Ensure you're using the latest MCP server version:

```bash
# Force latest version in CLI
terminator mcp run workflow.yml --command "npx -y terminator-mcp-agent@latest"

# Update MCP client configuration to use @latest
{
  "mcpServers": {
    "terminator-mcp-agent": {
      "command": "npx",
      "args": ["-y", "terminator-mcp-agent@latest"]
    }
  }
}

# Clear npm cache if needed
npm cache clean --force
```

### CLI Integration Issues

**Problem**: CLI commands not working or connection errors

**Solution**: Test MCP connectivity step by step:

```bash
# Test basic connectivity
terminator mcp exec get_applications

# Test with verbose logging
terminator mcp run workflow.yml --verbose

# Test with dry run first
terminator mcp run workflow.yml --dry-run

# Use HTTP connection for debugging
terminator mcp run workflow.yml --url http://localhost:3000/mcp
```

### JavaScript Execution Issues

**Problem**: JavaScript code fails or can't access desktop APIs

**Solution**: Verify JavaScript execution and API access:

```bash
# Test basic JavaScript execution
terminator mcp exec run_javascript '{"script": "return {test: true};"}'

# Test desktop API access with nodejs engine
terminator mcp exec run_javascript '{"engine": "nodejs", "script": "const elements = await desktop.locator(\"role:button\").all(); return {count: elements.length};"}'

# Debug with verbose logging
terminator mcp run workflow.yml --verbose
```

### Workflow File Issues

**Problem**: Workflow parsing errors or unexpected behavior

**Solution**: Validate workflow structure:

```bash
# Validate workflow syntax
terminator mcp run workflow.yml --dry-run

# Test with minimal workflow first
echo 'steps: [{tool_name: get_applications}]' > test.yml
terminator mcp run test.yml

# Check both YAML and JSON formats work
terminator mcp run workflow.yml   # YAML
terminator mcp run workflow.json  # JSON
```

### Platform-Specific Issues

**Windows**:

- Ensure Windows UI Automation APIs are available
- Run with administrator privileges if accessibility features are restricted
- Check Windows Defender/antivirus isn't blocking automation

**macOS**:

- Grant accessibility permissions in System Preferences > Security & Privacy
- Ensure the terminal/IDE has accessibility access
- Check macOS version compatibility (10.14+ recommended)

**Linux**:

- Ensure AT-SPI (assistive technology) is enabled
- Install required packages: `sudo apt-get install at-spi2-core`
- Check desktop environment compatibility (GNOME, KDE, XFCE supported)

### Performance Optimization

**Large UI Trees**:

- Use specific selectors instead of broad element searches
- Implement delays between rapid operations
- Consider using `include_tree: false` for intermediate steps

**JavaScript Performance**:

- Use `quickjs` engine for lightweight operations
- Use `nodejs` engine only when full APIs are needed
- Implement `sleep()` delays in loops to prevent overwhelming the UI

For additional help, see the [Terminator CLI documentation](../terminator-cli/README.md) or open an issue on GitHub.

---

## 📚 Full `execute_sequence` Reference & Sample Workflow

> **Why another example?** The quick start above shows the concept, but many users asked for a fully-annotated workflow schema. The example below automates the Windows **Calculator** app—so it is 100% safe to share and does **not** reveal any private customer data. Feel free to copy-paste and adapt it to your own application.

### 1. Anatomy of an `execute_sequence` Call

```jsonc
{
  "tool_name": "execute_sequence",
  "arguments": {
    "variables": {
      // 1️⃣ Re-usable inputs with type metadata
      "app_path": {
        "type": "string",
        "label": "Calculator EXE Path",
        "default": "calc.exe"
      },
      "first_number": {
        "type": "string",
        "label": "First Number",
        "default": "42"
      },
      "second_number": {
        "type": "string",
        "label": "Second Number",
        "default": "8"
      }
    },
    "inputs": {
      // 2️⃣ Concrete values for *this run*
      "app_path": "calc.exe",
      "first_number": "42",
      "second_number": "8"
    },
    "selectors": {
      // 3️⃣ Human-readable element shortcuts
      "calc_window": "role:Window|name:Calculator",
      "btn_clear": "role:Button|name:Clear",
      "btn_plus": "role:Button|name:Plus",
      "btn_equals": "role:Button|name:Equals"
    },
    "steps": [
      // 4️⃣ Ordered actions & control flow
      {
        "tool_name": "open_application",
        "arguments": { "path": "${{app_path}}" }
      },
      {
        "tool_name": "click_element", // 4a. Make sure the UI is reset
        "arguments": { "selector": "${{selectors.btn_clear}}" },
        "continue_on_error": true
      },
      {
        "group_name": "Enter First Number", // 4b. Groups improve logs
        "steps": [
          {
            "tool_name": "type_into_element",
            "arguments": {
              "selector": "${{selectors.calc_window}}",
              "text_to_type": "${{first_number}}"
            }
          }
        ]
      },
      {
        "tool_name": "click_element",
        "arguments": { "selector": "${{selectors.btn_plus}}" }
      },
      {
        "group_name": "Enter Second Number",
        "steps": [
          {
            "tool_name": "type_into_element",
            "arguments": {
              "selector": "${{selectors.calc_window}}",
              "text_to_type": "${{second_number}}"
            }
          }
        ]
      },
      {
        "tool_name": "click_element",
        "arguments": { "selector": "${{selectors.btn_equals}}" }
      },
      {
        "tool_name": "wait_for_element", // 4c. Capture final UI tree
        "arguments": {
          "selector": "${{selectors.calc_window}}",
          "condition": "exists",
          "include_tree": true,
          "timeout_ms": 2000
        }
      }
    ],
    "output_parser": {
      // 5️⃣ Turn the tree into clean JSON
      "javascript_code": "// Extract calculator display value\nconst results = [];\n\nfunction findElementsRecursively(element) {\n    if (element.attributes && element.attributes.role === 'Text') {\n        const item = {\n            displayValue: element.attributes.name || ''\n        };\n        results.push(item);\n    }\n    \n    if (element.children) {\n        for (const child of element.children) {\n            findElementsRecursively(child);\n        }\n    }\n}\n\nfindElementsRecursively(tree);\nreturn results;"
    }
  }
}
```

### 2. Key Concepts at a Glance

1. **Variables vs. Inputs** – Declare once, override per-run. This is perfect for parameterizing CI pipelines or A/B test data.
2. **Selectors** – Give every important UI element a _nickname_. It makes long workflows readable and easy to maintain.
3. **Templating** – `${{ ... }}` (GitHub Actions-style) _or_ legacy `{{ ... }}` lets you reference **any** key inside `variables`, `inputs`, or `selectors`. Both syntaxes are supported; the engine uses Mustache-style rendering.
4. **Groups & Control Flow** – Add `group_name`, `skippable`, `if`, or `continue_on_error` to any step for advanced branching.
5. **Output Parsing** – Always end with a step that includes the UI tree, then use the declarative JSON DSL to mine the data you need.

### 3. Running the Workflow

1. Ensure the Terminator MCP agent is running (it will auto-start in supported editors).
2. Send the JSON above as the body of an `execute_sequence` tool call from your LLM or test harness.
3. Inspect the response: if parsing succeeds you’ll see something like

```jsonc
{
  "parsed_output": {
    "displayValue": "50" // 42 + 8
  }
}
```

### 4. Tips for Production Workflows

- **Never hard-code credentials** – use environment variables or your secret manager.
- **Keep workflows short** – <100 steps is ideal. Break large tasks into multiple sequences.
- **Capture errors** – `continue_on_error` is useful, but also log `result.status` codes to catch silent failures.
- **Version control** – Store workflow JSON in a repo and use PR reviews just like regular code.

> Need more help? Browse the examples under `examples/` in this repo or open a discussion on GitHub.
