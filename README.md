# terminator 🤖




https://github.com/user-attachments/assets/00329105-8875-48cb-8970-a62a85a9ebd0



<p style="text-align: center;">
    <a href="https://discord.gg/dU9EBuw7Uq">
        <img src="https://img.shields.io/discord/823813159592001537?color=5865F2&logo=discord&logoColor=white&style=flat-square" alt="Join us on Discord">
    </a>
    <a href="https://docs.screenpi.pe/terminator/introduction">
        <img src="https://img.shields.io/badge/read_the-docs-blue" alt="docs">
    </a>
</p>

>Automate desktop apps like a browser. AI-native GUI automation for Windows, MacOS, Linux. Fast, reliable, agent-ready.

## ⚡ TL;DR — Hello World Example

> Skip the boilerplate. This is the fastest way to feel the magic.

### 🐍 Python

```bash
pip install maturin
cd bindings/python
maturin develop
```

```python
import terminator
desktop = terminator.Desktop()
desktop.open_application('calc')
seven = desktop.locator('name:Seven')
seven.click()
```

### 🟦 TypeScript / Node.js

```bash
cd bindings/nodejs
npm install
npm run build
```

```ts
const { Desktop } = require('../bindings/nodejs');
const desktop = new Desktop();
await client.openApplication('notepad')
await client.locator('name:Edit').typeText('hello world')
```

### 🧠 What is Terminator?
Terminator is the Playwright-style SDK for automating Windows GUI apps.

- 🪟 Built for Windows, works on macOS (partial)
- 🧠 Designed for AI agents, not humans
- ⚡ Uses OS-level accessibility (not vision)
- 🧩 TS, Python, and Rust support
- 📈 80ms UI scans, 10000x speedup via compiled workflows

## Benchmarks

The [benchmark test](./terminator/src/tests/e2e_tests.rs) illustrates how fast Terminator can query the UI. It finds all edit elements in about **80&nbsp;ms**, showcasing a big speed advantage over vision-based tools.

This [form-filling app](https://www.mediar.ai/) can read & fill 1000 inputs forms as soon as you see them in <1s end-to-end using Gemini.

## Demos

Check out Terminator in action:

- [📹 Desktop Copilot that autocompletes your work in real time](https://www.youtube.com/watch?v=FGywvWJY7wc)
- [📹 AI Agent that process 100 insurance claims in 5 minutes](https://www.youtube.com/watch?v=6wMNNQFj_dw)
- [📹 Technical Overview Video](https://youtu.be/ycS9G_jpl04)
- [📹 Technical Overview: PDF to Windows Legacy App Form](https://www.youtube.com/watch?v=CMw3iexyCMI)

## Documentation

For detailed information on features, installation, usage, and the API, please visit the **[Official Documentation](https://docs.screenpi.pe/terminator/introduction)**.

## Explore Further

-   **Vercel AI SDK Example:** Learn how to use Terminator with AI in the [PDF-to-Form example](https://github.com/mediar-ai/terminator/tree/main/examples/pdf-to-form).
-   **MCP:** Discover [how to Vibe Work using MCP](https://github.com/mediar-ai/terminator/tree/main/mcp).

## Technical Details & Debugging

### Key Dependencies
*   **Windows:** [uiautomation-rs](https://github.com/leexgone/uiautomation-rs)
*   **macOS:** Native macOS Accessibility API (exploring [cidre](https://github.com/yury/cidre) as an alternative)

### Debugging Tools
*   **Windows:**
    *   [Accessibility Insights for Windows](https://accessibilityinsights.io/downloads/)
    *   **FlaUInspect:** A recommended alternative for inspecting UI Automation properties on Windows.
        *   Install: `choco install flauinspect` or download from [FlaUI/FlaUInspect releases](https://github.com/FlaUI/FlaUInspect/releases).
        *   Usage: Launch `FlaUInspect.exe`, hover or click on elements to see properties like `AutomationId`, `Name`, and `ControlType`. This is great for debugging selectors.

## contributing

contributions are welcome! please feel free to submit issues and pull requests. many parts are experimental, and help is appreciated. join our [discord](https://discord.gg/dU9EBuw7Uq) to discuss.

## businesses 

if you want desktop automation at scale for your business, [let's talk](https://mediar.ai)
