use anyhow::{Context, Result};
use arboard::Clipboard;
use clap::Parser;
use rdev::{listen, Event, EventType, Key};
use std::io::{self, Write};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use terminator::{Desktop, Selector, UIElement};
use tracing::{info, warn, Level};

/// CLI args
#[derive(Parser)]
struct Args {
    /// Ollama model (e.g., gemma3, llama3, phi3)
    #[arg(long, default_value = "gemma3")]
    model: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    let args = Args::parse();

    let is_triggered = Arc::new(Mutex::new(false));
    let trigger_clone = Arc::clone(&is_triggered);
    let ctrl_pressed = Arc::new(Mutex::new(false));
    let ctrl_state = Arc::clone(&ctrl_pressed);

    thread::spawn(move || {
        let _ = listen(move |event: Event| {
            match event.event_type {
                EventType::KeyPress(Key::ControlLeft) | EventType::KeyPress(Key::ControlRight) => {
                    let mut state = ctrl_state.lock().unwrap();
                    *state = true;
                }
                EventType::KeyRelease(Key::ControlLeft) | EventType::KeyRelease(Key::ControlRight) => {
                    let mut state = ctrl_state.lock().unwrap();
                    *state = false;
                }
                EventType::KeyPress(Key::KeyJ) => {
                    let ctrl = *ctrl_state.lock().unwrap();
                    if ctrl {
                        println!("🎯 Ctrl+J pressed!");
                        let mut t = trigger_clone.lock().unwrap();
                        *t = true;
                    }
                }
                _ => {}
            }
        });
    });

    println!("🟢 Listening for Ctrl+J to trigger summarization...");

    let desktop = Desktop::new(false, true)
        .await
        .context("Failed to initialize Terminator Desktop")?;

    loop {
        if *is_triggered.lock().unwrap() {
            summarize_all_windows(&desktop, &args.model).await?;
            *is_triggered.lock().unwrap() = false;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

async fn summarize_all_windows(desktop: &Desktop, model: &str) -> Result<()> {
    println!("⌨ Hotkey triggered — capturing UI context");

    let windows = resolve_windows(desktop).await?;

    let mut ui_context = String::new();
    for window in windows {
        let wattr = window.attributes();
        let wname = wattr.name.clone().unwrap_or_else(|| "Unnamed window".into());
        ui_context.push_str(&format!("\n=== Window: {} [{}] ===\n", wname, wattr.role));

        let editable = window
            .locator(Selector::Role {
                role: "edit".into(),
                name: None,
            })?
            .all(None, None)
            .await?;

        for el in editable {
            if el.is_keyboard_focusable().unwrap_or(false) {
                let a = el.attributes();
                let name = a.name.clone().unwrap_or_else(|| "[unnamed]".into());
                ui_context.push_str(&format!("- {} [{}]\n", name, a.role));
                if let Some(val) = &a.value {
                    ui_context.push_str(&format!("  Value: {}\n", val));
                }
            }
        }
    }

    let prompt = format!(
        r#"
Application Context Analysis:
{ui_context}
Please provide:
1. One-line summary
2. Key user activities (bullets)
3. Suggestions or recommended actions (optional)
"#
    );

    let summary = run_ollama(&prompt, model).context("AI query failed")?;
    let clean_summary = summary.replace("", "").replace("* ", "- ");

    let mut clipboard = Clipboard::new().context("Clipboard init failed")?;
    clipboard.set_text(clean_summary.clone()).context("Copy failed")?;
    println!("{}", clean_summary);
    println!("✅ Results copied to clipboard");

    refinement_loop(&clean_summary, model)?;

    Ok(())
}

async fn resolve_windows(desktop: &Desktop) -> Result<Vec<UIElement>> {
    let browser_names = vec!["chrome", "firefox", "msedge", "brave", "arc", "safari"];
    let mut windows = Vec::new();

    for name in browser_names {
        if let Ok(app) = desktop.application(name) {
            let app_windows = app
                .locator(Selector::Role { role: "window".into(), name: None })?
                .all(None, None)
                .await?;
            windows.extend(app_windows);
        }
    }

    if windows.is_empty() {
        warn!("No browser windows found. Falling back to focused window.");
        if let Ok(focused) = desktop.focused_element() {
            windows.push(focused);
        }
    }

    Ok(windows)
}

fn run_ollama(prompt: &str, model: &str) -> Result<String> {
    let output = Command::new("ollama")
        .args(["run", model, prompt])
        .output()
        .context("Failed to run Ollama")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Ollama failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn extract_numbered_points(summary: &str) -> Vec<String> {
    summary
        .lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty()
                && (line.starts_with(|c: char| c.is_ascii_digit()) || line.starts_with('-'))
        })
        .map(|line| line.trim().trim_matches('"').to_string())
        .collect()
}

fn refinement_loop(summary: &str, model: &str) -> Result<()> {
    let points = extract_numbered_points(summary);

    if points.is_empty() {
        return Ok(());
    }

    loop {
        println!("\n🔍 Refine any point?");
        for (i, point) in points.iter().enumerate() {
            println!("{}. {}", i + 1, point);
        }
        println!("0. Exit");
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim() {
            "0" => break,
            n => {
                if let Ok(idx) = n.parse::<usize>() {
                    if idx > 0 && idx <= points.len() {
                        let prompt = format!("Expand on this point:\n{}", points[idx - 1]);
                        let refined = run_ollama(&prompt, model)?;
                        println!("\n✨ Refined:\n{}", refined);
                        Clipboard::new()?.set_text(refined)?;
                    }
                }
            }
        }
    }

    Ok(())
}


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
>>>>>>> 14aedf7 (made summarizer-example that enable running example easily)
