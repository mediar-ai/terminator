import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  TerminatorTools,
  FindWindowSchema,
  GetElementTextSchema,
  TypeIntoElementSchema,
  LocatorSchema, // For click
  PressKeySchema,
  RunCommandSchema,
  ExploreSchema,
  CaptureScreenSchema,
} from "./terminator.js"; // Import the wrapper class and schemas

// --- Configuration ---
// You might want to load this from .env or config files
const TERMINATOR_BASE_URL = process.env.TERMINATOR_URL; // e.g., http://127.0.0.1:9375 or leave undefined for default

// --- Initialize Terminator Tools ---
// Initialization is wrapped in the main function to handle potential errors
let terminatorTools: TerminatorTools;

// --- MCP Server Setup ---
const serverInfo = {
  name: "terminator-mcp-agent",
  version: "0.1.0",
  description: "An MCP server providing desktop automation via Terminator.",
};

const serverInstructions = `
You are an AI assistant capable of controlling a computer desktop using the available tools.
You can interact with UI elements, run commands, and read text.

**Workflow:**
1.  **Identify Target Window:** Use 'findWindow' to locate the main window of the application you need to interact with (e.g., by title). Note its selector (e.g., the \`suggested_selector\` field in the result, which often looks like '#12345...').
2.  **(Optional but Recommended) Explore:** Use 'explore' with the window's selector chain (e.g., ['window:"My App"'] or ['#windowId']) to understand its structure and find specific child elements (buttons, inputs, etc.). Pay close attention to the \`suggested_selector\` provided for each child element.
3.  **Interact:** Use tools like 'clickElement', 'getElementText', 'typeIntoElement', or 'pressKey' with the appropriate **selector chain**.
    *   A selector chain is an array starting with the window selector, followed by selectors for child elements, e.g., ['window:"My App"', '#saveButtonId'].
    *   **Crucially, prefer using the exact \`suggested_selector\` (like '#12345...') returned by 'explore' or 'findWindow'.** This ID is calculated based on multiple properties and is usually the most reliable way to target an element.
    *   If a suggested selector fails, you can try simpler selectors like \`text:"Save"\` or \`role:"button"\`, but these might match multiple elements or be less reliable.
    *   **Selector Failures:** If interaction fails (e.g., timeout), ensure the element is visible, try increasing the \`timeoutMs\` parameter (e.g., 10000 for 10 seconds), re-explore the parent element, or verify you have the correct window selector.
4.  **Execute Commands:** Use 'runCommand' for non-UI tasks.
    *   **This is the preferred method for running shell commands (like \`ls\`, \`dir\`, \`git status\`, etc.)** instead of trying to type into a terminal UI element, which can be unreliable. Specify the command for Windows (\`windowsCommand\`) or Unix (\`unixCommand\`).

**Available Tools:**

- **findWindow**: Finds a top-level window by its title. Returns the window element's details, including a \`suggested_selector\`.
- **explore**: Lists child elements within a given element (or the screen). Use its output to find the \`suggested_selector\` for child elements needed in other tools.
- **clickElement**: Clicks a UI element specified by its selector chain.
- **getElementText**: Reads text content from a UI element specified by its selector chain.
- **typeIntoElement**: Types text into a UI element (like an input field) specified by its selector chain. Requires a reliable selector for the input element.
- **pressKey**: Sends a key press (like 'Enter', 'Tab', 'Ctrl+C') to a UI element specified by its selector chain.
- **runCommand**: Executes a shell command directly on the system (specify \`windowsCommand\` or \`unixCommand\`). Ideal for terminal tasks.

Contextual information:
- The current date and time is ${new Date().toLocaleString()}.
- Current operating system: ${process.platform}.
- Current working directory: ${process.cwd()}.

**Important:** Always provide the full selector chain when interacting with elements inside a window. Start the chain with the window selector. **Prioritize using the \`suggested_selector\` from \`explore\` results.** Use \`runCommand\` for shell operations.
`;

const server = new McpServer(serverInfo, {
  instructions: serverInstructions,
  capabilities: {
    resources: {},
  },
});

// --- Tool Definitions ---

// findWindow Tool
server.tool(
  "findWindow",
  "Finds a top-level window by title.",
  FindWindowSchema.shape,
  async (args) => {
    const result = await terminatorTools.findWindow(args);
    return {
      content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
    };
  }
);

// getElementText Tool
server.tool(
  "getElementText",
  "Reads text content from a UI element.",
  GetElementTextSchema.shape,
  async (args) => {
    const result = await terminatorTools.getElementText(args);
    return {
      content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
    };
  }
);

// typeIntoElement Tool
server.tool(
  "typeIntoElement",
  "Types text into a UI element.",
  TypeIntoElementSchema.shape,
  async (args) => {
    const result = await terminatorTools.typeIntoElement(args);
    return {
      content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
    };
  }
);

// clickElement Tool
server.tool(
  "clickElement",
  "Clicks a UI element.",
  LocatorSchema.shape,
  async (args) => {
    const result = await terminatorTools.clickElement(args);
    return {
      content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
    };
  }
);

