use arboard::Clipboard;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn get_ui_tree() -> String {
    // Fetch the title of the currently active window using PowerShell
    let output = Command::new("powershell")
        .args([
            "-Command",
            "Add-Type @\"using System; using System.Runtime.InteropServices; public class WinAPI { [DllImport(\"user32.dll\")] public static extern IntPtr GetForegroundWindow(); [DllImport(\"user32.dll\", SetLastError=true)] public static extern int GetWindowText(IntPtr hWnd, System.Text.StringBuilder text, int count); }\"@; $buffer = New-Object System.Text.StringBuilder 256; [void][WinAPI]::GetWindowText([WinAPI]::GetForegroundWindow(), $buffer, $buffer.Capacity); $buffer.ToString()",
        ])
        .output()
        .expect("❌ Failed to get active window title");

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn run_ollama(prompt: &str) -> String {
    let output = Command::new("ollama")
        .args(["run", "gemma3", prompt])
        .output()
        .expect("❌ Failed to run Ollama");

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn main() {
    println!("⏳ Waiting for Ctrl+J to be pressed...");
    
    // Development placeholder for hotkey: simulate after delay
    thread::sleep(Duration::from_secs(10));
    
    println!("⌨️ Hotkey triggered. Capturing UI tree...");
    let tree = get_ui_tree();

    let prompt = format!(
        "Here's the data from the screen: \"{}\". Please turn this into easy-to-understand context I can use to describe what I'm doing.",
        tree
    );

    println!("🤖 Sending prompt to Ollama...");
    let summary = run_ollama(&prompt);

    let mut clipboard = Clipboard::new().expect("❌ Could not access clipboard");
    clipboard.set_text(summary.clone()).expect("❌ Failed to copy to clipboard");

    println!("✅ Copied summary to clipboard:\n{}", summary);
}
