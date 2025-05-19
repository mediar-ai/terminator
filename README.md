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

**Terminator** is the fastest, AI-first, computer use SDK. It's designed to interact with native GUI applications on Windows using a Playwright-like API, like parsing a website. By leveraging OS-level accessibility APIs, Terminator is significantly faster and more reliable for AI agents than vision-based approaches, and can interact with background applications.

> **⚠️ Experimental ⚠️:** Terminator is under active development. Expect bugs and breaking changes. Please report issues – we aim to fix them quickly!

## OS Support

| Operating System | Support Status        | Key Characteristics                                                     |
|------------------|-----------------------|-------------------------------------------------------------------------|
| Windows          | ✅ **Active Focus**   | Full features, best performance, actively developed & documented.       |
| macOS            | 🟡 **Partial**        | Core functionality available; community-driven, less documented.        |
| Linux            | ❌ **No Support**     | Not currently supported.                                                |

## Key Features

*   **AI-First & Agentic:** Built from the ground up for modern AI agents and workflows.
*   **Blazing Fast & Reliable:** Uses OS-level accessibility APIs, making it much faster and more robust than vision-based tools.
*   **Playwright-Style API:** Offers a familiar, powerful, and developer-friendly interface.
*   **Cross-Platform (Windows Focus):** Automate native GUI applications on Windows (primary) and macOS.
*   **Deep UI Introspection:** Enables detailed understanding and control of complex UI elements.
*   **Background App Interaction:** Capable of interacting with applications even if they are not in focus.

## Demos

Check out Terminator in action:

- [📹 Desktop Copilot that autocompletes your work in real time](https://www.youtube.com/watch?v=FGywvWJY7wc)
- [📹 AI Agent that process 100 insurance claims in 5 minutes](https://www.youtube.com/watch?v=6wMNNQFj_dw)
- [📹 Technical Overview Video](https://youtu.be/ycS9G_jpl04)
- [📹 Technical Overview: PDF to Windows Legacy App Form](https://www.youtube.com/watch?v=CMw3iexyCMI)

## Documentation

For detailed information on features, installation, usage, and the API, please visit the **[Official Documentation](https://docs.screenpi.pe/terminator/introduction)**.

## Quick Start

Get up and running with Terminator:

1.  **Clone the repo:**
    ```bash
    git clone https://github.com/mediar-ai/terminator
    cd terminator
    ```
2.  **Set up the server:**
    *   **Windows:** Download & unzip the pre-built server using PowerShell:
        ```powershell
        powershell -ExecutionPolicy Bypass -File .\setup_windows.ps1
        ```
    *   **macOS:** Compile the server using Rust/Cargo (ensure Rust and Xcode Command Line Tools are installed):
        ```bash
        cargo build --release --package server
        ```
3.  **Run the server:**
    *   **Windows:**
        ```powershell
        ./server_release/server.exe --debug
        ```
    *   **macOS:**
        ```bash
        ./target/release/examples/server --debug
        ```
4.  **Run an example client (in a separate terminal):**
    Navigate to the example directory, install dependencies, and run:
    ```bash
    cd examples/hello-world
    npm i
    npm run dev
    # Then, open http://localhost:3000 in your browser
    ```

For more details, see the [Getting Started Guide](https://docs.screenpi.pe/terminator/getting-started) in the docs.

## Explore Further

-   **Vercel AI SDK Example:** Learn how to use Terminator with AI in the [PDF-to-Form example](https://github.com/mediar-ai/terminator/tree/main/examples/pdf-to-form).
-   **Login Automator Example:** A minimal [dummy login automation](https://github.com/mediar-ai/terminator/tree/main/examples/login-automator) using the Python SDK.
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

