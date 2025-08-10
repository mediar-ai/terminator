# terminator 🤖 Mediar.ai

https://github.com/user-attachments/assets/00329105-8875-48cb-8970-a62a85a9ebd0

<p align="center">
  <a href="https://discord.gg/dU9EBuw7Uq">
    <img src="https://img.shields.io/discord/823813159592001537?color=5865F2&logo=discord&logoColor=white&style=flat-square" alt="Join us on Discord">
  </a>
  <a href="https://docs.screenpi.pe/terminator/introduction">
    <img src="https://img.shields.io/badge/read_the-docs-blue" alt="Docs">
  </a>
  <a href="https://www.youtube.com/@mediar_ai">
    <img src="https://img.shields.io/badge/YouTube-@mediar__ai-FF0000?logo=youtube&logoColor=white&style=flat-square" alt="YouTube @mediar_ai">
  </a>
  <a href="https://crates.io/crates/terminator-rs">
    <img src="https://img.shields.io/crates/v/terminator-rs.svg" alt="Crates.io - terminator-rs">
  </a>
  <a href="https://crates.io/crates/terminator-workflow-recorder">
    <img src="https://img.shields.io/crates/v/terminator-workflow-recorder.svg" alt="Crates.io - workflow recorder">
  </a>
</p>

<p align="center">
  <a href="https://insiders.vscode.dev/redirect?url=vscode%3Amcp%2Finstall%3F%7B%22terminator-mcp-agent%22%3A%7B%22command%22%3A%22npx%22%2C%22args%22%3A%5B%22-y%22%2C%22terminator-mcp-agent%40latest%22%5D%7D%7D">
    <img alt="Install in VS Code" src="https://img.shields.io/badge/VS_Code-VS_Code?style=flat-square&label=Install%20MCP&color=0098FF">
  </a>
  <a href="https://insiders.vscode.dev/redirect?url=vscode-insiders%3Amcp%2Finstall%3F%7B%22terminator-mcp-agent%22%3A%7B%22command%22%3A%22npx%22%2C%22args%22%3A%5B%22-y%22%2C%22terminator-mcp-agent%40latest%22%5D%7D%7D">
    <img alt="Install in VS Code Insiders" src="https://img.shields.io/badge/VS_Code_Insiders-VS_Code_Insiders?style=flat-square&label=Install%20MCP&color=24bfa5">
  </a>
  <a href="https://cursor.com/install-mcp?name=terminator-mcp-agent&config=eyJjb21tYW5kIjoibnB4IiwiYXJncyI6WyIteSIsInRlcm1pbmF0b3ItbWNwLWFnZW50QGxhdGVzdCJdfQ==">
    <img alt="Install in Cursor" src="https://img.shields.io/badge/Cursor-Cursor?style=flat-square&label=Install%20MCP&color=22272e">
  </a>
</p>

