//! Terminator-powered backend commands exposed to the React
//! front-end via Tauri.
//!
//! Windows implementation:
//!   • `get_ui_tree` –
//!       Enumerates every running application with `Desktop::applications()`
//!       and returns a vector of `SerializableUIElement` trees (depth = 5).
//!   • `highlight_element` –
//!       Deserialises a JSON snapshot back into a live `UIElement` and flashes
//!       it on the screen for one second.
//!   • All fallible operations are logged with `tracing::error!` and the error
//!     message is bubbled back to the JS side as `Err(String)`.
//!
//! Non-Windows targets compile stub implementations so the project remains
//! cross-platform ready. Once Terminator supports more platforms we can simply
//! replace the stubs.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::Manager;

// ────────────────────────────────────────────────
// Windows-specific implementation
// ────────────────────────────────────────────────
#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use serde_json;
    use std::time::Duration;
    use terminator::{element::SerializableUIElement, Desktop, UIElement};
    use tracing::error;

    /// Enumerate running applications and return their UI trees.
    #[tauri::command]
    pub async fn get_ui_tree() -> Result<Vec<SerializableUIElement>, String> {
        // 1. Bootstrap Terminator.
        let desktop = Desktop::new_default().map_err(report("Failed to create Desktop"))?;

        // 2. Collect root application elements.
        let applications = desktop
            .applications()
            .map_err(report("Failed to enumerate applications"))?;

        // 3. Build a serialisable tree for each app (depth capped for perf).
        const MAX_DEPTH: usize = 5;
        let trees: Vec<_> = applications
            .into_iter()
            .map(|app| app.to_serializable_tree(MAX_DEPTH))
            .collect();

        Ok(trees)
    }

    /// Highlight the given element for one second. The element is provided in
    /// its serialised form (coming from the UI tree).
    #[tauri::command]
    pub fn highlight_element(serialized: String, color: Option<u32>) -> Result<(), String> {
        // 1. Deserialize JSON back into a live UIElement.
        let element: UIElement = serde_json::from_str(&serialized)
            .map_err(report("Failed to deserialize UI element"))?;

        // 2. Highlight – protect against potential panics inside the platform
        //    code by catching unwinds.
        std::panic::catch_unwind(|| element.highlight(color, Some(Duration::from_millis(1_000))))
            .map_err(|_| "Highlight panicked".to_string())?
            .map_err(report("Failed to highlight element"))
    }

    /// Helper to capture and log errors uniformly.
    fn report(ctx: &'static str) -> impl Fn(impl std::fmt::Display) -> String {
        move |e| {
            error!(target: "terminator-inspector", "{ctx}: {e}");
            e.to_string()
        }
    }
}

// ────────────────────────────────────────────────
// Stub implementation for non-Windows targets
// ────────────────────────────────────────────────
#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;
    use terminator::element::SerializableUIElement;

    #[tauri::command]
    pub async fn get_ui_tree() -> Result<Vec<SerializableUIElement>, String> {
        Err("Accessibility inspection is currently supported only on Windows.".into())
    }

    #[tauri::command]
    pub fn highlight_element(_: String, _: Option<u32>) -> Result<(), String> {
        Err("Highlighting is currently supported only on Windows.".into())
    }
}

// ────────────────────────────────────────────────
// Tauri bootstrap
// ────────────────────────────────────────────────
use platform::*;

fn main() {
    // Initialise tracing in dev builds for helpful logs.
    #[cfg(debug_assertions)]
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_ui_tree, highlight_element])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
