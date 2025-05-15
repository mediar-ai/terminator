# Check Rust
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  Write-Host "🛠️ Installing Rust..."
  iex "& { $(irm https://sh.rustup.rs) } -y"
}

# Check Ollama
$ollamaPath = "$env:LOCALAPPDATA\Programs\Ollama\ollama.exe"
if (-not (Test-Path $ollamaPath)) {
  Write-Host "🌐 Opening Ollama download page..."
  Start-Process "https://ollama.com/download"
  Write-Host "⚠️ Please install Ollama manually, then rerun this script."
  exit
}

# Pull the Gemma model using correct model name
& $ollamaPath pull gemma3

# Build the Rust binary
Write-Host "🔨 Building Rust project..."
cd ../..  # go from examples/summarizer to project root
cargo build --release -p summarizer

Write-Host "`n✅ Build complete. Run with:`n    .\\target\\release\\summarizer.exe`n"
