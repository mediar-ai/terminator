# summarizer (🧠 Ctrl+J to Clipboard using Ollama)
Simple local AI context summarizer using:
- 🪟 Hotkey detection (Ctrl+J)
- 🪟 Accessibility tree capture
- 🤖 Ollama + Gemma 3
- 📋 Clipboard output
---
## ⚡ Quick Start (Windows)

# Clone the repo and go to the example folder
git clone https://github.com/mediar-ai/terminator
cd examples/summarizer

# Run setup script to install dependencies and configure environment
powershell -ExecutionPolicy Bypass -File setup_windows.ps1

# Build and run the summarizer CLI
cargo build --release
./target/release/summarizer.exe
