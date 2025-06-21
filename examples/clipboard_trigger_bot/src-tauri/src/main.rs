#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use rdev::{simulate, EventType, Key};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{
    CustomMenuItem, GlobalShortcutManager, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu,
    SystemTrayMenuItem, Window,
};

// State to store field data
struct FormState(Mutex<Vec<FieldMapping>>);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FieldMapping {
    name: String,
    hotkey: String,
    value: String,
}

// Commands
#[tauri::command]
async fn register_shortcut(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    app_handle
        .global_shortcut_manager()
        .register(&shortcut, move || {
            let app_handle = app_handle.clone();
            app_handle
                .emit_all("shortcut-triggered", {})
                .expect("Failed to emit event");
        })
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn unregister_shortcut(app_handle: tauri::AppHandle) -> Result<(), String> {
    app_handle
        .global_shortcut_manager()
        .unregister_all()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn check_permissions(window: Window) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        // On macOS, we can't reliably check permissions programmatically
        // So we show a notification to the user about required permissions
        window
            .emit("show-permission-dialog", {})
            .map_err(|e| e.to_string())?;
    }

    Ok(true)
}

#[tauri::command]
fn fill_field(field_hotkey: String, value: String) -> Result<bool, String> {
    // Small delay before starting
    thread::sleep(Duration::from_millis(500));

    // Press the hotkey to focus the field
    press_hotkey(&field_hotkey)?;

    // Small delay to ensure focus
    thread::sleep(Duration::from_millis(300));

    // Type the value
    type_string(&value)?;

    Ok(true)
}

#[tauri::command]
fn update_field_mappings(
    state: tauri::State<'_, FormState>,
    mappings: Vec<FieldMapping>,
) -> Result<(), String> {
    *state.0.lock().unwrap() = mappings;
    Ok(())
}

#[tauri::command]
fn get_field_mappings(state: tauri::State<'_, FormState>) -> Result<Vec<FieldMapping>, String> {
    Ok(state.0.lock().unwrap().clone())
}

