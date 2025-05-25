use anyhow::{Context, Result};
use arboard::Clipboard;
use clap::Parser;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use ollama_rs::{generation::completion::request::GenerationRequest, Ollama};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use tokio::sync::Mutex;
use tracing::{info, warn, error};

#[cfg(target_os = "windows")]
mod windows_ui;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Ollama model to use
    #[arg(short, long, default_value = "gemma:2b")]
    model: String,

    /// System prompt for context generation
    #[arg(short, long, default_value = "Here's data from the screen. Please turn this into easy to understand context that I can feed to another AI about what I'm currently doing in this app. Be concise but thorough.")]
    system_prompt: String,

    /// Hotkey combination (Ctrl+Shift+C by default)
    #[arg(short, long, default_value = "ctrl+shift+c")]
    hotkey: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct UIElement {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<UIElement>,
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};

    // Generic platform-agnostic UI element for non-Windows systems
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct UIElement {
        pub role: String,
        pub name: Option<String>,
        pub value: Option<String>,
        pub description: Option<String>,
        pub children: Vec<UIElement>,
    }

    pub fn capture_focused_app_tree() -> Result<UIElement> {
        // This is a placeholder implementation for non-Windows platforms
        // In a real application, you would implement platform-specific code
        let ui_tree = UIElement {
            role: "Application".to_string(),
            name: Some("Active Application".to_string()),
            value: None,
            description: Some("This is a placeholder for non-Windows platforms".to_string()),
            children: vec![],
        };
        
        Ok(ui_tree)
    }
}

async fn capture_app_context() -> Result<String> {
    #[cfg(target_os = "windows")]
    let ui_tree = windows_ui::capture_focused_app_tree()?;

    #[cfg(not(target_os = "windows"))]
    let ui_tree = platform::capture_focused_app_tree()?;
    
    let serialized = serde_json::to_string_pretty(&ui_tree)
        .context("Failed to serialize UI tree")?;
    
    Ok(serialized)
}

async fn process_with_ollama(model: &str, system_prompt: &str, context: &str) -> Result<String> {
    // Create the Ollama client
    let ollama = Ollama::default();
    
    info!("Sending context to Ollama model: {}", model);
    
    // Prepare the prompt with the system prompt and application context
    let prompt = format!(
        "{}

Application data:
{}", 
        system_prompt, 
        context
    );
    
    // Create a generation request
    let request = GenerationRequest::new(model.to_string(), prompt);
    
    // Generate a response from the model
    let response = ollama
        .generate(request)
        .await
        .context("Failed to generate response from Ollama")?;
    
    info!("Successfully received response from Ollama");
    
    // Return the generated response
    Ok(response.response)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for better logging
    tracing_subscriber::fmt::init();
    
    // Parse command-line arguments
    let args = Args::parse();
    
    info!("Starting context-capture with model: {}", args.model);
    info!("Hotkey set to: {}", args.hotkey);
    
    // Parse hotkey configuration
    let hotkey_parts: Vec<&str> = args.hotkey.split('+').collect();
    let mut modifiers = Modifiers::empty();
    let mut code = None;
    
    for part in hotkey_parts {
        match part.trim().to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "alt" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            "super" | "cmd" | "command" => modifiers |= Modifiers::SUPER,
            key => {
                code = Some(match key {
                    "a" => Code::KeyA,
                    "b" => Code::KeyB,
                    "c" => Code::KeyC,
                    "d" => Code::KeyD,
                    "e" => Code::KeyE,
                    "f" => Code::KeyF,
                    "g" => Code::KeyG,
                    "h" => Code::KeyH,
                    "i" => Code::KeyI,
                    "j" => Code::KeyJ,
                    "k" => Code::KeyK,
                    "l" => Code::KeyL,
                    "m" => Code::KeyM,
                    "n" => Code::KeyN,
                    "o" => Code::KeyO,
                    "p" => Code::KeyP,
                    "q" => Code::KeyQ,
                    "r" => Code::KeyR,
                    "s" => Code::KeyS,
                    "t" => Code::KeyT,
                    "u" => Code::KeyU,
                    "v" => Code::KeyV,
                    "w" => Code::KeyW,
                    "x" => Code::KeyX,
                    "y" => Code::KeyY,
                    "z" => Code::KeyZ,
                    _ => anyhow::bail!("Unsupported key: {}", key),
                });
            }
        }
    }
    
    let code = code.context("No key specified in hotkey")?;
    let hotkey = HotKey::new(Some(modifiers), code);
    
    println!("----------------------------");
    println!("Context Capture Tool Started");
    println!("----------------------------");
    println!("Using model: {}", args.model);
    println!("Press {} to capture context", args.hotkey);
    println!("----------------------------");
    
    // Set up the hotkey manager
    let hotkey_manager = GlobalHotKeyManager::new().context("Failed to create hotkey manager")?;
    hotkey_manager.register(hotkey).context("Failed to register hotkey")?;
    
    // Create a minimal hidden window to receive events
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_visible(false)
        .build(&event_loop)
        .context("Failed to create window")?;
    
    // Create shared state
    let model = Arc::new(args.model);
    let system_prompt = Arc::new(args.system_prompt);
    let hotkey_event_receiver = GlobalHotKeyEvent::receiver();
    let processing = Arc::new(Mutex::new(false));
    
    // Run the event loop
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::MainEventsCleared => {
                // Check for hotkey events
                if let Ok(event) = hotkey_event_receiver.try_recv() {
                    if event.id == hotkey.id() {
                        let model_clone = Arc::clone(&model);
                        let system_prompt_clone = Arc::clone(&system_prompt);
                        let processing_clone = Arc::clone(&processing);
                        
                        tokio::spawn(async move {
                            // Ensure we're not already processing a request
                            let mut lock = processing_clone.lock().await;
                            if *lock {
                                println!("Already processing a capture request, please wait...");
                                return;
                            }
                            
                            *lock = true;
                            
                            println!("Hotkey detected! Capturing context...");
                            
                            // Capture context
                            match capture_app_context().await {
                                Ok(context) => {
                                    println!("Context captured, processing with Ollama...");
                                    
                                    // Process with Ollama
                                    match process_with_ollama(&model_clone, &system_prompt_clone, &context).await {
                                        Ok(response) => {
                                            println!("Response generated, copying to clipboard...");
                                            
                                            // Copy to clipboard
                                            match Clipboard::new() {
                                                Ok(mut clipboard) => {
                                                    if let Err(e) = clipboard.set_text(response) {
                                                        println!("Failed to copy to clipboard: {}", e);
                                                    } else {
                                                        println!("Context successfully copied to clipboard!");
                                                    }
                                                }
                                                Err(e) => println!("Failed to access clipboard: {}", e),
                                            }
                                        }
                                        Err(e) => println!("Failed to process with Ollama: {}", e),
                                    }
                                }
                                Err(e) => println!("Failed to capture context: {}", e),
                            }
                            
                            *lock = false;
                        });
                    }
                }
            }
            _ => (),
        }
    });
}
