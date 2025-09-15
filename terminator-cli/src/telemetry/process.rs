use anyhow::Result;
use colored::Colorize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use super::receiver::TelemetryReceiver;
use super::traces::StepsTracker;

pub async fn process_span(span: &serde_json::Value, tracker: &Arc<Mutex<StepsTracker>>) {
    let name = span.get("name").and_then(|v| v.as_str()).unwrap_or("");

    // Parse attributes
    let mut attributes = std::collections::HashMap::new();
    if let Some(attrs) = span.get("attributes").and_then(|v| v.as_array()) {
        for attr in attrs {
            if let (Some(key), Some(value)) =
                (attr.get("key").and_then(|v| v.as_str()), attr.get("value"))
            {
                let val_str = extract_attribute_value(value);
                attributes.insert(key.to_string(), val_str);
            }
        }
    }

    // Parse events (step starts/completes)
    if let Some(events_array) = span.get("events").and_then(|v| v.as_array()) {
        for event in events_array {
            if let Some(event_name) = event.get("name").and_then(|v| v.as_str()) {
                let mut event_attrs = std::collections::HashMap::new();
                if let Some(attrs) = event.get("attributes").and_then(|v| v.as_array()) {
                    for attr in attrs {
                        if let (Some(key), Some(value)) =
                            (attr.get("key").and_then(|v| v.as_str()), attr.get("value"))
                        {
                            let val_str = extract_attribute_value(value);
                            event_attrs.insert(key.to_string(), val_str);
                        }
                    }
                }

                // Display step progress
                match event_name {
                    "workflow.started" => {
                        if let Some(total) = event_attrs.get("workflow.total_steps") {
                            let mut tracker = tracker.lock().await;
                            tracker.total_steps = total.parse().ok();

                            println!(
                                "\n{} {} {}",
                                "🎯".cyan(),
                                "WORKFLOW STARTED:".bold().cyan(),
                                format!("{total} steps").dimmed()
                            );
                        }
                    }
                    "step.started" => {
                        if let Some(tool) = event_attrs.get("step.tool") {
                            let step_index = event_attrs
                                .get("step.index")
                                .and_then(|s| s.parse::<usize>().ok())
                                .unwrap_or(0);

                            let mut tracker = tracker.lock().await;
                            tracker.current_step = step_index + 1;
                            let total = tracker.total_steps.unwrap_or(0);

                            println!(
                                "  {} Step {}/{}: {} {}",
                                "▶".blue(),
                                tracker.current_step,
                                total,
                                tool.yellow(),
                                "[running...]".dimmed()
                            );
                        }
                    }
                    "step.completed" => {
                        if let Some(status) = event_attrs.get("step.status") {
                            let icon = if status == "success" {
                                "✓".green()
                            } else if status == "skipped" {
                                "⏭".yellow()
                            } else {
                                "✗".red()
                            };
                            println!("    {icon} Status: {status}");
                        }
                    }
                    "workflow.completed" => {
                        let had_errors = event_attrs
                            .get("workflow.had_errors")
                            .and_then(|s| s.parse::<bool>().ok())
                            .unwrap_or(false);

                        if had_errors {
                            println!("\n{} Workflow completed with errors", "⚠".yellow());
                        } else {
                            println!("\n{} Workflow completed successfully", "✅".green());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Handle span-level info
    if name.starts_with("workflow.") {
        if let Some(total) = attributes.get("workflow.total_steps") {
            let mut tracker = tracker.lock().await;
            tracker.total_steps = total.parse().ok();
        }
    } else if name.starts_with("step.") {
        // Step span started
        if let Some(tool) = attributes.get("tool.name") {
            let step_num = attributes
                .get("step.number")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            let step_total = attributes
                .get("step.total")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);

            println!(
                "  {} Step {}/{}: {} {}",
                "📍".green(),
                step_num,
                step_total,
                tool.yellow(),
                "[executing...]".dimmed()
            );
        }
    }
}

pub fn extract_attribute_value(value: &serde_json::Value) -> String {
    if let Some(s) = value.get("stringValue").and_then(|v| v.as_str()) {
        s.to_string()
    } else if let Some(i) = value.get("intValue").and_then(|v| v.as_i64()) {
        i.to_string()
    } else if let Some(f) = value.get("doubleValue").and_then(|v| v.as_f64()) {
        f.to_string()
    } else if let Some(b) = value.get("boolValue").and_then(|v| v.as_bool()) {
        b.to_string()
    } else {
        value.to_string()
    }
}


// Extract value from protobuf attribute
pub fn extract_proto_attr_value(
    value: &Option<opentelemetry_proto::tonic::common::v1::AnyValue>,
) -> String {
    if let Some(val) = value {
        if let Some(v) = &val.value {
            match v {
                opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(s) => {
                    s.clone()
                }
                opentelemetry_proto::tonic::common::v1::any_value::Value::IntValue(i) => {
                    i.to_string()
                }
                opentelemetry_proto::tonic::common::v1::any_value::Value::DoubleValue(f) => {
                    f.to_string()
                }
                opentelemetry_proto::tonic::common::v1::any_value::Value::BoolValue(b) => {
                    b.to_string()
                }
                _ => String::new(),
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}

// Start the telemetry receiver
pub async fn start_telemetry_receiver() -> Result<JoinHandle<()>> {
    let receiver = TelemetryReceiver::new(4318);
    receiver.start().await
}