// Helper function to press a hotkey
fn press_hotkey(hotkey: &str) -> Result<(), String> {
    let parts: Vec<&str> = hotkey.split('+').collect();

    // Handle special case for Tab, which is very common
    if hotkey.to_lowercase() == "tab" {
        send_key(Key::Tab)?;
        // Ensure key is fully released before continuing
        thread::sleep(Duration::from_millis(100));
        return Ok(());
    }

    // Handle special case for Enter/Return
    if hotkey.to_lowercase() == "enter" || hotkey.to_lowercase() == "return" {
        send_key(Key::Return)?;
        thread::sleep(Duration::from_millis(100));
        return Ok(());
    }

    // Handle more complex hotkeys
    if parts.len() > 1 {
        // Press modifier keys
        for part in &parts[..parts.len() - 1] {
            match part.trim().to_lowercase().as_str() {
                "ctrl" | "control" => send_key_down(Key::ControlLeft)?,
                "alt" => send_key_down(Key::Alt)?,
                "shift" => send_key_down(Key::ShiftLeft)?,
                "meta" | "command" | "cmd" | "super" | "win" => send_key_down(Key::MetaLeft)?,
                _ => return Err(format!("Unknown modifier key: {}", part)),
            }
            // Small delay between modifier key presses
            thread::sleep(Duration::from_millis(20));
        }

        // Press the main key
        if let Some(last_key) = parts.last() {
            // Try to map the key
            let key = map_key_string(last_key)?;
            send_key_press_special(key)?;
        }

        // Release modifier keys in reverse order
        for part in parts[..parts.len() - 1].iter().rev() {
            match part.trim().to_lowercase().as_str() {
                "ctrl" | "control" => send_key_up(Key::ControlLeft)?,
                "alt" => send_key_up(Key::Alt)?,
                "shift" => send_key_up(Key::ShiftLeft)?,
                "meta" | "command" | "cmd" | "super" | "win" => send_key_up(Key::MetaLeft)?,
                _ => {} // Already checked above
            }
            // Small delay between modifier key releases
            thread::sleep(Duration::from_millis(20));
        }

        // Ensure all keys are fully released before continuing
        thread::sleep(Duration::from_millis(100));
    } else {
        // Single key
        let key = map_key_string(hotkey)?;
        send_key(key)?;
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

// Maps a string to a Key enum
fn map_key_string(key_str: &str) -> Result<Key, String> {
    match key_str.trim().to_lowercase().as_str() {
        "tab" => Ok(Key::Tab),
        "enter" | "return" => Ok(Key::Return),
        "space" => Ok(Key::Space),
        "backspace" => Ok(Key::Backspace),
        "escape" | "esc" => Ok(Key::Escape),
        "a" => Ok(Key::KeyA),
        "b" => Ok(Key::KeyB),
        "c" => Ok(Key::KeyC),
        "d" => Ok(Key::KeyD),
        "e" => Ok(Key::KeyE),
        "f" => Ok(Key::KeyF),
        "g" => Ok(Key::KeyG),
        "h" => Ok(Key::KeyH),
        "i" => Ok(Key::KeyI),
        "j" => Ok(Key::KeyJ),
        "k" => Ok(Key::KeyK),
        "l" => Ok(Key::KeyL),
        "m" => Ok(Key::KeyM),
        "n" => Ok(Key::KeyN),
        "o" => Ok(Key::KeyO),
        "p" => Ok(Key::KeyP),
        "q" => Ok(Key::KeyQ),
        "r" => Ok(Key::KeyR),
        "s" => Ok(Key::KeyS),
        "t" => Ok(Key::KeyT),
        "u" => Ok(Key::KeyU),
        "v" => Ok(Key::KeyV),
        "w" => Ok(Key::KeyW),
        "x" => Ok(Key::KeyX),
        "y" => Ok(Key::KeyY),
        "z" => Ok(Key::KeyZ),
        "0" => Ok(Key::Num0),
        "1" => Ok(Key::Num1),
        "2" => Ok(Key::Num2),
        "3" => Ok(Key::Num3),
        "4" => Ok(Key::Num4),
        "5" => Ok(Key::Num5),
        "6" => Ok(Key::Num6),
        "7" => Ok(Key::Num7),
        "8" => Ok(Key::Num8),
        "9" => Ok(Key::Num9),
        _ => Err(format!("Unsupported key: {}", key_str)),
    }
}

// Helper function to type a string
fn type_string(text: &str) -> Result<(), String> {
    for c in text.chars() {
        match c {
            ' ' => send_key(Key::Space)?,
            'a'..='z' => {
                let key_name = format!("Key{}", c.to_uppercase().next().unwrap());
                let key = match key_name.as_str() {
                    "KeyA" => Key::KeyA,
                    "KeyB" => Key::KeyB,
                    "KeyC" => Key::KeyC,
                    "KeyD" => Key::KeyD,
                    "KeyE" => Key::KeyE,
                    "KeyF" => Key::KeyF,
                    "KeyG" => Key::KeyG,
                    "KeyH" => Key::KeyH,
                    "KeyI" => Key::KeyI,
                    "KeyJ" => Key::KeyJ,
                    "KeyK" => Key::KeyK,
                    "KeyL" => Key::KeyL,
                    "KeyM" => Key::KeyM,
                    "KeyN" => Key::KeyN,
                    "KeyO" => Key::KeyO,
                    "KeyP" => Key::KeyP,
                    "KeyQ" => Key::KeyQ,
                    "KeyR" => Key::KeyR,
                    "KeyS" => Key::KeyS,
                    "KeyT" => Key::KeyT,
                    "KeyU" => Key::KeyU,
                    "KeyV" => Key::KeyV,
                    "KeyW" => Key::KeyW,
                    "KeyX" => Key::KeyX,
                    "KeyY" => Key::KeyY,
                    "KeyZ" => Key::KeyZ,
                    _ => return Err(format!("Unsupported key: {}", key_name)),
                };
                send_key(key)?;
            }
            'A'..='Z' => {
                // For uppercase, hold shift
                send_key_down(Key::ShiftLeft)?;

                let key_name = format!("Key{}", c);
                let key = match key_name.as_str() {
                    "KeyA" => Key::KeyA,
                    "KeyB" => Key::KeyB,
                    "KeyC" => Key::KeyC,
                    "KeyD" => Key::KeyD,
                    "KeyE" => Key::KeyE,
                    "KeyF" => Key::KeyF,
                    "KeyG" => Key::KeyG,
                    "KeyH" => Key::KeyH,
                    "KeyI" => Key::KeyI,
                    "KeyJ" => Key::KeyJ,
                    "KeyK" => Key::KeyK,
                    "KeyL" => Key::KeyL,
                    "KeyM" => Key::KeyM,
                    "KeyN" => Key::KeyN,
                    "KeyO" => Key::KeyO,
                    "KeyP" => Key::KeyP,
                    "KeyQ" => Key::KeyQ,
                    "KeyR" => Key::KeyR,
                    "KeyS" => Key::KeyS,
                    "KeyT" => Key::KeyT,
                    "KeyU" => Key::KeyU,
                    "KeyV" => Key::KeyV,
                    "KeyW" => Key::KeyW,
                    "KeyX" => Key::KeyX,
                    "KeyY" => Key::KeyY,
                    "KeyZ" => Key::KeyZ,
                    _ => return Err(format!("Unsupported key: {}", key_name)),
                };
                send_key(key)?;

                // Release shift
                send_key_up(Key::ShiftLeft)?;
            }
            '0'..='9' => {
                let key = match c {
                    '0' => Key::Num0,
                    '1' => Key::Num1,
                    '2' => Key::Num2,
                    '3' => Key::Num3,
                    '4' => Key::Num4,
                    '5' => Key::Num5,
                    '6' => Key::Num6,
                    '7' => Key::Num7,
                    '8' => Key::Num8,
                    '9' => Key::Num9,
                    _ => unreachable!(),
                };
                send_key(key)?;
            }
            '.' => send_key(Key::Dot)?,
            ',' => send_key(Key::Comma)?,
            '-' => send_key(Key::Minus)?,
            '_' => {
                send_key_down(Key::ShiftLeft)?;
                send_key(Key::Minus)?;
                send_key_up(Key::ShiftLeft)?;
            }
            '@' => {
                send_key_down(Key::ShiftLeft)?;
                send_key(Key::Num2)?;
                send_key_up(Key::ShiftLeft)?;
            }
            '!' => {
                send_key_down(Key::ShiftLeft)?;
                send_key(Key::Num1)?;
                send_key_up(Key::ShiftLeft)?;
            }
            // Add more special characters as needed
            _ => return Err(format!("Unsupported character: {}", c)),
        }

        // Small delay between characters for reliability
        thread::sleep(Duration::from_millis(20));
    }

    Ok(())
}

// Low-level functions for keyboard events
fn send_key(key: Key) -> Result<(), String> {
    send_key_down(key)?;
    thread::sleep(Duration::from_millis(20));
    send_key_up(key)?;
    Ok(())
}

fn send_key_down(key: Key) -> Result<(), String> {
    simulate(&EventType::KeyPress(key)).map_err(|e| format!("Failed to simulate key down: {:?}", e))
}

fn send_key_up(key: Key) -> Result<(), String> {
    simulate(&EventType::KeyRelease(key)).map_err(|e| format!("Failed to simulate key up: {:?}", e))
}

fn send_key_press_special(key: Key) -> Result<(), String> {
    simulate(&EventType::KeyPress(key))
        .map_err(|e| format!("Failed to simulate special key press: {:?}", e))?;
    thread::sleep(Duration::from_millis(20));
    simulate(&EventType::KeyRelease(key))
        .map_err(|e| format!("Failed to simulate special key release: {:?}", e))
}

fn main() {
    // System tray menu
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let show = CustomMenuItem::new("show".to_string(), "Show");
    let tray_menu = SystemTrayMenu::new()
        .add_item(show)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit);

    let system_tray = SystemTray::new().with_menu(tray_menu);

    // Default field mappings
    let default_mappings = vec![
        FieldMapping {
            name: "Field 1".to_string(),
            hotkey: "Tab".to_string(),
            value: "".to_string(),
        },
        FieldMapping {
            name: "Field 2".to_string(),
            hotkey: "Tab".to_string(),
            value: "".to_string(),
        },
        FieldMapping {
            name: "Field 3".to_string(),
            hotkey: "Tab".to_string(),
            value: "".to_string(),
        },
    ];

    tauri::Builder::default()
        .manage(FormState(Mutex::new(default_mappings)))
        .system_tray(system_tray)
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "quit" => {
                    std::process::exit(0);
                }
                "show" => {
                    let window = app.get_window("main").unwrap();
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                _ => {}
            },
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            register_shortcut,
            unregister_shortcut,
            fill_field,
            update_field_mappings,
            get_field_mappings,
            check_permissions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
