# summarizer (🧠 Ctrl+J to Clipboard using Ollama)
Simple local AI context summarizer using:
- 🪟 Hotkey detection (Ctrl+J)
- 🪟 Accessibility tree capture
- 🤖 Ollama + Gemma 3
- 📋 Clipboard output
---
## ⚡ Quick Start (Windows)

```bash
git clone https://github.com/mediar-ai/terminator
cd terminator/examples/summarizer
powershell -ExecutionPolicy Bypass -File setup_windows.ps1
cargo build --release