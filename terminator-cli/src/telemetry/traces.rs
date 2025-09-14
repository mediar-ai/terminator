use bytes::Bytes;
use colored::Colorize;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use axum::{extract::State, http::StatusCode, response::Json};
use prost::Message;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use super::process::{process_span, extract_proto_attr_value};

pub struct StepsTracker {
    pub total_steps: Option<usize>,
    pub current_step: usize,
}

impl StepsTracker {
    pub fn new() -> Self {
        Self {
            total_steps: None,
            current_step: 0,
        }
    }
}

pub async fn handle_traces(
    State(steps): State<Arc<Mutex<StepsTracker>>>,
    body: Bytes,
) -> (StatusCode, Json<serde_json::Value>) {
    // Try to parse as protobuf first (most common)
    if let Ok(request) = ExportTraceServiceRequest::decode(&body[..]) {
        process_protobuf_traces(request, steps).await;
    } else if let Ok(json_data) = serde_json::from_slice::<serde_json::Value>(&body) {
        // Fallback to JSON parsing
        process_json_traces(json_data, steps).await;
    }

    (StatusCode::OK, Json(json!({"partialSuccess": {}})))
}

pub async fn process_json_traces(data: serde_json::Value, steps: Arc<Mutex<StepsTracker>>) {
    if let Some(resource_spans) = data.get("resourceSpans").and_then(|v| v.as_array()) {
        for resource_span in resource_spans {
            if let Some(scope_spans) = resource_span.get("scopeSpans").and_then(|v| v.as_array()) {
                for scope_span in scope_spans {
                    if let Some(spans_array) = scope_span.get("spans").and_then(|v| v.as_array()) {
                        for span in spans_array {
                            process_span(span, &steps).await;
                        }
                    }
                }
            }
        }
    }
}

// Process protobuf traces
pub async fn process_protobuf_traces(
    request: ExportTraceServiceRequest,
    tracker: Arc<Mutex<StepsTracker>>,
) {
    for resource_span in request.resource_spans {
        for scope_span in resource_span.scope_spans {
            for span in scope_span.spans {
                let span_name = span.name.clone();

                // Process events in the span
                for event in &span.events {
                    let event_name = event.name.clone();
                    let mut event_attrs = std::collections::HashMap::new();

                    // Extract event attributes
                    for attr in &event.attributes {
                        let key = attr.key.clone();
                        let value = extract_proto_attr_value(&attr.value);
                        event_attrs.insert(key, value);
                    }

                    // Display step progress based on events
                    match event_name.as_str() {
                        "workflow.started" => {
                            if let Some(total) = event_attrs.get("workflow.total_steps") {
                                let mut t = tracker.lock().await;
                                t.total_steps = total.parse().ok();

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

                                let mut t = tracker.lock().await;
                                t.current_step = step_index + 1;
                                let total = t.total_steps.unwrap_or(0);

                                println!(
                                    "  {} Step {}/{}: {} {}",
                                    "▶".blue(),
                                    t.current_step,
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

                // Also check span-level attributes for step info
                if span_name.starts_with("step.") {
                    let mut span_attrs = std::collections::HashMap::new();
                    for attr in &span.attributes {
                        let key = attr.key.clone();
                        let value = extract_proto_attr_value(&attr.value);
                        span_attrs.insert(key, value);
                    }

                    if let Some(tool) = span_attrs.get("tool.name") {
                        let step_num = span_attrs
                            .get("step.number")
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(0);
                        let step_total = span_attrs
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
        }
    }
}
