use std::time::Instant;
use terminator_workflow_recorder::{
    McpConverter, WorkflowEvent, WorkflowRecorder, WorkflowRecorderConfig,
};
use tokio::signal::ctrl_c;
use tokio_stream::StreamExt;
use tracing::{info, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("🧪 MCP Debug Recorder Started");
    info!("=============================");
    info!("📝 This will show both RAW events and MCP conversions");
    info!("🎯 Focus on dropdown interactions and app switches");
    info!("🛑 Press Ctrl+C to stop recording");
    info!("");

    // Create recorder config with available fields
    let config = WorkflowRecorderConfig {
        record_keyboard: true,
        capture_ui_elements: true,
        record_clipboard: false, // Reduce noise
        record_hotkeys: true,
        filter_mouse_noise: true, // Filter out mouse movements and wheel events
        ..Default::default()
    };

    // Create recorder and converter
    let mut recorder = WorkflowRecorder::new("MCP Debug Session".to_string(), config);
    let mut event_stream = recorder.event_stream();
    let converter = McpConverter::new();

    recorder.start().await?;
    info!("✅ Recording started! Interact with your UI...");
    info!("");

    // Event processing loop with Ctrl+C handling
    let mut event_count = 0;
    let start_time = Instant::now();

    loop {
        tokio::select! {
            Some(event) = event_stream.next() => {
                event_count += 1;
                let elapsed = start_time.elapsed();

                // Show raw event with timing
                println!("┌─ EVENT #{} ({:.1}s) ─────────────────────────",
                    event_count, elapsed.as_secs_f32());

                match &event {
                    WorkflowEvent::TextInputCompleted(text_event) => {
                        println!("│ 📝 TEXT INPUT COMPLETED");
                        println!("│   Text: '{}'", text_event.text_value);
                        println!("│   Method: {:?}", text_event.input_method);
                        println!("│   Field: {:?}", text_event.field_name);
                        println!("│   Type: {}", text_event.field_type);
                        println!("│   Duration: {}ms", text_event.typing_duration_ms);
                        println!("│   Keystrokes: {}", text_event.keystroke_count);

                        if text_event.input_method == terminator_workflow_recorder::TextInputMethod::Suggestion {
                            println!("│   🔥 SUGGESTION/DROPDOWN DETECTED!");
                        }

                        if let Some(ref ui_element) = text_event.metadata.ui_element {
                            println!("│   App: {}", ui_element.application_name());
                            println!("│   Element Role: {}", ui_element.role());
                            if let Some(name) = ui_element.name() {
                                println!("│   Element Name: '{}'", name);
                            }
                        }
                    }

                                         WorkflowEvent::ApplicationSwitch(app_event) => {
                         println!("│ 🔄 APPLICATION SWITCH");
                         println!("│   From: {:?}", app_event.from_application);
                         println!("│   To: {}", app_event.to_application);
                         println!("│   Method: {:?}", app_event.switch_method);
                     }

                    WorkflowEvent::Click(click_event) => {
                        println!("│ 🔘 CLICK");
                        println!("│   Text: '{}'", click_event.element_text);
                        println!("│   Role: {}", click_event.element_role);
                        println!("│   Type: {:?}", click_event.interaction_type);
                        if let Some(pos) = click_event.click_position {
                            println!("│   Position: ({}, {})", pos.x, pos.y);
                        }

                        // 🆕 NEW: Display child text content
                        if !click_event.child_text_content.is_empty() {
                            println!("│   Child Texts: [{}]", click_event.child_text_content.join(", "));
                        } else {
                            println!("│   Child Texts: (none)");
                        }

                        if let Some(ref ui_element) = click_event.metadata.ui_element {
                            println!("│   App: {}", ui_element.application_name());
                        }
                    }

                                         WorkflowEvent::BrowserTabNavigation(nav_event) => {
                         println!("│ 🌐 BROWSER NAVIGATION");
                         if let Some(ref url) = nav_event.to_url {
                             println!("│   URL: {}", url);
                         }
                         if let Some(ref title) = nav_event.to_title {
                             println!("│   Title: {}", title);
                         }
                         println!("│   Action: {:?}", nav_event.action);
                     }

                    WorkflowEvent::Keyboard(kb_event) => {
                        if kb_event.is_key_down {
                            let modifiers = format!("{}{}{}{}",
                                if kb_event.ctrl_pressed { "Ctrl+" } else { "" },
                                if kb_event.alt_pressed { "Alt+" } else { "" },
                                if kb_event.shift_pressed { "Shift+" } else { "" },
                                if kb_event.win_pressed { "Win+" } else { "" }
                            );

                            if let Some(ch) = kb_event.character {
                                println!("│ ⌨️  KEYBOARD: {}{}", modifiers, ch);
                            } else {
                                println!("│ ⌨️  KEYBOARD: {}Key({})", modifiers, kb_event.key_code);
                            }
                        }
                    }

                    WorkflowEvent::Mouse(mouse_event) => {
                        println!("│ 🖱️ MOUSE EVENT");
                        println!("│   Type: {:?}", mouse_event.event_type);
                        println!("│   Button: {:?}", mouse_event.button);
                        println!("│   Position: ({}, {})", mouse_event.position.x, mouse_event.position.y);

                        if let Some(ref element) = mouse_event.metadata.ui_element {
                            println!("│   UI Element: Available");

                            // 🔍 DETAILED ELEMENT ANALYSIS
                            println!("│   ┌─ ELEMENT DETAILS ─────────────────────");

                            // Element name
                            let name = element.name().unwrap_or_default();
                            if !name.is_empty() {
                                println!("│   │ Name: '{}'", name);
                            } else {
                                println!("│   │ Name: <empty>");
                            }

                            // Element role/control type
                            let role = element.role();
                            if !role.is_empty() {
                                println!("│   │ Role/Type: '{}'", role);
                            } else {
                                println!("│   │ Role/Type: <unknown>");
                            }

                            // Element attributes
                            let attrs = element.attributes();

                            // Element class name (from properties if available)
                            if let Some(class_name_value) = attrs.properties.get("ClassName") {
                                if let Some(serde_json::Value::String(class_name)) = class_name_value {
                                    println!("│   │ Class: '{}'", class_name);
                                }
                            } else {
                                println!("│   │ Class: <unknown>");
                            }

                            // Element description
                            if let Some(description) = &attrs.description {
                                if !description.is_empty() {
                                    println!("│   │ Description: '{}'", description);
                                }
                            }

                            // Element automation ID (from attributes if available)
                            if let Some(automation_id_value) = attrs.properties.get("AutomationId") {
                                if let Some(serde_json::Value::String(aid)) = automation_id_value {
                                    println!("│   │ Automation ID: '{}'", aid);
                                }
                            }

                            // 📋 SHOW ALL TEXT-CONTAINING PROPERTIES
                            println!("│   │ ── ALL TEXT PROPERTIES ──");
                            for (key, value) in attrs.properties.iter() {
                                if let Some(serde_json::Value::String(text_value)) = value {
                                    if !text_value.is_empty() && key != "ClassName" && key != "AutomationId" {
                                        println!("│   │ {}: '{}'", key, text_value);
                                    }
                                }
                            }

                            // Check for additional properties that might contain search text
                            if let Some(Some(serde_json::Value::String(localized_type))) = attrs.properties.get("LocalizedControlType") {
                                if !localized_type.is_empty() {
                                    println!("│   │ LocalizedControlType: '{}'", localized_type);
                                }
                            }

                            if let Some(Some(serde_json::Value::String(access_key))) = attrs.properties.get("AccessKey") {
                                if !access_key.is_empty() {
                                    println!("│   │ AccessKey: '{}'", access_key);
                                }
                            }

                            // Element enabled state
                            if let Ok(is_enabled) = element.is_enabled() {
                                println!("│   │ Enabled: {}", is_enabled);
                            }

                            // Element bounds
                            if let Ok(bounds) = element.bounds() {
                                println!("│   │ Bounds: ({}, {}, {}, {})", bounds.0, bounds.1, bounds.2, bounds.3);
                            }

                            // CLICKABLE ANALYSIS
                            let is_clickable_by_current_logic = role.contains("button")
                                || role.contains("menuitem")
                                || role.contains("listitem")
                                || role.contains("hyperlink")
                                || role.contains("link")
                                || role.contains("checkbox")
                                || role.contains("radiobutton")
                                || role.contains("togglebutton");

                            println!("│   │ Clickable (current logic): {}", is_clickable_by_current_logic);

                            // Show if this element would be detected as clickable with expanded rules
                            let is_clickable_expanded = is_clickable_by_current_logic
                                || role.contains("combobox")
                                || role.contains("text")
                                || role.contains("edit")
                                || role.contains("dropdown")
                                || role.contains("list");

                            if !is_clickable_by_current_logic && is_clickable_expanded {
                                println!("│   │ Clickable (with expanded rules): ✅ YES");
                            }

                            println!("│   └─────────────────────────────────────");
                        } else {
                            println!("│   UI Element: None");
                        }
                    }

                    _ => {
                        println!("│ 📋 OTHER: {:?}", event);
                    }
                }

                // Convert to MCP and show results
                println!("│");
                print!("│ 🔄 MCP CONVERSION: ");

                match converter.convert_event(&event, None).await {
                    Ok(result) => {
                        if result.primary_sequence.is_empty() {
                            println!("No sequence generated");
                        } else {
                            println!("");
                            println!("│   Action: {}", result.semantic_action);
                            println!("│   Steps: {}", result.primary_sequence.len());

                            for (i, step) in result.primary_sequence.iter().enumerate() {
                                println!("│   {}. {} -> {}", i+1, step.tool_name, step.description);
                                println!("│      Args: {}", step.arguments);
                                if let Some(timeout) = step.timeout_ms {
                                    println!("│      Timeout: {}ms", timeout);
                                }
                                if let Some(delay) = step.delay_ms {
                                    println!("│      Delay: {}ms", delay);
                                }
                            }

                            if !result.fallback_sequences.is_empty() {
                                println!("│   Fallbacks: {} sequences", result.fallback_sequences.len());
                            }

                            if !result.conversion_notes.is_empty() {
                                println!("│   Notes: {:?}", result.conversion_notes);
                            }
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed: {}", e);
                    }
                }

                println!("└─────────────────────────────────────────────");
                println!("");
            }

            _ = ctrl_c() => {
                println!("\n🛑 Ctrl+C received, stopping recording...");
                break;
            }
        }
    }

    // Stop recording and show summary
    let total_time = start_time.elapsed();

    info!("");
    info!("📊 RECORDING SUMMARY");
    info!("===================");
    info!("⏱️  Duration: {:.1} seconds", total_time.as_secs_f32());
    info!("📋 Total Events Processed: {}", event_count);
    info!(
        "📈 Rate: {:.1} events/second",
        event_count as f32 / total_time.as_secs_f32()
    );
    info!("🎯 Recording complete! Check the output above for dropdown/MCP conversion analysis.");

    Ok(())
}
