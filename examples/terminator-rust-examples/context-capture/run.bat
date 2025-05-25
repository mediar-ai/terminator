@echo off
echo Starting Context Capture Tool...
echo.
echo Make sure Ollama is running and has the Gemma model installed.
echo If not, run: ollama pull gemma:2b
echo.
echo Press Ctrl+Shift+C to capture the current application's context.
echo.
cargo run --release
