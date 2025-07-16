use clap::Parser;
use std::sync::Arc;
use anyhow::Result;
use std::sync::Mutex;
use arboard::Clipboard;
use tracing::{debug, info, error};
use rdev::{listen, Event, EventType, Key};
use crate::{
    utils::{init_logging, Args},
    client::get_mcp_tool_result,
    ollama::summrize_by_ollama,
};

mod client;
mod ollama;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;

    let args = Args::parse();
    info!("initializing summarizer with model: '{}' hotkey: {}", args.model, args.hotkey);

    let is_triggered = Arc::new(Mutex::new(false));
    let trigger_clone = Arc::clone(&is_triggered);

    let ctrl_pressed = Arc::new(Mutex::new(false));
    let ctrl_state = Arc::clone(&ctrl_pressed);

    let alt_pressed = Arc::new(Mutex::new(false));
    let alt_state = Arc::clone(&alt_pressed);

    std::thread::spawn(move || {
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
                EventType::KeyPress(Key::Alt) => {
                    if let Ok(mut alt) = alt_state.lock() {
                        *alt = true;
                    }
                }
                EventType::KeyRelease(Key::Alt) => {
                    if let Ok(mut alt) = alt_state.lock() {
                        *alt = false;
                    }
                }
                EventType::KeyPress(Key::KeyJ) => {
                    let ctrl = ctrl_state.lock().unwrap();
                    let alt = alt_state.lock().unwrap();
                    if *ctrl && *alt {
                        info!("'{}' pressed!", args.hotkey);
                        if let Ok(mut triggered) = trigger_clone.lock() {
                            *triggered = true;
                        }
                    }
                }
                _ => {}
            }
        }) {
            error!("error listening to keyboard events: {:?}", e);
        }
    });

    loop {
        let triggered = {
            let mut flag = is_triggered.lock().unwrap();
            if *flag {
                *flag = false;
                true
            } else {
                false
            }
        };

        if triggered {
            match get_mcp_tool_result("get_focused_window_tree".to_string(), None).await {
                Ok(result) => {
                    debug!("current screen context captured: {}", result);
                    let text_to_copy = if args.ai_mode {
                        match summrize_by_ollama(&args.model, &args.system_prompt, &result).await {
                            Ok(summary) => {
                                debug!("ai summary generated successfully");
                                summary
                            }
                            Err(e) => {
                                error!("failed to summarize with Ollama: {}", e);
                                continue;
                            }
                        }
                    } else {
                        result.to_string()
                    };

                    debug!("copying text to clipboard...");
                    match Clipboard::new() {
                        Ok(mut clipboard) => {
                            if let Err(e) = clipboard.set_text(text_to_copy) {
                                error!("failed to copy to clipboard: {}", e);
                            } else {
                                info!("context successfully copied to clipboard!");
                            }
                        }
                        Err(e) => error!("failed to access clipboard: {}", e),
                    }
                }
                Err(e) => error!("failed to capture context: {}", e),
            }

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
}
}

