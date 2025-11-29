//! Inspection tools: get_window_tree, get_applications, overlay display
//!
//! This module contains helpers for UI inspection tools that gather information
//! about application windows, UI trees, OCR, and computer vision results.

use serde_json::{json, Value};
use std::collections::HashMap;
use terminator::Desktop;
use tracing::{info, warn};

/// Represents a source of element bounds for overlay display
pub enum OverlaySource<'a> {
    /// UIA tree elements: index -> (role, name, bounds)
    UiaTree(&'a HashMap<u32, (String, String, (f64, f64, f64, f64))>),
    /// OCR words: index -> (text, bounds)
    Ocr(&'a HashMap<u32, (String, (f64, f64, f64, f64))>),
    /// DOM elements: index -> (tag, identifier, bounds)
    Dom(&'a HashMap<u32, (String, String, (f64, f64, f64, f64))>),
    /// Omniparser items: index -> item with box_2d as [x_min, y_min, x_max, y_max]
    Omniparser(&'a HashMap<u32, crate::omniparser::OmniparserItem>),
    /// Vision items: index -> item with box_2d as [x_min, y_min, x_max, y_max]
    Vision(&'a HashMap<u32, crate::vision::VisionElement>),
}

impl<'a> OverlaySource<'a> {
    /// Convert the source data to InspectElements for overlay display
    pub fn to_inspect_elements(&self) -> Vec<terminator::InspectElement> {
        match self {
            OverlaySource::UiaTree(bounds) => bounds
                .iter()
                .map(|(idx, (role, name, bounds))| terminator::InspectElement {
                    index: *idx,
                    role: role.clone(),
                    name: if name.is_empty() {
                        None
                    } else {
                        Some(name.clone())
                    },
                    bounds: *bounds,
                })
                .collect(),

            OverlaySource::Ocr(bounds) => bounds
                .iter()
                .map(|(idx, (text, bounds))| terminator::InspectElement {
                    index: *idx,
                    role: "OCR".to_string(),
                    name: Some(text.clone()),
                    bounds: *bounds,
                })
                .collect(),

            OverlaySource::Dom(bounds) => bounds
                .iter()
                .map(|(idx, (tag, identifier, bounds))| terminator::InspectElement {
                    index: *idx,
                    role: tag.clone(),
                    name: if identifier.is_empty() {
                        None
                    } else {
                        Some(identifier.clone())
                    },
                    bounds: *bounds,
                })
                .collect(),

            OverlaySource::Omniparser(items) => items
                .iter()
                .filter_map(|(idx, item)| {
                    item.box_2d.map(|b| terminator::InspectElement {
                        index: *idx,
                        role: item.label.clone(),
                        name: item.content.clone(),
                        // Convert [x_min, y_min, x_max, y_max] to (x, y, width, height)
                        bounds: (b[0], b[1], b[2] - b[0], b[3] - b[1]),
                    })
                })
                .collect(),

            OverlaySource::Vision(items) => items
                .iter()
                .filter_map(|(idx, item)| {
                    item.box_2d.map(|b| terminator::InspectElement {
                        index: *idx,
                        role: item.element_type.clone(),
                        name: item.content.clone(),
                        // Convert [x_min, y_min, x_max, y_max] to (x, y, width, height)
                        bounds: (b[0], b[1], b[2] - b[0], b[3] - b[1]),
                    })
                })
                .collect(),
        }
    }

    /// Get the overlay type name for logging and result JSON
    pub fn overlay_type_name(&self) -> &'static str {
        match self {
            OverlaySource::UiaTree(_) => "ui_tree",
            OverlaySource::Ocr(_) => "ocr",
            OverlaySource::Dom(_) => "dom",
            OverlaySource::Omniparser(_) => "omniparser",
            OverlaySource::Vision(_) => "vision",
        }
    }

    /// Get error message when cache is empty
    pub fn empty_cache_error(&self) -> &'static str {
        match self {
            OverlaySource::UiaTree(_) => {
                "No UI elements with bounds in cache - ensure include_tree_after_action is true"
            }
            OverlaySource::Ocr(_) => "No OCR elements in cache - use include_ocr=true first",
            OverlaySource::Dom(_) => {
                "No DOM elements in cache - ensure include_browser_dom is true and browser extension is active"
            }
            OverlaySource::Omniparser(_) => {
                "No omniparser elements in cache - use include_omniparser=true first"
            }
            OverlaySource::Vision(_) => {
                "No vision elements in cache - use include_gemini_vision=true first"
            }
        }
    }
}

/// Show overlay for a given source, updating result_json with outcome
#[cfg(target_os = "windows")]
pub fn show_overlay_for_source(
    source: OverlaySource<'_>,
    desktop: &Desktop,
    pid: u32,
    display_mode: terminator::OverlayDisplayMode,
    overlay_handle: &std::sync::Mutex<Option<terminator::InspectOverlayHandle>>,
    result_json: &mut Value,
) {
    let elements = source.to_inspect_elements();
    let overlay_type = source.overlay_type_name();

    if elements.is_empty() {
        result_json["overlay_error"] = json!(source.empty_cache_error());
        return;
    }

    // Log first element bounds for DPI debugging
    if let Some(first) = elements.first() {
        info!(
            "{} OVERLAY DEBUG: first_element bounds=({:.0},{:.0},{:.0},{:.0})",
            overlay_type.to_uppercase(),
            first.bounds.0,
            first.bounds.1,
            first.bounds.2,
            first.bounds.3
        );
    }

    // Find the application window bounds
    let Ok(apps) = desktop.applications() else {
        result_json["overlay_error"] = json!("Failed to get applications");
        return;
    };

    let Some(app) = apps.iter().find(|a| a.process_id().ok() == Some(pid)) else {
        result_json["overlay_error"] = json!(format!("No application found for PID {}", pid));
        return;
    };

    let Ok((x, y, w, h)) = app.bounds() else {
        result_json["overlay_error"] = json!("Failed to get application bounds");
        return;
    };

    info!(
        "{} OVERLAY DEBUG: window_bounds for overlay=({:.0},{:.0},{:.0},{:.0})",
        overlay_type.to_uppercase(),
        x,
        y,
        w,
        h
    );

    // Clear existing overlay
    if let Ok(mut handle) = overlay_handle.lock() {
        *handle = None;
    }
    terminator::hide_inspect_overlay();

    // Show new overlay
    match terminator::show_inspect_overlay(
        elements,
        (x as i32, y as i32, w as i32, h as i32),
        display_mode,
    ) {
        Ok(new_handle) => {
            if let Ok(mut handle) = overlay_handle.lock() {
                *handle = Some(new_handle);
            }
            result_json["overlay_shown"] = json!(overlay_type);
            info!("Inspect overlay shown for {}", overlay_type);
        }
        Err(e) => {
            warn!("Failed to show inspect overlay for {}: {}", overlay_type, e);
            result_json["overlay_error"] = json!(e.to_string());
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn show_overlay_for_source(
    _source: OverlaySource<'_>,
    _desktop: &Desktop,
    _pid: u32,
    _display_mode: terminator::OverlayDisplayMode,
    _overlay_handle: &std::sync::Mutex<Option<()>>,
    result_json: &mut Value,
) {
    result_json["overlay_error"] = json!("Overlay display is only supported on Windows");
}

/// Parse overlay display mode from string argument
pub fn parse_overlay_display_mode(
    mode_str: Option<&str>,
    result_json: &mut Value,
) -> terminator::OverlayDisplayMode {
    match mode_str {
        Some("rectangles") => terminator::OverlayDisplayMode::Rectangles,
        Some("index") | None => terminator::OverlayDisplayMode::Index,
        Some("role") => terminator::OverlayDisplayMode::Role,
        Some("index_role") => terminator::OverlayDisplayMode::IndexRole,
        Some("name") => terminator::OverlayDisplayMode::Name,
        Some("index_name") => terminator::OverlayDisplayMode::IndexName,
        Some("full") => terminator::OverlayDisplayMode::Full,
        Some(other) => {
            result_json["overlay_error"] = json!(format!(
                "Unknown overlay_display_mode: '{}'. Valid options: rectangles, index, role, index_role, name, index_name, full",
                other
            ));
            terminator::OverlayDisplayMode::Index // fallback to default
        }
    }
}

/// Find PID for a process by name
pub fn find_pid_for_process(
    desktop: &Desktop,
    process_name: &str,
) -> Result<u32, rmcp::ErrorData> {
    use sysinfo::{ProcessesToUpdate, System};

    let apps = desktop.applications().map_err(|e| {
        rmcp::ErrorData::resource_not_found(
            "Failed to get applications",
            Some(json!({"reason": e.to_string()})),
        )
    })?;

    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);

    apps.iter()
        .filter_map(|app| {
            let app_pid = app.process_id().unwrap_or(0);
            if app_pid > 0 {
                system
                    .process(sysinfo::Pid::from_u32(app_pid))
                    .and_then(|p| {
                        let name = p.name().to_string_lossy().to_string();
                        if name.to_lowercase().contains(&process_name.to_lowercase()) {
                            Some(app_pid)
                        } else {
                            None
                        }
                    })
            } else {
                None
            }
        })
        .next()
        .ok_or_else(|| {
            rmcp::ErrorData::resource_not_found(
                format!(
                    "Process '{}' not found. Use open_application to start it first.",
                    process_name
                ),
                Some(json!({"process": process_name})),
            )
        })
}

/// Detect if a PID belongs to a known browser process
pub fn is_browser_pid(pid: u32) -> bool {
    const KNOWN_BROWSER_PROCESS_NAMES: &[&str] = &[
        "chrome", "firefox", "msedge", "edge", "iexplore", "opera", "brave", "vivaldi", "browser",
        "arc",
    ];

    #[cfg(target_os = "windows")]
    {
        use terminator::get_process_name_by_pid;
        if let Ok(process_name) = get_process_name_by_pid(pid as i32) {
            let process_name_lower = process_name.to_lowercase();
            return KNOWN_BROWSER_PROCESS_NAMES
                .iter()
                .any(|&browser| process_name_lower.contains(browser));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = pid; // Suppress unused warning
    }

    false
}
