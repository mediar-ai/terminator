#!/usr/bin/env node

const { spawn, execSync } = require("child_process");
const path = require("path");
const fs = require("fs");
const os = require("os");
const readline = require("readline");
const config = require("./config");
const { supportedClients } = require("./config");

function getPlatformInfo() {
  const platform = process.platform;
  const arch = process.arch;
  if (platform === "win32" && arch === "x64")
    return {
      pkg: "terminator-mcp-win32-x64-msvc",
      bin: "terminator-mcp-agent.exe",
      npmDir: "win32-x64-msvc",
    };
  if (platform === "linux" && arch === "x64")
    return {
      pkg: "terminator-mcp-linux-x64-gnu",
      bin: "terminator-mcp-agent",
      npmDir: "linux-x64-gnu",
    };
  if (platform === "darwin" && arch === "x64")
    return {
      pkg: "terminator-mcp-darwin-x64",
      bin: "terminator-mcp-agent",
      npmDir: "darwin-x64",
    };
  if (platform === "darwin" && arch === "arm64")
    return {
      pkg: "terminator-mcp-darwin-arm64",
      bin: "terminator-mcp-agent",
      npmDir: "darwin-arm64",
    };
  throw new Error(`Unsupported platform: ${platform} ${arch}`);
}

function addToApp(app) {
  try {
    const client = (app || "").toLowerCase();
    const mcpServer = {
      command: "npx",
      args: ["-y", "terminator-mcp-agent"],
    };
    const currentConfig = config.readConfig(client);
    currentConfig.mcpServers = currentConfig.mcpServers || {};
    currentConfig.mcpServers["terminator-mcp-agent"] = mcpServer;
    config.writeConfig(currentConfig, client);
    console.log(`Configured MCP for ${client}`);
  } catch (e) {
    console.error(`Failed to configure MCP for ${app}:`, e.message);
    process.exit(1);
  }
}

const argv = process.argv.slice(2);

if (argv.includes("--add-to-app")) {
  const appIndex = argv.indexOf("--add-to-app") + 1;
  const app =
    argv[appIndex] && !argv[appIndex].startsWith("--")
      ? argv[appIndex]
      : undefined;
  if (!app) {
    // Interactive prompt
    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout,
    });
    console.log("========== Terminator MCP Setup ==========");
    console.log("Which app do you want to configure Terminator MCP for?");
    console.log("");
    const pad = (n) =>
      String(n).padStart(String(supportedClients.length).length, " ");
    supportedClients.forEach((client, idx) => {
      console.log(`  ${pad(idx + 1)}. ${client.label}`);
    });
    console.log("");
    rl.question(
      `Enter your choice (1-${supportedClients.length}): `,
      (answer) => {
        const idx = parseInt(answer.trim(), 10) - 1;
        if (isNaN(idx) || idx < 0 || idx >= supportedClients.length) {
          console.error("Invalid choice. Skipping app configuration.");
          rl.close();
          process.exit(1);
        }
        const selectedApp = supportedClients[idx].key;
        rl.close();
        addToApp(selectedApp);
        process.exit(0);
      },
    );
    return;
  } else {
    addToApp(app);
    process.exit(0);
  }
} else {
  // Default: run the agent and forward arguments
  const packageInfo = require('./package.json');

  // Display version banner
  console.error(`🤖 Terminator MCP Agent v${packageInfo.version}`);
  console.error(`📦 Platform: ${process.platform}-${process.arch}`);
  console.error(`🚀 Starting MCP server...`);
  console.error(''); // Empty line for readability

  const { pkg, bin, npmDir } = getPlatformInfo();
  let binary;

  // 1. Try local build (for dev)
  const localPath = path.join(__dirname, "npm", npmDir, bin);
  if (fs.existsSync(localPath)) {
    binary = localPath;
    console.error(`🔧 Using local binary: ${path.relative(process.cwd(), binary)}`);
  } else {
    // 2. Try installed npm package
    try {
      binary = require.resolve(pkg);
      console.error(`📦 Using npm package: ${pkg}`);
    } catch (e) {
      console.error(`❌ Failed to find platform binary: ${pkg}`);
      process.exit(1);
    }
  }
  console.error(''); // Empty line before starting

  // Filter out --start if it exists, as it's for the wrapper script
  const agentArgs = argv.filter((arg) => arg !== "--start");

  const child = spawn(binary, agentArgs, {
    stdio: ["pipe", "pipe", "pipe"],
    shell: false,
    detached: process.platform !== "win32",
  });

  process.stdin.pipe(child.stdin);
  child.stdout.pipe(process.stdout);
  child.stderr.pipe(process.stderr);

  function killProcess(proc) {
    if (!proc) return;
    const pid = proc.pid;
    if (process.platform === "win32") {
      try {
        execSync(`taskkill /PID ${pid} /T /F`);
      } catch (e) { }
    } else {
      try {
        process.kill(-pid, "SIGKILL");
      } catch (e) { }
    }
  }

  let shuttingDown = false;
  function shutdown() {
    if (shuttingDown) return;
    shuttingDown = true;
    if (child && !child.killed) {
      if (child.stdin) child.stdin.end();
      const termTimeout = setTimeout(() => {
        if (!child.killed) {
          if (process.platform === "win32") {
            killProcess(child);
          } else {
            try {
              process.kill(child.pid, "SIGTERM");
            } catch (e) { }
            setTimeout(() => {
              if (!child.killed) killProcess(child);
            }, 2000);
          }
        }
      }, 2000);
      child.on("exit", () => clearTimeout(termTimeout));
    }
    process.exit();
  }

  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
  process.on("exit", shutdown);

  child.on("exit", (code) => {
    if (code !== 0) {
      console.error(`[MCP exited with code ${code}]`);
    }
    process.exit(code);
  });
}
