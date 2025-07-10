## Terminator MCP Agent

<!-- BADGES:START -->

[<img alt="Install in VS Code" src="https://img.shields.io/badge/VS_Code-VS_Code?style=flat-square&label=Install%20Server&color=0098FF">](https://insiders.vscode.dev/redirect?url=vscode%3Amcp%2Finstall%3F%257B%2522terminator-mcp-agent%2522%253A%257B%2522command%2522%253A%2522npx%2522%252C%2522args%2522%253A%255B%2522-y%2522%252C%2522terminator-mcp-agent%2522%255D%257D%257D)
[<img alt="Install in VS Code Insiders" src="https://img.shields.io/badge/VS_Code_Insiders-VS_Code_Insiders?style=flat-square&label=Install%20Server&color=24bfa5">](https://insiders.vscode.dev/redirect?url=vscode-insiders%3Amcp%2Finstall%3F%257B%2522terminator-mcp-agent%2522%253A%257B%2522command%2522%253A%2522npx%2522%252C%2522args%2522%253A%255B%2522-y%2522%252C%2522terminator-mcp-agent%2522%255D%257D%257D)
[<img alt="Install in Cursor" src="https://img.shields.io/badge/Cursor-Cursor?style=flat-square&label=Install%20Server&color=22272e">](https://cursor.com/install-mcp?name=terminator-mcp-agent&config=eyJjb21tYW5kIjoibnB4IiwiYXJncyI6WyIteSIsInRlcm1pbmF0b3ItbWNwLWFnZW50Il19)

<!-- BADGES:END -->

A Model Context Protocol (MCP) server that provides desktop GUI automation capabilities using the [Terminator](https://github.com/mediar-ai/terminator) library. This server enables LLMs and agentic clients to interact with Windows, macOS, and Linux applications through structured accessibility APIs—no vision models or screenshots required.

### Getting Started

The easiest way to get started is to use the one-click install buttons above for your specific editor (VS Code, Cursor, etc.).

Alternatively, you can install and configure the agent from your command line.

**1. Install & Configure Automatically**
Run the following command and select your MCP client from the list:

```sh
npx -y terminator-mcp-agent --add-to-app
```

**2. Manual Configuration**
If you prefer, you can add the following to your MCP client's settings file:

```json
{
	"mcpServers": {
		"terminator-mcp-agent": {
			"command": "npx",
			"args": ["-y", "terminator-mcp-agent"]
		}
	}
}
```

### Core Workflows: From Interaction to Structured Data

The Terminator MCP agent offers two primary workflows for automating desktop tasks. Both paths lead to the same goal: creating a >95% accuracy, 10000x faster than humans, automation.

#### 1. Iterative Development with `execute_sequence`

This is the most powerful and flexible method. You build a workflow step-by-step, using MCP tools to inspect the UI and refine your actions.

1.  **Inspect the UI**: Start by using `get_focused_window_tree` to understand the structure of your target application. This gives you the roles, names, and IDs of all elements.
2.  **Build a Sequence**: Create an `execute_sequence` tool call with a series of actions (`click_element`, `type_into_element`, etc.). Use robust selectors (like `role|name` or stable `properties:AutomationId:value` selectors) whenever possible.
3.  **Capture the Final State**: Ensure the last step in your sequence is an action that returns a UI tree. The `wait_for_element` tool with `include_tree: true` is perfect for this, as it captures the application's state after your automation has run.
4.  **Extract Structured Data with `output_parser`**: Add the `output_parser` argument to your `execute_sequence` call. Define a set of rules using our JSON-based DSL to parse the final UI tree. If successful, the tool result will contain a `parsed_output` field with your clean JSON data.

Here is an example of an `output_parser` that extracts insurance quote data from a web page:
```json
"output_parser": {
    "uiTreeJsonPath": "$.results[-1].results[-1].result.content[0].Json.ui_tree",
    "itemContainerDefinition": {
        "nodeConditions": [{ "property": "role", "op": "equals", "value": "Group" }],
        "childConditions": {
            "logic": "and",
            "conditions": [
                { "existsChild": { "conditions": [{ "property": "name", "op": "startsWith", "value": "$" }] } },
                { "existsChild": { "conditions": [{ "property": "name", "op": "equals", "value": "Monthly Price" }] } }
            ]
        }
    },
    "fieldsToExtract": {
        "monthlyPrice": {
            "fromChild": {
                "conditions": [{ "property": "name", "op": "startsWith", "value": "$" }],
                "extractProperty": "name"
            }
        }
    }
}
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

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Workflow Conditions

### Using `always()` in if expressions

Similar to GitHub Actions, you can use `always()` in the `if` expression to ensure a step runs regardless of previous failures:

```json
{
  "steps": [
    {
      "tool_name": "type_into_element",
      "arguments": { "selector": "input", "text_to_type": "test" }
    },
    {
      "tool_name": "capture_screen",
      "if": "always()",
      "arguments": {},
      "continue_on_error": true
    }
  ]
}
```

The `capture_screen` step will run even if `type_into_element` fails. This is useful for:
- Cleanup steps (closing applications, releasing resources)
- Logging and debugging (capturing screenshots, saving logs)
- Final reporting steps

Both `always()` and `${{ always() }}` syntax are supported.

