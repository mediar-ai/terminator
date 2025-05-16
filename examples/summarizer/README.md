# summarizer (🧠 Ctrl+J to Clipboard using Ollama)

Local AI context summarizer using:
- 🪟 Ctrl+J hotkey detection
- 🪟 Accessibility tree capture (real UI Automation)
- 🤖 Ollama with Gemma 3 or local LLM
- 📋 Clipboard output

---

## ⚡ Quick Start (Windows)

```bash
git clone https://github.com/mediar-ai/terminator
cd terminator/examples/summarizer
powershell -ExecutionPolicy Bypass -File setup_windows.ps1
cargo build --release
