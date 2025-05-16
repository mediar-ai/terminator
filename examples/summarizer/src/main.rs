use arboard::Clipboard;
use std::{process::Command, thread, time::Duration};

/// Gets the title of the currently active window (basic placeholder)
fn get_ui_context() -> String {
    let output = Command::new("powershell")
        .args(["(Get-Process | Where-Object {$_.MainWindowTitle }).MainWindowTitle"])
        .output()
        .expect("❌ Failed to get active window title");

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Sends the prompt to Ollama using gemma3 (or any model installed)
fn run_ollama(prompt: &str) -> String {
    let output = Command::new("ollama")
        .args(["run", "gemma3", prompt])
        .output()
        .expect("❌ Failed to run Ollama");

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn main() {
    println!("⏳ Waiting for Ctrl+J to be pressed...");
    // Simulate Ctrl+J for testing after delay
    thread::sleep(Duration::from_secs(5));

    println!("⌨️ Hotkey triggered. Capturing UI context...");
    let context = get_ui_context();

    let prompt = format!(
        "Here's the data from the current screen: \"{}\". Please turn this into easy-to-understand context I can feed to another AI about what I’m currently doing.",
        context
    );

    println!("🤖 Sending prompt to Ollama...");
    let summary = run_ollama(&prompt);

    let mut clipboard = Clipboard::new().unwrap();
    clipboard.set_text(summary.clone()).unwrap();

    println!("✅ Copied summary to clipboard:\n{}", summary);
}
