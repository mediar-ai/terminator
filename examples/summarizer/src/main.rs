use anyhow::{anyhow, Context, Result};
use arboard::Clipboard;
use clap::Parser;
use rdev::{listen, Event, EventType, Key};
use std::{
    collections::HashMap,
    io::{self, Write},
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use terminator::{Desktop, Selector, UIElement};
use tracing::{info, warn, error, Level};

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

    let cache: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

    thread::spawn(move || {
        if let Err(e) = listen(move |event: Event| {
            match event.event_type {
                EventType::KeyPress(Key::ControlLeft) | EventType::KeyPress(Key::ControlRight) => {
                    if let Ok(mut ctrl) = ctrl_state.lock() {
                        *ctrl = true;
                    }
                }
                EventType::KeyRelease(Key::ControlLeft) | EventType::KeyRelease(Key::ControlRight) => {
                    if let Ok(mut ctrl) = ctrl_state.lock() {
                        *ctrl = false;
                    }
                }
                EventType::KeyPress(Key::KeyJ) => {
                    if let Ok(ctrl) = ctrl_state.lock() {
                        if *ctrl {
                            info!("🎯 Ctrl+J pressed!");
                            if let Ok(mut triggered) = trigger_clone.lock() {
                                *triggered = true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }) {
            error!("Error listening to keyboard events: {:?}", e);
        }
    });

    println!("🟢 Listening for Ctrl+J to trigger summarization...");

    let desktop = Desktop::new(true, true)
        .context("Failed to initialize Terminator Desktop")?;

    loop {
        let triggered = {
            // scope lock
            let mut flag = is_triggered.lock().unwrap();
            if *flag {
                *flag = false;
                true
            } else {
                false
            }
        };

        if triggered {
            if let Err(e) = summarize_all_windows(&desktop, &args.model, Arc::clone(&cache)).await {
                warn!("Error summarizing windows: {}", e);
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
}

async fn summarize_all_windows(
    desktop: &Desktop,
    model: &str,
    cache: Arc<Mutex<HashMap<String, String>>>,
) -> Result<()> {
    info!("⌨ Hotkey triggered — capturing UI context");

    let windows = resolve_windows(desktop).await?;
    if windows.is_empty() {
        warn!("No windows found to summarize");
        return Ok(());
    }

    let mut ui_context = String::new();
    for window in windows {
        let wattr = window.attributes();
        let wname = wattr.name.clone().unwrap_or_else(|| "Unnamed window".into());
        ui_context.push_str(&format!("\n=== Window: {} [{}] ===\n", wname, wattr.role));

        // Get all editable elements
        let editable = match window.locator(Selector::Role {
            role: "edit".into(),
            name: None,
        }) {
            Ok(locator) => match locator.all(Some(Duration::from_millis(500)), None).await {
                Ok(elements) => elements,
                Err(e) => {
                    warn!("❌ Timeout getting editable elements: {}", e);
                    Vec::new()
                }
            },
            Err(e) => {
                warn!("❌ Failed to create locator: {}", e);
                Vec::new()
            }
        };

        for el in editable {
            let attr = el.attributes();
            if attr.is_keyboard_focusable.unwrap_or(false) {
                let name = attr.name.clone().unwrap_or_else(|| "[unnamed]".into());
                ui_context.push_str(&format!("- {} [{}]\n", name, attr.role));
                if let Some(val) = &attr.value {
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

    let summary = run_ollama_cached(&prompt, model, Arc::clone(&cache))?;
    let clean_summary = summary.replace("* ", "- ");

    let mut clipboard = Clipboard::new().context("Clipboard init failed")?;
    clipboard
        .set_text(clean_summary.clone())
        .context("Copy failed")?;

    println!("{}", clean_summary);
    println!("✅ Results copied to clipboard");

    refinement_loop(&clean_summary, model, cache)?;

    Ok(())
}

async fn resolve_windows(desktop: &Desktop) -> Result<Vec<UIElement>> {
    let locator = desktop.locator(Selector::Role {
        role: "window".into(),
        name: None,
    });

    let mut windows = match locator.all(Some(Duration::from_secs(2)), None).await {
        Ok(wins) => wins,
        Err(e) => {
            warn!("Failed to get windows via desktop locator: {}", e);
            Vec::new()
        }
    };

    if windows.is_empty() {
        let browser_names = [
            "chrome", "firefox", "msedge", "edge", "microsoftedge", "brave", "arc", "safari",
        ];

        for name in browser_names {
            match desktop.application(name) {
                Ok(app) => {
                    match app.locator(Selector::Role {
                        role: "window".into(),
                        name: None,
                    }) {
                        Ok(locator) => {
                            if let Ok(win) = locator.all(Some(Duration::from_secs(1)), None).await {
                                windows.extend(win);
                            }
                        }
                        Err(e) => warn!("Error getting windows for {}: {}", name, e),
                    }
                }
                Err(e) => warn!("Error getting application {}: {}", name, e),
            }
        }
    }

    for window in &windows {
        let a = window.attributes();
        info!("Window found: name={:?}, role={:?}", a.name, a.role);
    }

    Ok(windows)
}

fn run_ollama_cached(
    prompt: &str,
    model: &str,
    cache: Arc<Mutex<HashMap<String, String>>>,
) -> Result<String> {
    let key = format!("{}::{}", model, prompt);

    {
        let cache_lock = cache.lock().unwrap();
        if let Some(response) = cache_lock.get(&key) {
            info!("✅ Returning cached response");
            return Ok(response.clone());
        }
    }

    let result = run_ollama(prompt, model)?;

    {
        let mut cache_lock = cache.lock().unwrap();
        cache_lock.insert(key, result.clone());
    }

    Ok(result)
}

fn run_ollama(prompt: &str, model: &str) -> Result<String> {
    let output = Command::new("ollama")
        .args(["run", model, prompt])
        .output()
        .context("Failed to run Ollama")?;

    if !output.status.success() {
        return Err(anyhow!(
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

fn refinement_loop(
    summary: &str,
    model: &str,
    cache: Arc<Mutex<HashMap<String, String>>>,
) -> Result<()> {
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
                        match run_ollama_cached(&prompt, model, Arc::clone(&cache)) {
                            Ok(refined) => {
                                println!("\n✨ Refined:\n{}", refined);

                                if let Err(e) = Clipboard::new()?.set_text(refined) {
                                    warn!("Failed to copy to clipboard: {}", e);
                                }
                            }
                            Err(e) => warn!("Error refining point: {}", e),
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_numbered_points() {
        let summary = "1. Open browser\n- Navigate to site\n2. Click login\n";
        let extracted = extract_numbered_points(summary);
        assert_eq!(extracted.len(), 3);
        assert_eq!(extracted[0], "1. Open browser");
    }

    #[test]
    fn test_caching_mechanism() {
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let prompt = "test prompt";
        let model = "test-model";
        let result = "cached result".to_string();

        {
            let mut cache_lock = cache.lock().unwrap();
            cache_lock.insert(format!("{}::{}", model, prompt), result.clone());
        }

        let cached = run_ollama_cached(prompt, model, Arc::clone(&cache)).unwrap();
        assert_eq!(cached, result);
    }
}