// pressKey Tool
server.tool(
  "pressKey",
  "Sends a key press to a UI element.",
  PressKeySchema.shape,
  async (args) => {
    const result = await terminatorTools.pressKey(args);
    return {
      content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
    };
  }
);

// runCommand Tool
server.tool(
  "runCommand",
  "Executes a shell command.",
  RunCommandSchema.shape,
  async (args) => {
    const result = await terminatorTools.runCommand(args);
    return {
      content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
    };
  }
);

// explore Tool
server.tool(
  "explore",
  "Explores UI elements and their children.",
  ExploreSchema.shape,
  async (args) => {
    const result = await terminatorTools.explore(args);
    console.log(JSON.stringify({
      type: "mcp_tool_result",
      tool: "explore",
      args: args,
      result_summary: {
        success: !("error" in result),
        children_count: "error" in result ? 0 : result.children.length,
        has_error: "error" in result
      }
    }));
    return {
      content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
    };
  }
);

// activateElement Tool (New)
server.tool(
  "activateElement",
  "Activates the window containing the specified element, bringing it to the foreground.",
  LocatorSchema.shape, // Reuses schema needing selectorChain + timeoutMs
  async (args) => {
    // Assuming terminatorTools has an activateApp method mapped to the backend
    const result = await terminatorTools.activateApp(args);
    return {
      content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
    };
  }
);

// captureScreen Tool (New)
server.tool(
  "captureScreen",
  "Captures a screenshot of the primary monitor and returns the recognized text content (OCR).",
  CaptureScreenSchema.shape,
  async (args) => {
    const result = await terminatorTools.captureScreen();
    if ("error" in result) {
      return {
        content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        isError: true,
      };
    } else {
      // Result now directly contains { text: "..." }
      return {
        content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
      };
    }
  }
);

// --- Resource Definitions ---

const OPEN_WINDOWS_URI = "terminator://windows/open";

// Use server.resource() instead of setRequestHandler
server.resource(
  "open-windows", // A unique name for this resource registration
  OPEN_WINDOWS_URI, // The static URI for this resource
  {
    // Metadata for the resource (used for listing/discovery)
    name: "Open Windows",
    description:
      "Lists the titles and selectors of currently open top-level windows.",
    mimeType: "application/json",
  },
  async (uri) => {
    // The handler function to read the resource
    console.log(JSON.stringify({
      type: "mcp_resource_request",
      resource: "open-windows",
      uri: uri.href,
      timestamp: new Date().toISOString()
    }));
    const result = await terminatorTools.listOpenWindows();

    if ("error" in result) {
      console.error(JSON.stringify({
        type: "mcp_resource_error",
        resource: "open-windows",
        uri: uri.href,
        error: result.error,
        timestamp: new Date().toISOString()
      }));
      // Throw an error to indicate failure
      throw new Error(`Failed to list open windows: ${result.error}`);
    }

    console.log(JSON.stringify({
      type: "mcp_resource_success",
      resource: "open-windows",
      uri: uri.href,
      windows_count: result.windows.length,
      timestamp: new Date().toISOString()
    }));
    return {
      contents: [
        {
          uri: uri.href, // Use the requested URI
          mimeType: "application/json",
          text: JSON.stringify(result.windows, null, 2), // Return the list as JSON
        },
      ],
    };
  }
);

// --- Start Server ---
async function main() {
  console.log(JSON.stringify({
    type: "mcp_server_start",
    server: serverInfo.name,
    version: serverInfo.version,
    timestamp: new Date().toISOString()
  }));

  try {
    // Initialize TerminatorTools here so errors are caught before server connection
    terminatorTools = new TerminatorTools(TERMINATOR_BASE_URL);
    console.log(JSON.stringify({
      type: "mcp_initialization",
      status: "success",
      component: "TerminatorTools",
      timestamp: new Date().toISOString()
    }));
  } catch (error) {
    console.error(JSON.stringify({
      type: "mcp_initialization",
      status: "error",
      component: "TerminatorTools",
      error: error instanceof Error ? error.message : String(error),
      timestamp: new Date().toISOString()
    }));
    process.exit(1); // Exit if tools can't be initialized
  }

  const transport = new StdioServerTransport();
  try {
    // Connect the server to the transport (stdio in this case)
    await server.connect(transport);
    console.log(JSON.stringify({
      type: "mcp_server_ready",
      status: "connected",
      transport: "stdio",
      timestamp: new Date().toISOString()
    }));
  } catch (error) {
    console.error(JSON.stringify({
      type: "mcp_server_error",
      status: "connection_failed",
      error: error instanceof Error ? error.message : String(error),
      timestamp: new Date().toISOString()
    }));
    process.exit(1); // Exit if connection fails
  }
}

// Run the main function and handle potential errors
main().catch((error) => {
  console.error("Uncaught error in main execution:", error);
  process.exit(1); // Exit on unhandled errors
});
