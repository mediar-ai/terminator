#!/usr/bin/env node

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const args = process.argv.slice(2);
const isRelease = args.includes("--release");
const isDebug = args.includes("--debug");

if (!isRelease && !isDebug) {
  console.error("Specify --release or --debug");
  process.exit(1);
}

const platform = process.platform;
const arch = process.arch;
let target, binName, npmDir;

if (platform === "win32" && arch === "x64") {
  target = "x86_64-pc-windows-msvc";
  binName = "terminator-mcp-agent.exe";
  npmDir = "win32-x64-msvc";
} else if (platform === "linux" && arch === "x64") {
  target = "x86_64-unknown-linux-gnu";
  binName = "terminator-mcp-agent";
  npmDir = "linux-x64-gnu";
} else if (platform === "darwin" && arch === "x64") {
  target = "x86_64-apple-darwin";
  binName = "terminator-mcp-agent";
  npmDir = "darwin-x64";
} else if (platform === "darwin" && arch === "arm64") {
  target = "aarch64-apple-darwin";
  binName = "terminator-mcp-agent";
  npmDir = "darwin-arm64";
} else {
  console.error(`Unsupported platform: ${platform} ${arch}`);
  process.exit(1);
}

// Kill any existing terminator-mcp-agent process before building
try {
  // windows is annoying w filesystem
  if (platform === "win32") {
    execSync("taskkill /f /im terminator-mcp-agent.exe 2>nul || exit 0", { stdio: "ignore" });
  }
} catch (error) {
  // Ignore errors if process doesn't exist
}

const buildType = isRelease ? "--release" : "";
console.log(`Building for target ${target} (${buildType || "debug"})`);
execSync(
  `cargo build -p terminator-mcp-agent ${buildType} --target ${target}`,
  { stdio: "inherit" },
);

const buildDir = isRelease ? "release" : "debug";
const binaryPath = path.join(
  __dirname,
  "../../target",
  target,
  buildDir,
  binName,
);
const destDir = path.join(__dirname, "../npm", npmDir);
const destPath = path.join(destDir, binName);

if (!fs.existsSync(destDir)) {
  fs.mkdirSync(destDir, { recursive: true });
}

fs.copyFileSync(binaryPath, destPath);
console.log(`Copied ${binaryPath} to ${destPath}`);

if (platform === "darwin") {
  console.log(`Signing ${destPath}...`);
  try {
    execSync(`codesign --force --deep --sign - ${destPath}`);
    console.log("Signing successful.");
  } catch (error) {
    console.error("Signing failed. Please ensure you have Xcode Command Line Tools installed.");
    console.error(error);
    process.exit(1);
  }
}
