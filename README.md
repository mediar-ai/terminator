# [Want a turnkey automation? mediar.ai](https://mediar.ai)

## [Want No code workflow builder? app.mediar.ai](https://app.mediar.ai)

## 💰 Bounty Developer Program

We have listed a few issues with bounties - ask questions, complete tasks, make money!

Check out our bounty-tagged issues and start contributing today:
- [Example Issue #372 - Add GitHub Workflow to Handle Dependent PR Releases](https://github.com/mediar-ai/terminator/issues/372)
- [View all bounty issues](https://github.com/mediar-ai/terminator/issues?q=is%3Aissue+is%3Aopen+label%3Abounty)

Join our [Discord](https://discord.gg/dU9EBuw7Uq) to discuss bounties and get help getting started.

# demo of terminator drawing in paint

https://github.com/user-attachments/assets/b7bec5df-64a9-4f0e-aba4-1578c11275c7

<p align="center">
  <a href="https://discord.gg/dU9EBuw7Uq">
    <img src="https://img.shields.io/discord/823813159592001537?color=5865F2&logo=discord&logoColor=white&style=flat-square" alt="Join us on Discord">
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
  <a href="https://github.com/mediar-ai/terminator/blob/main/terminator-mcp-agent/README.md#quick-install">
    <img alt="Install in Cursor" src="https://img.shields.io/badge/Cursor-Cursor?style=flat-square&label=Install%20MCP&color=22272e">
  </a>
  <a href="https://insiders.vscode.dev/redirect?url=vscode%3Amcp%2Finstall%3F%7B%22terminator-mcp-agent%22%3A%7B%22command%22%3A%22npx%22%2C%22args%22%3A%5B%22-y%22%2C%22terminator-mcp-agent%22%5D%7D%7D">
    <img alt="Install in VS Code" src="https://img.shields.io/badge/VS_Code-VS_Code?style=flat-square&label=Install%20MCP&color=0098FF">
  </a>
  <a href="https://insiders.vscode.dev/redirect?url=vscode-insiders%3Amcp%2Finstall%3F%7B%22terminator-mcp-agent%22%3A%7B%22command%22%3A%22npx%22%2C%22args%22%3A%5B%22-y%22%2C%22terminator-mcp-agent%22%5D%7D%7D">
    <img alt="Install in VS Code Insiders" src="https://img.shields.io/badge/VS_Code_Insiders-VS_Code_Insiders?style=flat-square&label=Install%20MCP&color=24bfa5">
  </a>
</p>

## 🚀 What's new

- 10/30 Public alpha is live - [n8n for data entry automation in legacy systems](https://www.mediar.ai/)
- 09/26 Terminator was on [Cohere Labs podcast](https://www.youtube.com/watch?v=cfQxlk8KNmY), also [check the slides](https://092025-cohere.mediar.ai/)
- 08/25 Big release — NodeJS SDK in YAML workflows, run JS in browser, OS event recording → YAML generation in MCP, and more
- 08/25 [we raised $2.8m to give AI hands to every desktop](https://x.com/louis030195/status/1948745185178914929) ⚠

> AI-first data entry automation in legacy systems 

## 🧠 Why Terminator

### For Developers

- Create automations that work across any desktop app or browser
- Runs 100x faster than ChatGPT Agents, Claude, Perplexity Comet, BrowserBase, BrowserUse
- \>95% success rate without costly consultant armies (UIPath, Automation Anywhere)
- MIT-licensed — fork it, ship it, no lock-in

We achieve this by pre-training workflows as deterministic code, and calling AI only when recovery is needed.

### For Teams

[Our public beta workflow builder](https://www.mediar.ai/) + managed hosting = n8n for data entry automation in legacy systems:

- Record & map your processes in 5 mins
- Deploy AI to execute them at >95% success rate
- Kill repetitive work without legacy RPA complexity or cost

## 🎯 Choose Your Path

### 🤖 Want AI Automation with Claude Code? (Recommended for Most Users)

Install the Terminator MCP in Claude Code:

```bash
claude mcp add terminator "npx -y terminator-mcp-agent@latest" -s user
```

Or install via the [MCP directory](https://github.com/mediar-ai/terminator/tree/main/terminator-mcp-agent) for other clients (Cursor, VS Code, etc.).

**What this does:** Gives AI assistants the ability to control your desktop and automate tasks across any application.

To create workflows, please [download the desktop app](https://www.mediar.ai/).

## Feature Support

While Terminator aims for full cross-platform support, current capabilities vary by OS. Windows is the primary development target and has the most complete feature set.

| Feature                      | Windows | macOS | Linux | Notes                                                |
| ---------------------------- | :-----: | :---: | :---: | ---------------------------------------------------- |
| **Core Automation**          |         |       |       |                                                      |
| Element Locators             |    ✅    |   🟡   |   🟡   | Find elements by `name`, `role`, `window`, etc.      |
| UI Actions (`click`, `type`) |    ✅    |   🟡   |   🟡   | Core interactions with UI elements.                  |
| Application Management       |    ✅    |   🟡   |   🟡   | Launch, list, and manage applications.               |
| Window Management            |    ✅    |   🟡   |   🟡   | Get active window, list windows.                     |
| **Advanced Features**        |         |       |       |                                                      |
| Browser Automation           |    ✅    |   ✅   |   ✅   | Chrome extension enables browser control.            |
| Workflow Recording           |    ✅    |   ❌   |   ❌   | Record human workflows for deterministic automation. |
| Monitor Management           |    ✅    |   🟡   |   🟡   | Multi-display support.                               |
| Screen & Element Capture     |    ✅    |   ✅   |   🟡   | Take screenshots of displays or elements.            |
| **Language Bindings**        |         |       |       |                                                      |
| Python (`terminator.py`)     |    🟡    |   🟡   |   🟡   | `pip install terminator`                          |
| TypeScript (`@mediar-ai/terminator`) |    ✅    |   ✅   |   ✅   | `npm i @mediar-ai/terminator`                                |
| MCP (`terminator-mcp-agent`) |    ✅    |   ✅   |   ✅   | `npx -y terminator-mcp-agent --add-to-app [app]`     |
| Rust (`terminator-rs`)       |    ✅    |   ✅   |   ✅   | `cargo add terminator-rs`                            |

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

## Explore Further

- **[Examples](https://github.com/mediar-ai/terminator/tree/main/examples)**

## Troubleshooting

For detailed troubleshooting, debugging, and MCP server logs, [send us a message](https://www.mediar.ai/).

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests. many parts are experimental, and help is appreciated. 