⚠️ News: 
- 08/25 shipped tons of new features & bug fixes: run NodeJS SDK in YAML workflows, run JS in browser, record OS events and genereate YAML workflows directly in MCP end-to-end, etc.
- 08/25 [we raised $2.8m to give AI hands to every desktop](https://x.com/louis030195/status/1948745185178914929) ⚠


> Computer use SDK for building agents that learn from human screen recordings. Accessibility-first. Cross-platform (Windows/macOS/Linux).

There are three paths to train deterministic workflows with AI fallback:

1.  **AI-Assisted Workflow Building**: Use an MCP client like [Cursor](https://cursor.com) to iteratively build and test complex workflows with an AI assistant in a human-in-the-loop process.
2.  **Record Human Baselines**: Use our open-source tools to record a human demonstrating a task (our MCP has a recording tool). This generates a baseline workflow that can be refined and automated.
3.  **Enterprise-Grade Recording**: For businesses needing scalable, >95% success rate automation workflows, our [b2b product](https://mediar.ai) provides the most robust solution.

For detailed instructions on building with AI agents through MCP client, see our [**Terminator MCP Agent README**](terminator-mcp-agent/README.md).

## ⚡ Quick Start: Programmatic Control

### 🐍 Python

```python
import terminator

# Control applications programmatically
desktop = terminator.Desktop()
desktop.open_application('calc')
desktop.locator('name:Seven').click()
desktop.locator('name:Plus').click()
desktop.locator('name:Three').click()
desktop.locator('name:Equals').click()
# Result: 10 appears in calculator
```

**Installation:**

```bash
pip install terminator.py
```

### 🟦 TypeScript / Node.js

```typescript
const { Desktop } = require("terminator.js");

// Async/await for modern control flow
const desktop = new Desktop();
await desktop.openApplication("notepad");
await desktop.locator("name:Edit").typeText("Hello from TypeScript!");
await desktop.pressKey("{Ctrl}s"); // Save
```

**Installation:**

```bash
npm install terminator.js
# or: bun add terminator.js
```

## Feature Support

While Terminator aims for full cross-platform support, current capabilities vary by OS. Windows is the primary development target and has the most complete feature set.

| Feature                      | Windows | macOS | Linux | Notes                                                |
| ---------------------------- | :-----: | :---: | :---: | ---------------------------------------------------- |
| **Core Automation**          |         |       |       |                                                      |
| Element Locators             |   ✅    |  🟡   |  🟡   | Find elements by `name`, `role`, `window`, etc.      |
| UI Actions (`click`, `type`) |   ✅    |  🟡   |  🟡   | Core interactions with UI elements.                  |
| Application Management       |   ✅    |  🟡   |  🟡   | Launch, list, and manage applications.               |
| Window Management            |   ✅    |  🟡   |  🟡   | Get active window, list windows.                     |
| **Advanced Features**        |         |       |       |                                                      |
| Workflow Recording           |   ✅    |  ❌   |  ❌   | Record human workflows for deterministic automation. |
| Monitor Management           |   ✅    |  🟡   |  🟡   | Multi-display support.                               |
| Screen & Element Capture     |   ✅    |  ✅   |  🟡   | Take screenshots of displays or elements.            |
| **Language Bindings**        |         |       |       |                                                      |
| Python (`terminator.py`)     |   ✅    |  ✅   |  ✅   | `pip install terminator.py`                          |
| TypeScript (`terminator.js`) |   ✅    |  ✅   |  ✅   | `npm i terminator.js`                                |
| MCP (`terminator-mcp-agent`) |   ✅    |  ✅   |  ✅   | `npx -y terminator-mcp-agent --add-to-app [app]`     |
| Rust (`terminator-rs`)       |   ✅    |  ✅   |  ✅   | `cargo add terminator-rs`                            |

**Legend:**

- ✅: **Supported** - The feature is stable and well-tested.
- 🟡: **Partial / Experimental** - The feature is in development and may have limitations.
- ❌: **Not Supported** - The feature is not yet available on this platform.

## 🕵️ How to Inspect Accessibility Elements (like `name:Seven`)

To create reliable selectors (e.g. `name:Seven`, `role:Button`, `window:Calculator`), you need to inspect the Accessibility Tree of your OS. Here's how to explore UI elements on each platform:

### 🪟 Windows

- **Tool:** [Accessibility Insights for Windows](https://accessibilityinsights.io/downloads/)
- **Alt:** [Inspect.exe](https://learn.microsoft.com/en-us/windows/win32/winauto/inspect-objects) (comes with Windows SDK)
- **Usage:** Open the app you want to inspect → launch Accessibility Insights → hover or use keyboard navigation to explore the UI tree (Name, Role, ControlType, AutomationId).

> These tools show you the `Name`, `Role`, `ControlType`, and other metadata used in Terminator selectors.

---

### 🍎 macOS

- **Tool:** [Accessibility Inspector](https://developer.apple.com/documentation/xcode/accessibility_inspector)
- **Usage:** Comes with Xcode → Open `Xcode > Open Developer Tool > Accessibility Inspector` → Use the target icon to explore UI elements on screen.

---

### 🐧 Linux

- **Tool:** [Accerciser](https://wiki.gnome.org/Apps/Accerciser)
- **Install:**

  ```bash
  sudo apt install accerciser
  ```

- **Usage:** Launch Accerciser → Select the window/app → Browse the accessible widget tree.

---

### 💡 Tip

Once you identify the structure of your UI:

```python
# Sample pattern
desktop.locator('window:Calculator')
       .locator('role:Button')
       .locator('name:Seven')
```

You can build and debug selector paths incrementally using `.locator()` chaining.

## Explore Further

- [https://github.com/mediar-ai/terminator/examples](https://github.com/mediar-ai/terminator/tree/main/examples)

## contributing

contributions are welcome! please feel free to submit issues and pull requests. many parts are experimental, and help is appreciated. join our [discord](https://discord.gg/dU9EBuw7Uq) to discuss.

## businesses

if you want desktop automation at scale for your business, [let's talk](https://mediar.ai)
