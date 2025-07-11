#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use terminator::{Desktop, UIElement, UINode};

#[tauri::command]
async fn get_ui_tree() -> Result<Vec<UINode>, String> {
    let desktop = Desktop::new_default().map_err(|e| e.to_string())?;
    desktop
        .get_all_applications_tree()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn highlight_element(serialized: String) -> Result<(), String> {
    let element: UIElement =
        serde_json::from_str(&serialized).map_err(|e| format!("serde error: {e}"))?;
    element
        .highlight(None, Some(std::time::Duration::from_millis(1000)))
        .map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_ui_tree, highlight_element])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
