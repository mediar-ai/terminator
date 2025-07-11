#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use serde_json;
    use std::time::Duration;
    use terminator::element::SerializableUIElement;
    use terminator::{Desktop, UIElement};
    use tracing::{error, info};

    #[tauri::command]
    pub async fn get_ui_tree() -> Result<Vec<SerializableUIElement>, String> {
        let desktop = Desktop::new_default().map_err(|e| {
            error!("Failed to create Desktop: {e}");
            e.to_string()
        })?;

        let apps = desktop.applications().map_err(|e| {
            error!("Failed to enumerate applications: {e}");
            e.to_string()
        })?;

        let mut trees = Vec::new();
        for app in apps {
            // Depth of 5 is a good compromise for performance vs detail.
            trees.push(app.to_serializable_tree(5));
        }

        Ok(trees)
    }

    #[tauri::command]
    pub fn highlight_element(serialized: String, color: Option<u32>) -> Result<(), String> {
        // 1. Deserialize into a live UIElement (Termin­ator will locate it).
        let element: UIElement = match serde_json::from_str(&serialized) {
            Ok(el) => el,
            Err(e) => {
                error!("Failed to deserialize SerializableUIElement: {e}");
                return Err(format!("serde error: {e}"));
            }
        };

        // 2. Attempt to highlight – wrap in catch_unwind so we never panic across FFI.
        std::panic::catch_unwind(move || {
            element.highlight(color, Some(Duration::from_millis(1000)))
        })
        .map_err(|_| "highlight panicked".to_string())?
        .map_err(|e| {
            error!("Highlighting failed: {e}");
            e.to_string()
        })
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::*;
    use terminator::element::SerializableUIElement;

    #[tauri::command]
    pub async fn get_ui_tree() -> Result<Vec<SerializableUIElement>, String> {
        Err("Accessibility engine currently supported only on Windows".into())
    }

    #[tauri::command]
    pub fn highlight_element(_serialized: String, _color: Option<u32>) -> Result<(), String> {
        Err("Highlight not supported on this platform".into())
    }
}

use platform::*;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_ui_tree, highlight_element])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
