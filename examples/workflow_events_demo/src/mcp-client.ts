/**
 * MCP Client Demo - Receiving Workflow Events
 *
 * This example demonstrates how to build an MCP client that connects to the
 * terminator-mcp-agent and receives real-time workflow events.
 *
 * Events are received via two MCP notification types:
 * 1. notifications/progress - Progress updates with current/total/message
 * 2. notifications/message - Structured log messages with event data
 *
 * Run: bun run src/mcp-client.ts
 */

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import * as path from "path";

// Event types matching the workflow SDK
interface ProgressEvent {
  progressToken: string;
  progress: number;
  total?: number;
  message?: string;
}

interface LoggingEvent {
  level: "debug" | "info" | "warning" | "error";
  logger?: string;
  data: any;
}

// Pretty print helpers
const colors = {
  reset: "\x1b[0m",
  bright: "\x1b[1m",
  dim: "\x1b[2m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  blue: "\x1b[34m",
  magenta: "\x1b[35m",
  cyan: "\x1b[36m",
  red: "\x1b[31m",
};

function formatProgress(event: ProgressEvent): string {
  const percent = event.total ? Math.round((event.progress / event.total) * 100) : "?";
  const bar = event.total
    ? "█".repeat(Math.round((event.progress / event.total) * 20)).padEnd(20, "░")
    : "░".repeat(20);

  return `${colors.cyan}[${bar}] ${percent}%${colors.reset} ${event.message || ""}`;
}

function formatLogEvent(event: LoggingEvent): string {
  const levelColors: Record<string, string> = {
    debug: colors.dim,
    info: colors.blue,
    warning: colors.yellow,
    error: colors.red,
  };

  const color = levelColors[event.level] || colors.reset;
  const logger = event.logger ? `[${event.logger}]` : "";

  // Handle structured event data
  if (event.data && typeof event.data === "object") {
    const eventType = event.data.type;
    switch (eventType) {
      case "step_started":
        return `${colors.green}▶ Step ${event.data.step}/${event.data.total || "?"}: ${event.data.name}${colors.reset}`;
      case "step_completed":
        return `${colors.green}✓ Completed: ${event.data.name} (${event.data.duration_ms}ms)${colors.reset}`;
      case "step_failed":
        return `${colors.red}✗ Failed: ${event.data.name} - ${event.data.error}${colors.reset}`;
      case "screenshot":
        return `${colors.magenta}📷 Screenshot: ${event.data.annotation || event.data.path}${colors.reset}`;
      default:
        if (event.data.key && event.data.value !== undefined) {
          return `${colors.cyan}📊 Data: ${event.data.key} = ${JSON.stringify(event.data.value)}${colors.reset}`;
        }
        if (event.data.message) {
          return `${color}${logger} ${event.data.message}${colors.reset}`;
        }
        return `${color}${logger} ${JSON.stringify(event.data)}${colors.reset}`;
    }
  }

  return `${color}${logger} ${event.data}${colors.reset}`;
}

async function main() {
  console.log(`${colors.bright}=== MCP Workflow Events Demo ===${colors.reset}\n`);

  // Find the MCP agent binary
  const mcpAgentPath = process.env.MCP_AGENT_PATH || "terminator-mcp-agent";

  console.log(`Starting MCP agent: ${mcpAgentPath}\n`);

  // Create transport to the MCP agent
  const transport = new StdioClientTransport({
    command: mcpAgentPath,
    args: [],
    env: {
      ...process.env,
      RUST_LOG: "info",
      LOG_LEVEL: "debug",
    },
  });

  // Create client
  const client = new Client(
    {
      name: "workflow-events-demo",
      version: "1.0.0",
    },
    {
      capabilities: {},
    }
  );

  // Set up notification handlers BEFORE connecting
  // Use fallback handler approach for notifications
  client.fallbackNotificationHandler = async (notification: { method: string; params?: unknown }) => {
    if (notification.method === "notifications/progress") {
      const params = notification.params as ProgressEvent;
      console.log(formatProgress(params));
    } else if (notification.method === "notifications/message") {
      const params = notification.params as LoggingEvent;
      console.log(formatLogEvent(params));
    }
  };

  // Connect
  console.log("Connecting to MCP agent...\n");
  await client.connect(transport);
  console.log(`${colors.green}Connected!${colors.reset}\n`);

  // List available tools
  const tools = await client.listTools();
  console.log(
    `Available tools: ${tools.tools.map((t) => t.name).join(", ")}\n`
  );

  // Execute the workflow
  console.log(`${colors.bright}--- Executing Workflow ---${colors.reset}\n`);

  try {
    // Get the workflow path
    const workflowPath = path.resolve(
      process.cwd(),
      "examples/workflow_events_demo"
    );

    const result = await client.callTool({
      name: "execute_sequence",
      arguments: {
        url: `file://${workflowPath}`,
        inputs: {
          message: "Hello from the MCP client! 🚀",
          simulateError: false,
        },
      },
    });

    console.log(`\n${colors.bright}--- Workflow Result ---${colors.reset}`);
    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    console.error(`${colors.red}Error executing workflow:${colors.reset}`, error);
  }

  // Cleanup
  await client.close();
  console.log(`\n${colors.dim}Client disconnected.${colors.reset}`);
}

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
