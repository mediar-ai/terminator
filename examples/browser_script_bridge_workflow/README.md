# Browser Script Bridge Workflow (TypeScript)

This workflow exercises the Terminator browser script bridge (`executeBrowserScript`) across success and failure scenarios, ensuring the Chrome extension path works reliably and surfaces errors consistently.

What it covers:
- Open Chrome and navigate to `about:blank`
- Execute inline string scripts (e.g., `document.title`)
- Execute function-based scripts with JSON return (auto-parsed by the Node wrapper)
- Execute scripts from file with `env` parameters
- Handle Promise rejections from browser scripts (maps to PlatformError)
- Handle structured failure objects (e.g., `{ success: false, message: "..." }`)
- Validate bridge retry/reset (simulated transient issue, then success)

Run locally with Terminator CLI (one-liner):

```
cargo run --bin terminator -- mcp run examples/browser_script_bridge_workflow
```

Or point directly to the entry file:

```
cargo run --bin terminator -- mcp run examples/browser_script_bridge_workflow/src/terminator.ts
```

Notes:
- Requires the Terminator Chrome extension installed and active. The CI installs it automatically.
- Type-checking is performed by the CLI using `tsc --noEmit` via bun/npx/tsc.
