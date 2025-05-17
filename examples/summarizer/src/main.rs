use anyhow::{Context, Result};
use arboard::Clipboard;
use terminator::{Desktop, Selector, UIElement};
use tracing::{info, warn, Level};
use std::{
    io::{self, Write},
    process::Command,
    thread,
    time::Duration,
};

fn run_ollama(prompt: &str) -> Result<String> {
    let output = Command::new("ollama")
        .args(["run", "gemma3", prompt])
        .output()
        .context("Failed to execute Ollama command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Ollama execution failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn extract_numbered_points(summary: &str) -> Vec<String> {
    summary
        .lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty()
                && !line.starts_with("---")
                && !line.starts_with("Okay")
                && !line.starts_with("Would you like")
                && (line.starts_with(|c: char| c.is_ascii_digit()) || line.starts_with('-'))
        })
        .map(|line| line.trim().trim_matches('"').to_string())
        .collect()
}

async fn resolve_window(desktop: &Desktop) -> Result<UIElement> {
    // Try browser first
    match desktop.get_current_browser_window().await {
        Ok(win) => Ok(win),
        Err(e) => {
            warn!("Not a browser window: {}. Falling back to active window.", e);
            desktop.focused_element()
                .context("Failed to get focused element")
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Initializing Terminator desktop automation");
    let desktop = Desktop::new(false, true)
        .await
        .context("Failed to initialize Desktop")?;

    info!("⏳ Waiting for activation trigger...");
    thread::sleep(Duration::from_secs(5));

    info!("⌨ Hotkey triggered — capturing UI context");

    // Pick the right window (browser or active window)
    let window = resolve_window(&desktop)
        .await
        .context("Failed to determine which window to capture")?;

    // Get all editable elements
    let elements = window
        .locator(Selector::Role {
            role: "edit".into(),
            name: None,
        })
        .context("Failed to create locator")?
        .all(None, None)
        .await
        .context("Failed to find editable elements")?;

    let mut ui_context = String::new();

    // Window metadata via attributes
    let wattr = window.attributes();
    let wname = wattr.name.clone().unwrap_or_else(|| "Unnamed window".into());
    ui_context.push_str(&format!("Window: {} [{}]\n", wname, wattr.role));

    // Analyze each editable field
    for el in elements {
        if el.is_keyboard_focusable().unwrap_or(false) {
            let a = el.attributes();
            let name = a.name.clone().unwrap_or_else(|| "[unnamed]".into());
            ui_context.push_str(&format!("\n- {} [{}]\n", name, a.role));
            if let Some(val) = &a.value {
                ui_context.push_str(&format!("  Current value: {}\n", val));
            }
        }
    }

    // Build and send the prompt
    let prompt = format!(
        r#"
Application Context Analysis:
{ui_context}

Please provide:
1. User activity summary
2. Potential assistance opportunities
3. Recommended actions
"#,
        ui_context = ui_context
    );

    info!("🤖 Querying AI model...");
    let summary = run_ollama(&prompt).context("AI query failed")?;

    // Copy to clipboard and print
    let mut clipboard = Clipboard::new().context("Clipboard init failed")?;
    clipboard.set_text(summary.clone()).context("Copy failed")?;
    info!("✅ Results copied to clipboard");
    println!("{}", summary);

    // Refinement loop
    let points = extract_numbered_points(&summary);
    if !points.is_empty() {
        loop {
            println!("\n🔍 Refinement Options:");
            for (i, p) in points.iter().enumerate() {
                println!("{}. {}", i + 1, p);
            }
            println!("0. Exit");

            print!("> ");
            io::stdout().flush().context("Flush failed")?;
            let mut input = String::new();
            io::stdin().read_line(&mut input).context("Read failed")?;

            match input.trim() {
                "0" => break,
                n => {
                    if let Ok(idx) = n.parse::<usize>() {
                        if idx > 0 && idx <= points.len() {
                            let refined = run_ollama(&format!(
                                "Expand on this point in detail:\n{}",
                                points[idx - 1]
                            ))
                            .context("Refinement query failed")?;
                            println!("\n✨ Refined Analysis:\n{}", refined);
                            
                            // Copy refined analysis to clipboard
                            clipboard.set_text(refined.clone()).context("Copy failed")?;
                            info!("✅ Refined results copied to clipboard");
                        }
                    }
                }
            }
        }
    }

    Ok(())
}