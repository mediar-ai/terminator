# Check Rust installation
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "🛠️ Installing Rust..."
    iex "& { $(irm https://sh.rustup.rs) } -y"
} else {
    Write-Host "✅ Rust is already installed."
}

# Check Ollama installation
$ollamaPath = "$env:LOCALAPPDATA\Programs\Ollama\ollama.exe"
if (-not (Test-Path $ollamaPath)) {
    Write-Host "🌐 Opening Ollama download page..."
    Start-Process "https://ollama.com/download"
    Write-Host "⚠️ Please install Ollama manually, then rerun this script."
    exit
} else {
    Write-Host "✅ Ollama found."
}

# Pull the Gemma model using correct model name
Write-Host "⬇️ Pulling Ollama model gemma3..."
& $ollamaPath pull gemma3

# Build the Rust binary
Write-Host "🔨 Building Rust summarizer binary..."
# Navigate from examples/summarizer to project root (adjust if your path differs)
Set-Location ../..

cargo build --release -p summarizer

Write-Host '`nBuild complete. Run with:`n    .\target\release\summarizer.exe`n'


