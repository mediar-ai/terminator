//! Test script for keyboard focus and caret position restoration
//!
//! Run with: cargo run --example focus_restore_test -p terminator-rs
//!
//! Test scenario:
//! 1. Opens Notepad, types text, positions cursor in middle
//! 2. Saves focus and caret position
//! 3. Opens another Notepad (stealing focus)
//! 4. Restores focus and caret to original Notepad
//! 5. Verifies cursor is back where it was

use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
fn main() {
    println!("=== Focus & Caret Restore Test ===\n");
    println!("This test will:");
    println!("  1. Save your current focus and caret position");
    println!("  2. Type into Notepad");
    println!("  3. Restore your caret position");
    println!("\n>>> CLICK INTO A TEXT EDITOR (like Notepad) NOW! <<<");
    println!(">>> You have 5 seconds... <<<\n");
    thread::sleep(Duration::from_secs(5));

    unsafe {
        test_type_and_restore();
    }
}

#[cfg(target_os = "windows")]
fn activate_notepad_window() {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    use windows::core::w;

    unsafe {
        match FindWindowW(w!("Notepad"), None) {
            Ok(hwnd) if hwnd != HWND(std::ptr::null_mut()) => {
                println!("  [LOG] Found Notepad window, activating...");
                ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
            _ => {
                println!("  [LOG] Could not find Notepad window");
            }
        }
    }
}

#[cfg(target_os = "windows")]
unsafe fn test_type_and_restore() {
    use windows::core::BOOL;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern2,
        IUIAutomationTextRange, UIA_CONTROLTYPE_ID, UIA_TextPattern2Id,
    };
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE};
    use windows::core::w;

    // Initialize COM
    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

    let total_start = Instant::now();

    // Create UI Automation instance
    let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
        .expect("Failed to create UIAutomation");

    // Step 1: Save current focus and caret position (user's current position)
    println!("Step 1: Saving YOUR current focus and caret position...");

    let t_get_focused = Instant::now();
    let user_focused_element: IUIAutomationElement = automation
        .GetFocusedElement()
        .expect("Failed to get focused element");
    println!("  [TIMING] GetFocusedElement: {:?}", t_get_focused.elapsed());

    let name = user_focused_element.CurrentName().unwrap_or_default();
    let control_type = user_focused_element.CurrentControlType().unwrap_or(UIA_CONTROLTYPE_ID(0));
    println!(
        "  [LOG] Your focused element: '{}' (control type: {:?})",
        name.to_string(),
        control_type
    );

    let mut saved_caret_range: Option<IUIAutomationTextRange> = None;

    let t_get_pattern = Instant::now();
    match user_focused_element.GetCurrentPatternAs::<IUIAutomationTextPattern2>(UIA_TextPattern2Id) {
        Ok(text_pattern) => {
            println!("  [TIMING] GetCurrentPatternAs<TextPattern2>: {:?}", t_get_pattern.elapsed());
            println!("  [LOG] Element supports TextPattern2!");
            let mut active = BOOL::default();
            let t_get_caret = Instant::now();
            match text_pattern.GetCaretRange(&mut active) {
                Ok(range) => {
                    println!("  [TIMING] GetCaretRange: {:?}", t_get_caret.elapsed());
                    println!("  [LOG] Caret is active: {}", active.as_bool());

                    let t_clone = Instant::now();
                    if let Ok(cloned) = range.Clone() {
                        println!("  [TIMING] Clone range: {:?}", t_clone.elapsed());
                        saved_caret_range = Some(cloned);
                        println!("  [LOG] Your caret position saved!");
                    }
                }
                Err(e) => println!("  Failed to get caret range: {:?}", e),
            }
        }
        Err(e) => {
            println!("  [LOG] Element does NOT support TextPattern2: {:?}", e);
        }
    }

    // Step 2: Open Notepad and type into it
    println!("\nStep 2: Opening Notepad and typing...");
    Command::new("notepad.exe").spawn().expect("Failed to open notepad");
    thread::sleep(Duration::from_secs(1));

    // Activate Notepad
    match FindWindowW(w!("Notepad"), None) {
        Ok(hwnd) if hwnd != HWND(std::ptr::null_mut()) => {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
        _ => println!("  [LOG] Could not find Notepad window"),
    }
    thread::sleep(Duration::from_millis(500));

    // Type into Notepad
    type_text("This text was typed by automation!");
    println!("  [LOG] Typed into Notepad");
    thread::sleep(Duration::from_millis(300));

    // Step 3: Restore focus and caret to user's original position
    println!("\nStep 3: Restoring YOUR focus and caret position...");

    let t_set_focus = Instant::now();
    match user_focused_element.SetFocus() {
        Ok(_) => {
            println!("  [TIMING] SetFocus: {:?}", t_set_focus.elapsed());
            println!("  [LOG] Focus restored to your element!");
        }
        Err(e) => println!("  SetFocus() failed: {:?}", e),
    }

    // Restore caret position
    if let Some(range) = saved_caret_range {
        let t_select = Instant::now();
        match range.Select() {
            Ok(_) => {
                println!("  [TIMING] Select (restore caret): {:?}", t_select.elapsed());
                println!("  [LOG] Caret restored to your original position!");
            }
            Err(e) => println!("  Failed to restore caret: {:?}", e),
        }
    } else {
        println!("  [LOG] No caret position saved (TextPattern2 not supported)");
    }

    println!("\n  [TIMING] Total operation time: {:?}", total_start.elapsed());

    println!("\n=== Test Complete ===");
    println!("Your caret should be back where it was!");
    println!("Notepad window with automation text is still open.");
}

#[cfg(target_os = "windows")]
fn type_text(text: &str) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    };

    for c in text.chars() {
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0),
                        wScan: c as u16,
                        dwFlags: KEYEVENTF_UNICODE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0),
                        wScan: c as u16,
                        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(target_os = "windows")]
fn send_key(vk: u16) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    };

    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: 0,
                    dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    println!("This test only works on Windows");
}
