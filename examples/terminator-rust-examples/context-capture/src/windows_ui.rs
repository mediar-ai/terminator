use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::os::windows::prelude::*;
use windows::Win32::Foundation::{BOOL, HWND};
use windows::Win32::UI::Accessibility::{
    AccessibleChildren, AccessibleObjectFromWindow, CreateStdAccessibleObject,
    GetRoleTextW, IAccessible, OBJID_CLIENT, OBJID_WINDOW, ROLE_SYSTEM_WINDOW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowTextW, IsWindowVisible,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UIElement {
    pub role: String,
    pub name: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub children: Vec<UIElement>,
}

impl UIElement {
    pub fn new(role: String) -> Self {
        UIElement {
            role,
            name: None,
            value: None,
            description: None,
            children: vec![],
        }
    }
}

// Get the text of the foreground window
fn get_window_text(hwnd: HWND) -> String {
    let mut text: [u16; 512] = [0; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut text) };
    let text = OsString::from_wide(&text[..len as usize])
        .to_string_lossy()
        .to_string();
    text
}

// Get the role text of an IAccessible object
fn get_role_text(role_id: i32) -> String {
    let mut buffer = [0u16; 1024];
    let len = unsafe { GetRoleTextW(role_id, &mut buffer) };
    if len > 0 {
        OsString::from_wide(&buffer[..len as usize])
            .to_string_lossy()
            .to_string()
    } else {
        format!("Role_{}", role_id)
    }
}

// Process an IAccessible object into a UIElement
fn process_accessible(acc: &IAccessible, depth: usize) -> Result<UIElement> {
    // Get role
    let mut role_id: i32 = 0;
    unsafe {
        acc.get_accRole(windows::core::VARIANT::default()).map(|v| {
            if let Some(val) = v.get_i4() {
                role_id = val;
            }
        })
    };
    let role = get_role_text(role_id);
    
    let mut element = UIElement::new(role);
    
    // Get name
    unsafe {
        acc.get_accName(windows::core::VARIANT::default()).map(|v| {
            if let Some(val) = v.get_pstr() {
                element.name = Some(val.to_string());
            }
        })
    };
    
    // Get value
    unsafe {
        acc.get_accValue(windows::core::VARIANT::default()).map(|v| {
            if let Some(val) = v.get_pstr() {
                element.value = Some(val.to_string());
            }
        })
    };
    
    // Get description
    unsafe {
        acc.get_accDescription(windows::core::VARIANT::default()).map(|v| {
            if let Some(val) = v.get_pstr() {
                element.description = Some(val.to_string());
            }
        })
    };
    
    // Stop at a reasonable depth to prevent excessive tree size
    if depth < 5 {
        // Get children
        let mut child_count: i32 = 0;
        unsafe {
            acc.get_accChildCount().map(|count| {
                child_count = count;
            })
        };
        
        if child_count > 0 {
            let max_children = std::cmp::min(child_count, 50) as usize; // Limit to 50 children to prevent huge trees
            let mut children_array = vec![windows::core::VARIANT::default(); max_children];
            let mut obtained = 0;
            
            unsafe {
                AccessibleChildren(
                    acc,
                    0,
                    max_children as i32,
                    children_array.as_mut_ptr(),
                    &mut obtained,
                )
            };
            
            for i in 0..obtained as usize {
                let child_variant = &children_array[i];
                
                if let Some(disp) = child_variant.get_pdispatch() {
                    let child_acc: Result<IAccessible, _> = disp.cast();
                    if let Ok(child_acc) = child_acc {
                        match process_accessible(&child_acc, depth + 1) {
                            Ok(child_element) => element.children.push(child_element),
                            Err(_) => continue,
                        }
                    }
                } else if let Some(child_id) = child_variant.get_i4() {
                    // Handle child elements that are not full IAccessible objects
                    let mut child_element = UIElement::new("ElementChild".to_string());
                    
                    let child_variant = windows::core::VARIANT::from(child_id);
                    
                    unsafe {
                        acc.get_accName(child_variant.clone()).map(|v| {
                            if let Some(val) = v.get_pstr() {
                                child_element.name = Some(val.to_string());
                            }
                        })
                    };
                    
                    unsafe {
                        acc.get_accValue(child_variant.clone()).map(|v| {
                            if let Some(val) = v.get_pstr() {
                                child_element.value = Some(val.to_string());
                            }
                        })
                    };
                    
                    element.children.push(child_element);
                }
            }
        }
    }
    
    Ok(element)
}

// Get the UI tree of the foreground window
pub fn get_foreground_window_tree() -> Result<UIElement> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0 == 0 {
        anyhow::bail!("No foreground window found");
    }
    
    let window_text = get_window_text(hwnd);
    
    // Create the root element for the window
    let mut root = UIElement::new("Window".to_string());
    root.name = Some(window_text);
    
    // Get the accessible object for the window
    let mut window_acc = None;
    unsafe {
        let result = AccessibleObjectFromWindow(
            hwnd,
            OBJID_WINDOW,
            &IAccessible::IID,
            &mut window_acc as *mut _ as *mut _,
        );
        if result.is_err() {
            // If OBJID_WINDOW fails, try OBJID_CLIENT
            let result = AccessibleObjectFromWindow(
                hwnd,
                OBJID_CLIENT,
                &IAccessible::IID,
                &mut window_acc as *mut _ as *mut _,
            );
            if result.is_err() {
                anyhow::bail!("Failed to get accessible object for window");
            }
        }
    }
    
    if let Some(acc) = window_acc {
        // Process the accessible object
        match process_accessible(&acc, 0) {
            Ok(element) => {
                root.children.push(element);
            }
            Err(e) => {
                tracing::warn!("Error processing window accessible object: {:?}", e);
            }
        }
    }
    
    Ok(root)
}

// Capture the full UI tree of the focused application
pub fn capture_focused_app_tree() -> Result<UIElement> {
    get_foreground_window_tree().context("Failed to capture foreground window UI tree")
}
