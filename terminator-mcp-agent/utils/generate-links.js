#!/usr/bin/env node

// VS Code and VS Code Insiders config
const configObjVSCode = {
  "terminator-mcp-agent": {
    command: "npx",
    args: ["-y", "terminator-mcp-agent"],
  },
};
const configJSONVSCode = JSON.stringify(configObjVSCode);

// Cursor config (flat, not nested)
const configObjCursor = {
  command: "npx",
  args: ["-y", "terminator-mcp-agent"],
};
const configJSONCursor = JSON.stringify(configObjCursor);
const configBase64Cursor = Buffer.from(configJSONCursor).toString("base64");

const cursorWebUrl = `https://cursor.com/install-mcp?name=terminator-mcp-agent&config=${encodeURIComponent(configBase64Cursor)}`;

const urlForVSCode = `vscode:mcp/install?${encodeURIComponent(configJSONVSCode)}`;
const urlForVSCodeGithub = `https://insiders.vscode.dev/redirect?url=${encodeURIComponent(urlForVSCode)}`;

const urlForVSCodeInsiders = `vscode-insiders:mcp/install?${encodeURIComponent(configJSONVSCode)}`;
const urlForVSCodeInsidersGithub = `https://insiders.vscode.dev/redirect?url=${encodeURIComponent(urlForVSCodeInsiders)}`;

// Just log the raw URLs, in order
console.log(urlForVSCodeGithub);
console.log(urlForVSCodeInsidersGithub);
console.log(cursorWebUrl);
