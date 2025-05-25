# Context Capture Tool for Windows

A Windows CLI application that captures the currently focused application's information, processes it with a local LLM (Ollama with Gemma3), and copies the result to the clipboard for easy sharing with other AI tools.

## Features

- **Global hotkey support** - Press Ctrl+Shift+C (customizable) to capture the current app's UI tree
- **Local AI processing** - Uses Ollama and Gemma3 (or other models) running locally for privacy
- **Clipboard integration** - Automatically copies the AI's summary to clipboard
- **Windows UI Automation** - Uses Windows Accessibility APIs to get detailed UI information

## Prerequisites

1. **Windows 10/11**
   
   This tool is designed primarily for Windows. Though it has fallback code for other platforms, the UI tree capture works best on Windows.

2. **Install Ollama**

   Download and install from [ollama.ai](https://ollama.ai).
   
   After installation, open a command prompt and download the Gemma model:
   ```
   ollama pull gemma:2b
   ```

3. **Install Rust**

   If you don't have Rust installed, get it from [rustup.rs](https://rustup.rs).

## Quick Start

1. Clone the repository:
   ```
   git clone https://github.com/yourusername/terminator.git
   cd terminator\examples\terminator-rust-examples\context-capture
   ```

2. Make sure Ollama is running in the background

3. Run the application using the provided batch file:
   ```
   run.bat
   ```
   
   Or run directly with cargo:
   ```
   cargo run --release
   ```

4. Press **Ctrl+Shift+C** while using any application to capture its context

5. The processed description will be automatically copied to your clipboard

6. Paste the result into any AI chat tool

## Command-line Options

```
context-capture.exe [OPTIONS]
```

- `-m, --model <MODEL>`: Specify the Ollama model (default: "gemma:2b")
- `-s, --system-prompt <PROMPT>`: Custom system prompt for context generation
- `-h, --hotkey <HOTKEY>`: Custom hotkey combination (default: "ctrl+shift+c")

Example with custom settings:
```
cargo run --release -- --model mistral:7b --hotkey "alt+shift+x"
```

## How It Works

1. The application uses Windows UI Automation APIs to capture the UI tree of the focused application
2. When you press the configured hotkey, it gets the window hierarchy, control types, and properties
3. This structured data is sent to the local Ollama instance with Gemma model
4. The model transforms the technical UI data into a human-readable description
5. The result is copied to the clipboard for easy sharing

## Example Output

When using a web browser, the output might look like:

```
You're currently browsing GitHub.com in Microsoft Edge. The page shows a repository 
named "terminator" with several tabs open including Code, Issues, and Pull Requests. 
You're viewing a pull request discussion thread with 3 comments. The sidebar shows 
the repository has 45 stars and 12 forks. This appears to be a development tool 
related to UI automation.
```

## Build for Distribution

To create a standalone executable:

```
cargo build --release
```

The executable will be at `target\release\context-capture.exe`

## License

[MIT](LICENSE)

## Contributing

Contributions are welcome! Feel free to submit a Pull Request.
