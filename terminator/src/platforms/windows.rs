use crate::element::UIElementImpl;
use crate::platforms::AccessibilityEngine;
use crate::utils::normalize;
use crate::{AutomationError, Locator, Selector, UIElement, UIElementAttributes};
use crate::{ClickResult, ScreenshotResult};
use image::DynamicImage;
use image::{ImageBuffer, Rgba};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;
use uiautomation::UIAutomation;
use uiautomation::controls::ControlType;
use uiautomation::filters::{ClassNameFilter, ControlTypeFilter, NameFilter, OrFilter};
use uiautomation::inputs::Mouse;
use uiautomation::patterns;
use uiautomation::types::{Point, TreeScope, UIProperty};
use uiautomation::variants::Variant;
use uni_ocr::{OcrEngine, OcrProvider};

// Windows API imports
use windows::core::Error;
use windows::core::HSTRING;
use windows::core::HRESULT;
use windows::core::PWSTR;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::HANDLE;

use windows::Win32::System::Com::CLSCTX_ALL;
use windows::Win32::System::Com::CoCreateInstance;
use windows::Win32::System::Com::CoInitializeEx;
use windows::Win32::System::Com::COINIT_APARTMENTTHREADED;

use windows::Win32::System::Diagnostics::ToolHelp::CreateToolhelp32Snapshot;
use windows::Win32::System::Diagnostics::ToolHelp::Process32FirstW;
use windows::Win32::System::Diagnostics::ToolHelp::Process32NextW;
use windows::Win32::System::Diagnostics::ToolHelp::PROCESSENTRY32W;
use windows::Win32::System::Diagnostics::ToolHelp::TH32CS_SNAPPROCESS;

use windows::Win32::System::Threading::CREATE_NEW_CONSOLE;
use windows::Win32::System::Threading::CreateProcessW;
use windows::Win32::System::Threading::PROCESS_INFORMATION;
use windows::Win32::System::Threading::STARTUPINFOW;

use windows::Win32::UI::Shell::ACTIVATEOPTIONS;
use windows::Win32::UI::Shell::ApplicationActivationManager;
use windows::Win32::UI::Shell::IApplicationActivationManager;


// Define a default timeout duration
const DEFAULT_FIND_TIMEOUT: Duration = Duration::from_millis(5000);

// List of common browser process names (without .exe)
const KNOWN_BROWSER_PROCESS_NAMES: &[&str] = &[
    "chrome", "firefox", "msedge", "iexplore", "opera", "brave", "vivaldi", "browser", "arc", "explorer"
];

// Helper function to get process name by PID using native Windows API
pub fn get_process_name_by_pid(pid: i32) -> Result<String, AutomationError> {
    unsafe {
        // Create a snapshot of all processes
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| AutomationError::PlatformError(format!("Failed to create process snapshot: {}", e)))?;
        
        if snapshot.is_invalid() {
            return Err(AutomationError::PlatformError("Invalid snapshot handle".to_string()));
        }
        
        // Ensure we close the handle when done
        let _guard = HandleGuard(snapshot);
        
        let mut process_entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        
        // Get the first process
        if Process32FirstW(snapshot, &mut process_entry).is_err() {
            return Err(AutomationError::PlatformError("Failed to get first process".to_string()));
        }
        
        // Iterate through processes to find the one with matching PID
        loop {
            if process_entry.th32ProcessID == pid as u32 {
                // Convert the process name from wide string to String
                let name_slice = &process_entry.szExeFile;
                let name_len = name_slice.iter().position(|&c| c == 0).unwrap_or(name_slice.len());
                let process_name = String::from_utf16_lossy(&name_slice[..name_len]);
                
                // Remove .exe extension if present
                let clean_name = process_name
                    .strip_suffix(".exe")
                    .or_else(|| process_name.strip_suffix(".EXE"))
                    .unwrap_or(&process_name);
                
                return Ok(clean_name.to_string());
            }
            
            // Get the next process
            if Process32NextW(snapshot, &mut process_entry).is_err() {
                break;
            }
        }
        
        Err(AutomationError::PlatformError(format!(
            "Process with PID {} not found",
            pid
        )))
    }
}

// RAII guard to ensure handle is closed
struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

// thread-safety
#[derive(Clone)]
pub struct ThreadSafeWinUIAutomation(Arc<UIAutomation>);

// send and sync for wrapper
unsafe impl Send for ThreadSafeWinUIAutomation {}
unsafe impl Sync for ThreadSafeWinUIAutomation {}

#[allow(unused)]
// there is no need of `use_background_apps` or `activate_app`
// windows IUIAutomation will get current running app &
// background running app spontaneously, keeping it anyway!!
pub struct WindowsEngine {
    automation: ThreadSafeWinUIAutomation,
    use_background_apps: bool,
    activate_app: bool,
}

impl WindowsEngine {
    pub fn new(use_background_apps: bool, activate_app: bool) -> Result<Self, AutomationError> {
        let automation =
            UIAutomation::new().map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        let arc_automation = ThreadSafeWinUIAutomation(Arc::new(automation));
        Ok(Self {
            automation: arc_automation,
            use_background_apps,
            activate_app,
        })
    }
}

#[async_trait::async_trait]
impl AccessibilityEngine for WindowsEngine {
    fn get_root_element(&self) -> UIElement {
        let root = self.automation.0.get_root_element().unwrap();
        let arc_root = ThreadSafeWinUIElement(Arc::new(root));
        UIElement::new(Box::new(WindowsUIElement { element: arc_root }))
    }

    fn get_element_by_id(&self, id: i32) -> Result<UIElement, AutomationError> {
        let root_element = self.automation.0.get_root_element().unwrap();
        let condition = self
            .automation
            .0
            .create_property_condition(UIProperty::ProcessId, Variant::from(id), None)
            .unwrap();
        let ele = root_element
            .find_first(TreeScope::Subtree, &condition)
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))?;
        let arc_ele = ThreadSafeWinUIElement(Arc::new(ele));

        Ok(UIElement::new(Box::new(WindowsUIElement {
            element: arc_ele,
        })))
    }

    fn get_focused_element(&self) -> Result<UIElement, AutomationError> {
        let element = self
            .automation
            .0
            .get_focused_element()
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))?;
        let arc_element = ThreadSafeWinUIElement(Arc::new(element));

        Ok(UIElement::new(Box::new(WindowsUIElement {
            element: arc_element,
        })))
    }

    fn get_applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        let root = self.automation.0.get_root_element().unwrap();
        let condition = self
            .automation
            .0
            .create_property_condition(
                UIProperty::ControlType,
                Variant::from(ControlType::Window as i32),
                None,
            )
            .unwrap();
        let elements = root
            .find_all(TreeScope::Subtree, &condition)
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))?;
        let arc_elements: Vec<UIElement> = elements
            .into_iter()
            .map(|ele| {
                let arc_ele = ThreadSafeWinUIElement(Arc::new(ele));
                UIElement::new(Box::new(WindowsUIElement { element: arc_ele }))
            })
            .collect();

        Ok(arc_elements)
    }

    fn get_application_by_name(&self, name: &str) -> Result<UIElement, AutomationError> {
        debug!("searching application from name: {}", name);

        // Strip .exe suffix if present
        let search_name = name
            .strip_suffix(".exe")
            .or_else(|| name.strip_suffix(".EXE")) // Also check uppercase
            .unwrap_or(name);
        debug!("using search name: {}", search_name);

        // first find element by matcher
        let root_ele = self.automation.0.get_root_element().unwrap();
        let search_name_norm = normalize(search_name);
        let matcher = self
            .automation
            .0
            .create_matcher()
            .control_type(ControlType::Window)
            .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                let name = normalize(&e.get_name().unwrap_or_default());
                Ok(name.contains(&search_name_norm))
            }))
            .from_ref(&root_ele)
            .depth(7)
            .timeout(5000);
        let ele_res = matcher
            .find_first()
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()));

        // fallback to find by pid
        let ele = match ele_res {
            Ok(ele) => ele,
            Err(_) => {
                let pid = match get_pid_by_name(search_name) {
                    // Use stripped name
                    Some(pid) => pid,
                    None => {
                        return Err(AutomationError::PlatformError(format!(
                            "no running application found from name: {:?} (searched as: {:?})",
                            name,
                            search_name // Include original name in error
                        )));
                    }
                };
                let condition = self
                    .automation
                    .0
                    .create_property_condition(
                        UIProperty::ProcessId,
                        Variant::from(pid as i32),
                        None,
                    )
                    .unwrap();
                root_ele
                    .find_first(TreeScope::Subtree, &condition)
                    .map_err(|e| AutomationError::ElementNotFound(e.to_string()))?
            }
        };
        let arc_ele = ThreadSafeWinUIElement(Arc::new(ele));
        return Ok(UIElement::new(Box::new(WindowsUIElement {
            element: arc_ele,
        })));
    }

    fn get_application_by_pid(&self, pid: i32, timeout: Option<Duration>) -> Result<UIElement, AutomationError> {
        let root_ele = self.automation.0.get_root_element().unwrap();
        let timeout_ms = timeout.unwrap_or(DEFAULT_FIND_TIMEOUT).as_millis() as u64;
        
        // Create a matcher with timeout
        let matcher = self
            .automation
            .0
            .create_matcher()
            .from_ref(&root_ele)
            .filter(Box::new(OrFilter {
                left: Box::new(ControlTypeFilter {
                    control_type: ControlType::Window,
                }),
                right: Box::new(ControlTypeFilter {
                    control_type: ControlType::Pane,
                }),
            }))
            .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                match e.get_process_id() {
                    Ok(element_pid) => Ok(element_pid == pid as u32),
                    Err(_) => Ok(false),
                }
            }))
            .timeout(timeout_ms);

        let ele = matcher.find_first().map_err(|e| {
            AutomationError::ElementNotFound(format!(
                "Application with PID {} not found within {}ms timeout: {}",
                pid, timeout_ms, e
            ))
        })?;
        
        let arc_ele = ThreadSafeWinUIElement(Arc::new(ele));

        Ok(UIElement::new(Box::new(WindowsUIElement {
            element: arc_ele,
        })))
    }

    fn find_elements(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
        timeout: Option<Duration>,
        depth: Option<usize>,
    ) -> Result<Vec<UIElement>, AutomationError> {
        let root_ele = if let Some(el) = root {
            if let Some(ele) = el.as_any().downcast_ref::<WindowsUIElement>() {
                &ele.element.0
            } else {
                &Arc::new(self.automation.0.get_root_element().unwrap())
            }
        } else {
            &Arc::new(self.automation.0.get_root_element().unwrap())
        };

        let timeout_ms = timeout.unwrap_or(DEFAULT_FIND_TIMEOUT).as_millis() as u32;

        // make condition according to selector
        match selector {
            Selector::Role { role, name } => {
                let win_control_type = map_generic_role_to_win_roles(role);
                debug!(
                    "searching elements by role: {:?} (from: {}), name_filter: {:?}, depth: {:?}, timeout: {}ms, within: {:?}",
                    win_control_type, role, name, depth, timeout_ms, root_ele.get_name().unwrap_or_default()
                );

                let actual_depth = depth.unwrap_or(50) as u32;

                let matcher_builder = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .control_type(win_control_type)
                    .depth(actual_depth)
                    .timeout(timeout_ms as u64);
   
                
                let elements = matcher_builder.find_all().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "Role: '{}' (mapped to {:?}), Name: {:?}, Err: {}",
                        role, win_control_type, name, e
                    ))
                })?;

                debug!("found {} elements with role: {} (mapped to {:?}), name_filter: {:?}", elements.len(), role, win_control_type, name);
                return Ok(elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect());
            }
            Selector::Id(id) => {
                debug!("Searching for element with ID: {}", id);
                // Clone id to move into the closure
                let target_id = id.clone();
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        // Use the common function to generate ID
                        match generate_element_id(e) {
                            Ok(calculated_id) => {
                                let matches = calculated_id.to_string() == target_id;
                                if matches {
                                    debug!("Found matching element with ID: {}", calculated_id);
                                }
                                Ok(matches)
                            },
                            Err(e) => {
                                debug!("Failed to generate ID for element: {}", e);
                                Ok(false)
                            }
                        }
                    }))
                    .timeout(timeout_ms as u64);

                debug!("Starting element search with timeout: {}ms", timeout_ms);
                let elements = matcher.find_all().map_err(|e| {
                    debug!("Element search failed: {}", e);
                    AutomationError::ElementNotFound(format!("ID: '{}', Err: {}", id, e))
                })?;

                debug!("Found {} elements matching ID: {}", elements.len(), id);
                let collected_elements: Vec<UIElement> = elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect();

                return Ok(collected_elements);
            }
            Selector::Name(name) => {
                debug!("searching element by name: {}", name);

                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .contains_name(name)
                    .depth(depth.unwrap_or(50) as u32)
                    .timeout(timeout_ms as u64);

                let elements = matcher.find_all().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "Name: '{}', Err: {}",
                        name,
                        e.to_string()
                    ))
                })?;

                return Ok(elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect());
            }
            Selector::Text(text) => {
                let filter = OrFilter {
                    left: Box::new(NameFilter {
                        value: String::from(text),
                        casesensitive: false,
                        partial: true,
                    }),
                    right: Box::new(ControlTypeFilter {
                        control_type: ControlType::Text,
                    }),
                };
                // Create a matcher that uses contains_name which is more reliable for text searching
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .filter(Box::new(filter)) // This is the key improvement from the example
                    .depth(depth.unwrap_or(50) as u32) // Search deep enough to find most elements
                    .timeout(timeout_ms as u64); // Allow enough time for search

                // Get the first matching element
                let elements = matcher.find_all().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "Text: '{}', Err: {}",
                        text,
                        e.to_string()
                    ))
                })?;

                return Ok(elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect());
            }
            Selector::Path(_) => {
                return Err(AutomationError::UnsupportedOperation(
                    "`Path` selector not supported".to_string(),
                ));
            }
            Selector::NativeId(automation_id) => {    // for windows passing `UIProperty::AutomationID` as `NativeId`
                debug!("searching for elements using AutomationId: {}", automation_id);

                let ele_id = automation_id.clone();
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        match e.get_automation_id() {
                            Ok(id) => {
                                let matches = id == ele_id;
                                if matches {
                                    debug!("found matching elements with AutomationID : {}", ele_id);
                                }
                                Ok(matches)
                            }
                            Err(err) => {
                                debug!("failed to get AutomationId: {}", err);
                                Ok(false)
                            }
                        }
                    }))
                    .timeout(timeout_ms as u64);

                debug!("searching elements with timeout: {}ms", timeout_ms);
                let elements = matcher.find_all().map_err(|e| {
                    debug!("Elements search failed: {}", e);
                    AutomationError::ElementNotFound(format!(
                        "AutomationId: '{}', Err: {}", automation_id, e))
                })?;

                debug!("found {} elements matching AutomationID: {}", elements.len(), automation_id);
                let collected_elements: Vec<UIElement> = elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect();
                return Ok(collected_elements);
            }
            Selector::Attributes(_attributes) => {
                return Err(AutomationError::UnsupportedOperation(
                    "`Attributes` selector not supported".to_string(),
                ));
            }
            Selector::Filter(_filter) => {
                return Err(AutomationError::UnsupportedOperation(
                    "`Filter` selector not supported".to_string(),
                ));
            }
            Selector::Chain(selectors) => {
                if selectors.is_empty() {
                    return Err(AutomationError::InvalidArgument(
                        "Selector chain cannot be empty".to_string(),
                    ));
                }

                // Start with the initial root
                let mut current_roots = if let Some(root) = root {
                    vec![Some(root.clone())]
                } else {
                    vec![None]
                };

                // Iterate through selectors, refining the list of matching elements
                for (i, selector) in selectors.iter().enumerate() {
                    let mut next_roots = Vec::new();
                    let is_last_selector = i == selectors.len() - 1;

                    for root_element in &current_roots {
                        // Find elements matching the current selector within the current root
                        let found_elements = self.find_elements(
                            selector,
                            root_element.as_ref(),
                            timeout,
                            depth,
                        )?;

                        if is_last_selector {
                            // If it's the last selector, collect all found elements
                            next_roots.extend(found_elements.into_iter().map(Some));
                        } else {
                            // If not the last selector, and we found exactly one element,
                            // use it as the root for the next iteration.
                            if found_elements.len() == 1 {
                                next_roots.push(Some(found_elements.into_iter().next().unwrap()));
                            } else {
                                // If 0 or >1 elements found before the last selector,
                                // it means the path diverged or ended. No elements match the full chain.
                                next_roots.clear();
                                break;
                            }
                        }
                    }

                    current_roots = next_roots;
                    if current_roots.is_empty() && !is_last_selector {
                        // If no elements were found matching an intermediate selector, break early.
                        break;
                    }
                }

                // Convert Vec<Option<UIElement>> to Vec<UIElement> by filtering out None values
                return Ok(current_roots.into_iter().filter_map(|x| x).collect());
            }
            Selector::ClassName(classname) => {
                debug!("searching elements by class name: {}", classname);
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .filter(Box::new(ClassNameFilter {
                        classname: classname.clone(),
                    }))
                    .depth(depth.unwrap_or(50) as u32)
                    .timeout(timeout_ms as u64);
                let elements = matcher.find_all().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "ClassName: '{}', Err: {}",
                        classname,
                        e.to_string()
                    ))
                })?;
                return Ok(elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect());
            }
        };


    }

    fn find_element(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
        timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError> {
        let root_ele = if let Some(el) = root {
            if let Some(ele) = el.as_any().downcast_ref::<WindowsUIElement>() {
                &ele.element.0
            } else {
                &Arc::new(self.automation.0.get_root_element().unwrap())
            }
        } else {
            &Arc::new(self.automation.0.get_root_element().unwrap())
        };

        let timeout_ms = timeout.unwrap_or(DEFAULT_FIND_TIMEOUT).as_millis() as u32;

        match selector {
            Selector::Role { role, name } => {
                let win_control_type = map_generic_role_to_win_roles(role);
                debug!(
                    "searching element by role: {:?} (from: {}), name_filter: {:?}, timeout: {}ms, within: {:?}",
                    win_control_type, role, name, timeout_ms, root_ele.get_name().unwrap_or_default()
                );

                let matcher_builder = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .control_type(win_control_type)
                    .depth(50) // Default depth for find_element
                    .timeout(timeout_ms as u64);


                let element = matcher_builder.find_first().map_err(|e| {
                     AutomationError::ElementNotFound(format!(
                        "Role: '{}' (mapped to {:?}), Name: {:?}, Root: {:?}, Err: {}",
                        role, win_control_type, name, root, e
                    ))
                })?;

                let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
                Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_ele,
                })))
            }
            Selector::Id(id) => {
                debug!("Searching for element with ID: {}", id);
                // Clone id to move into the closure
                let target_id = id.clone();
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        // Use the common function to generate ID
                        match generate_element_id(e) {
                            Ok(calculated_id) => {
                                let matches = calculated_id.to_string() == target_id;
                                if matches {
                                    debug!("Found matching element with ID: {}", calculated_id);
                                }
                                Ok(matches)
                            },
                            Err(e) => {
                                debug!("Failed to generate ID for element: {}", e);
                                Ok(false)
                            }
                        }
                    }))
                    .timeout(timeout_ms as u64);

                debug!("Starting element search with timeout: {}ms", timeout_ms);
                let element = matcher.find_first().map_err(|e| {
                    debug!("Element search failed: {}", e);
                    AutomationError::ElementNotFound(format!("ID: '{}', Err: {}", id, e))
                })?;

                debug!("Found element matching ID: {}", id);
                let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
                Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_ele,
                })))
            }
            Selector::Name(name) => {
                // find use create matcher api

                debug!("searching element by name: {}", name);

                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .contains_name(name)
                    .depth(50)
                    .timeout(timeout_ms as u64);

                let element = matcher.find_first().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "Name: '{}', Err: {}",
                        name,
                        e.to_string()
                    ))
                })?;

                let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
                return Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_ele,
                })));
            }
            Selector::Text(text) => {
                let filter = OrFilter {
                    left: Box::new(NameFilter {
                        value: String::from(text),
                        casesensitive: false,
                        partial: true,
                    }),
                    right: Box::new(ControlTypeFilter {
                        control_type: ControlType::Text,
                    }),
                };
                // Create a matcher that uses contains_name which is more reliable for text searching
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .filter(Box::new(filter)) // This is the key improvement from the example
                    .depth(50) // Search deep enough to find most elements
                    .timeout(timeout_ms as u64); // Allow enough time for search

                // Get the first matching element
                let element = matcher.find_first().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "Text: '{}', Root: {:?}, Err: {}",
                        text, root, e
                    ))
                })?;

                let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
                return Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_ele,
                })));
            }
            Selector::Path(_) => {
                return Err(AutomationError::UnsupportedOperation(
                    "`Path` selector not supported".to_string(),
                ));
            }
            Selector::NativeId(automation_id) => {    // for windows passing `UIProperty::AutomationID` as `NativeId`
                debug!("searching for element using AutomationId: {}", automation_id);

                let ele_id = automation_id.clone();
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        match e.get_automation_id() {
                            Ok(id) => {
                                let matches = id == ele_id;
                                if matches {
                                    debug!("found matching element with AutomationID : {}", ele_id);
                                }
                                Ok(matches)
                            }
                            Err(err) => {
                                debug!("failed to get AutomationId: {}", err);
                                Ok(false)
                            }
                        }
                    }))
                    .timeout(timeout_ms as u64);

                debug!("searching element with timeout: {}ms", timeout_ms);

                let element = matcher.find_first().map_err(|e| {
                    debug!("Element search failed: {}", e);
                    AutomationError::ElementNotFound(format!(
                        "AutomationId: '{}', Err: {}", automation_id, e))
                })?;

                let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
                return Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_ele,
                })));
            }
            Selector::Attributes(_attributes) => {
                return Err(AutomationError::UnsupportedOperation(
                    "`Attributes` selector not supported".to_string(),
                ));
            }
            Selector::Filter(_filter) => {
                return Err(AutomationError::UnsupportedOperation(
                    "`Filter` selector not supported".to_string(),
                ));
            }
            Selector::Chain(selectors) => {
                if selectors.is_empty() {
                    return Err(AutomationError::InvalidArgument(
                        "Selector chain cannot be empty".to_string(),
                    ));
                }

                // Recursively find the element by traversing the chain.
                let mut current_element = root.cloned();
                for selector in selectors {
                    let found_element =
                        self.find_element(selector, current_element.as_ref(), timeout)?;
                    current_element = Some(found_element);
                }

                // Return the final single element found after the full chain traversal.
                return current_element.ok_or_else(|| {
                    AutomationError::ElementNotFound(
                        "Element not found after traversing chain".to_string(),
                    )
                });
            }
            Selector::ClassName(classname) => {
                debug!("searching element by class name: {}", classname);
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .filter(Box::new(ClassNameFilter {
                        classname: classname.clone(),
                    }))
                    .depth(50)
                    .timeout(timeout_ms as u64);
                let element = matcher.find_first().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "ClassName: '{}', Err: {}",
                        classname,
                        e.to_string()
                    ))
                })?;
                let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
                return Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_ele,
                })));
            }
        }
    }

    fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError> {
        // Check if this is a UWP app by looking for the 'uwp:' prefix
        if let Some(uwp_app_name) = app_name.strip_prefix("uwp:") {
            launch_uwp_app(self, uwp_app_name)
        } else {
            launch_regular_application(self, app_name)
        }
    }

    fn open_url(&self, url: &str, browser: Option<&str>) -> Result<UIElement, AutomationError> {
        let browser = browser.unwrap_or(""); // when empty it'll open url in system's default browser
        let status = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-WindowStyle",
                "hidden",
                "-Command",
                "start",
                browser,
                url,
            ])
            .status()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        if !status.success() {
            return Err(AutomationError::PlatformError(
                "Failed to open URL".to_string(),
            ));
        }

        std::thread::sleep(std::time::Duration::from_millis(200));

        self.get_application_by_name(browser)
    }

    fn open_file(&self, file_path: &str) -> Result<(), AutomationError> {
        // Use Invoke-Item and explicitly quote the path within the command string.
        // Also use -LiteralPath to prevent PowerShell from interpreting characters in the path.
        // Escape any pre-existing double quotes within the path itself using PowerShell's backtick escape `"
        let command_str = format!(
            "Invoke-Item -LiteralPath \"{}\"",
            file_path.replace('\"', "`\"")
        );
        info!("Running command to open file: {}", command_str);

        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-WindowStyle",
                "hidden",
                "-Command",
                &command_str, // Pass the fully formed command string
            ])
            .output() // Capture output instead of just status
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!(
                "Failed to open file '{}' using Invoke-Item. Stderr: {}",
                file_path, stderr
            );
            return Err(AutomationError::PlatformError(format!(
                "Failed to open file '{}' using Invoke-Item. Error: {}",
                file_path, stderr
            )));
        }
        Ok(())
    }

    async fn run_command(
        &self,
        windows_command: Option<&str>,
        _unix_command: Option<&str>,
    ) -> Result<crate::CommandOutput, AutomationError> {
        let command_str = windows_command.ok_or_else(|| {
            AutomationError::InvalidArgument("Windows command must be provided".to_string())
        })?;

        // Use tokio::process::Command for async execution
        let output = tokio::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-WindowStyle",
                "hidden",
                "-Command",
                command_str,
            ])
            .output()
            .await // Await the async output
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

        Ok(crate::CommandOutput {
            exit_status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    async fn capture_screen(&self) -> Result<ScreenshotResult, AutomationError> {
        let monitors = xcap::Monitor::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitors: {}", e))
        })?;
        let mut primary_monitor: Option<xcap::Monitor> = None;
        for monitor in monitors {
            match monitor.is_primary() {
                Ok(true) => {
                    primary_monitor = Some(monitor);
                    break;
                }
                Ok(false) => continue,
                Err(e) => {
                    return Err(AutomationError::PlatformError(format!(
                        "Error checking monitor primary status: {}",
                        e
                    )));
                }
            }
        }
        let primary_monitor = primary_monitor.ok_or_else(|| {
            AutomationError::PlatformError("Could not find primary monitor".to_string())
        })?;

        let image = primary_monitor.capture_image().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to capture screen: {}", e))
        })?;

        Ok(ScreenshotResult {
            image_data: image.to_vec(),
            width: image.width(),
            height: image.height(),
        })
    }

    async fn capture_monitor_by_name(
        &self,
        name: &str,
    ) -> Result<ScreenshotResult, AutomationError> {
        let monitors = xcap::Monitor::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitors: {}", e))
        })?;
        let mut target_monitor: Option<xcap::Monitor> = None;
        for monitor in monitors {
            match monitor.name() {
                Ok(monitor_name) if monitor_name == name => {
                    target_monitor = Some(monitor);
                    break;
                }
                Ok(_) => continue,
                Err(e) => {
                    return Err(AutomationError::PlatformError(format!(
                        "Error getting monitor name: {}",
                        e
                    )));
                }
            }
        }
        let target_monitor = target_monitor.ok_or_else(|| {
            AutomationError::ElementNotFound(format!("Monitor '{}' not found", name))
        })?;

        let image = target_monitor.capture_image().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to capture monitor '{}': {}", name, e))
        })?;

        Ok(ScreenshotResult {
            image_data: image.to_vec(),
            width: image.width(),
            height: image.height(),
        })
    }

    async fn ocr_image_path(&self, image_path: &str) -> Result<String, AutomationError> {
        // Create a Tokio runtime to run the async OCR operation
        let rt = Runtime::new().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to create Tokio runtime: {}", e))
        })?;

        // Run the async code block on the runtime
        rt.block_on(async {
            let engine = OcrEngine::new(OcrProvider::Auto).map_err(|e| {
                AutomationError::PlatformError(format!("Failed to create OCR engine: {}", e))
            })?;

            let (text, _language, _confidence) = engine // Destructure the tuple
                .recognize_file(image_path)
                .await
                .map_err(|e| {
                    AutomationError::PlatformError(format!("OCR recognition failed: {}", e))
                })?;

            Ok(text) // Return only the text
        })
    }

    async fn ocr_screenshot(
        &self,
        screenshot: &ScreenshotResult,
    ) -> Result<String, AutomationError> {
        // Reconstruct the image buffer from raw data
        let img_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
            screenshot.width,
            screenshot.height,
            screenshot.image_data.clone(), // Clone data into the buffer
        )
        .ok_or_else(|| {
            AutomationError::InvalidArgument(
                "Invalid screenshot data for buffer creation".to_string(),
            )
        })?;

        // Convert to DynamicImage
        let dynamic_image = DynamicImage::ImageRgba8(img_buffer);

        // Directly await the OCR operation within the existing async context
        let engine = OcrEngine::new(OcrProvider::Auto).map_err(|e| {
            AutomationError::PlatformError(format!("Failed to create OCR engine: {}", e))
        })?;

        let (text, _language, _confidence) = engine
            .recognize_image(&dynamic_image) // Use recognize_image
            .await // << Directly await here
            .map_err(|e| {
                AutomationError::PlatformError(format!("OCR recognition failed: {}", e))
            })?;

        Ok(text)
    }

    fn activate_browser_window_by_title(&self, title: &str) -> Result<(), AutomationError> {
        info!(
            "Attempting to activate browser window containing title: {}",
            title
        );
        let root = self
            .automation
            .0
            .get_root_element() // Cache root element lookup
            .map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get root element: {}", e))
            })?;

        // Find top-level windows
        let window_matcher = self
            .automation
            .0
            .create_matcher()
            .from_ref(&root)
            .filter(Box::new(ControlTypeFilter {
                control_type: ControlType::TabItem,
            }))
            .contains_name(title)
            .depth(50)
            .timeout(5000);

        let window = window_matcher.find_first().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to find top-level windows: {}", e))
        })?;

        // TODO: focus part does not work (at least in browser firefox)
        // If find_first succeeds, 'window' is the UIElement. Now try to focus it.
        window.set_focus().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to set focus on window/tab: {}", e))
        })?; // Map focus error

        Ok(()) // If focus succeeds, return Ok
    }

    async fn find_window_by_criteria(
        &self,
        title_contains: Option<&str>,
        timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError> {
        let timeout_duration = timeout.unwrap_or(DEFAULT_FIND_TIMEOUT);
        info!(
            "Searching for window: title_contains={:?}, timeout={:?}",
            title_contains, timeout_duration
        );

        let title_contains = title_contains.unwrap_or_default();

        // first find element by matcher
        let root_ele = self.automation.0.get_root_element().unwrap();
        let automation_engine_instance = WindowsEngine::new(false, false) 
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        let matcher = automation_engine_instance 
            .automation
            .0
            .create_matcher()
            // content type window or pane
            .filter(Box::new(OrFilter {
                left: Box::new(ControlTypeFilter {
                    control_type: ControlType::Window,
                }),
                right: Box::new(ControlTypeFilter {
                    control_type: ControlType::Pane,
                }),
            }))
            .filter(Box::new(OrFilter {
                left: Box::new(NameFilter {
                    value: String::from(title_contains),
                    casesensitive: false,
                    partial: true,
                }),
                right: Box::new(ClassNameFilter {
                    classname: String::from(title_contains),
                }),
            }))
            .from_ref(&root_ele)
            .depth(3)
            .timeout(timeout_duration.as_millis() as u64);
        let ele_res = matcher
            .find_first()
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()));

        return Ok(UIElement::new(Box::new(WindowsUIElement {
            element: ThreadSafeWinUIElement(Arc::new(ele_res.unwrap())),
        })));
    }

    async fn get_current_browser_window(&self) -> Result<UIElement, AutomationError> {
        info!("Attempting to get the current focused browser window.");
        let focused_element_raw = self
            .automation
            .0
            .get_focused_element()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get focused element: {}", e)))?;

        let pid = focused_element_raw.get_process_id().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get process ID for focused element: {}", e))
        })?;

        let process_name_raw = get_process_name_by_pid(pid as i32)?;
        let process_name = process_name_raw.to_lowercase(); // Compare lowercase

        info!("Focused element belongs to process: {} (PID: {})", process_name, pid);

        if KNOWN_BROWSER_PROCESS_NAMES.iter().any(|&browser_name| process_name.contains(browser_name)) {
            // First try to get the focused element's parent chain to find a tab
            let mut current_element = focused_element_raw.clone();
            let mut found_tab = false;
            
            // Walk up the parent chain looking for a TabItem
            for _ in 0..10 { // Limit depth to prevent infinite loops
                if let Ok(control_type) = current_element.get_control_type() {
                    debug!("get_current_browser_window, control_type: {:?}", control_type);
                    if control_type == ControlType::Document {
                        info!("Found browser tab in parent chain");
                        found_tab = true;
                        break;
                    }
                }
                
                match current_element.get_cached_parent() {
                    Ok(parent) => current_element = parent,
                    Err(_) => break,
                }
            }

            if found_tab {
                // If we found a tab, use the focused element
                info!("Using focused element as it's part of a browser tab");
                let arc_focused_element = ThreadSafeWinUIElement(Arc::new(focused_element_raw));
                Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_focused_element,
                })))
            } else {
                // If no tab found, fall back to the main window
                info!("No tab found in parent chain, falling back to main window");
                match self.get_application_by_pid(pid as i32, Some(DEFAULT_FIND_TIMEOUT)) {
                    Ok(app_window_element) => {
                        info!("Successfully fetched main application window for browser");
                        Ok(app_window_element)
                    }
                    Err(e) => {
                        error!("Failed to get application window by PID {} for browser {}: {}. Falling back to focused element.", pid, process_name, e);
                        // Fallback to returning the originally focused element
                        let arc_focused_element = ThreadSafeWinUIElement(Arc::new(focused_element_raw));
                        Ok(UIElement::new(Box::new(WindowsUIElement {
                            element: arc_focused_element,
                        })))
                    }
                }
            }
        } else {
            Err(AutomationError::ElementNotFound(
                "Currently focused window is not a recognized browser.".to_string(),
            ))
        }
    }

    fn activate_application(&self, app_name: &str) -> Result<(), AutomationError> {
        info!("Attempting to activate application by name: {}", app_name);
        // Find the application window first
        let app_element = self.get_application_by_name(app_name)?;

        // Attempt to activate/focus the window
        // Downcast to the specific WindowsUIElement to call set_focus or activate_window
        let win_element_impl = app_element
            .as_any()
            .downcast_ref::<WindowsUIElement>()
            .ok_or_else(|| {
                AutomationError::PlatformError(
                    "Failed to get window element implementation for activation".to_string(),
                )
            })?;

        // Use set_focus, which typically brings the window forward on Windows
        win_element_impl.element.0.set_focus().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to set focus on application window '{}': {}",
                app_name, e
            ))
        })
    }

    async fn get_current_window(&self) -> Result<UIElement, AutomationError> {
        info!("Attempting to get the current focused window.");
        let focused_element_raw = self
            .automation
            .0
            .get_focused_element()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get focused element: {}", e)))?;

        let mut current_element_arc = Arc::new(focused_element_raw);

        for _ in 0..20 { // Max depth to prevent infinite loops
            match current_element_arc.get_control_type() {
                Ok(control_type) => {
                    if control_type == ControlType::Window {
                        let window_ui_element = WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::clone(&current_element_arc)),
                        };
                        return Ok(UIElement::new(Box::new(window_ui_element)));
                    }
                }
                Err(e) => {
                    return Err(AutomationError::PlatformError(format!(
                        "Failed to get control type during window search: {}",
                        e
                    )));
                }
            }

            match current_element_arc.get_cached_parent() {
                Ok(parent_uia_element) => {
                    // Check if parent is same as current (e.g. desktop root's parent is itself)
                    let current_runtime_id = current_element_arc.get_runtime_id().map_err(|e| AutomationError::PlatformError(format!("Failed to get runtime_id for current element: {}", e)))?;
                    let parent_runtime_id = parent_uia_element.get_runtime_id().map_err(|e| AutomationError::PlatformError(format!("Failed to get runtime_id for parent element: {}", e)))?;

                    if parent_runtime_id == current_runtime_id {
                        debug!("Parent element has same runtime ID as current, stopping window search.");
                        break; // Reached the top or a cycle.
                    }
                    current_element_arc = Arc::new(parent_uia_element); // Move to the parent
                }
                Err(_e) => {
                    // No parent found, or error occurred.
                    // This could mean the focused element itself is the top-level window, or it's detached.
                    // We break here and if the loop didn't find a window, we'll return an error below.
                    break;
                }
            }
        }

        Err(AutomationError::ElementNotFound(
            "Could not find a parent window for the focused element.".to_string(),
        ))
    }

    async fn get_current_application(&self) -> Result<UIElement, AutomationError> {
        info!("Attempting to get the current focused application.");
        let focused_element_raw = self
            .automation
            .0
            .get_focused_element()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get focused element: {}", e)))?;

        let pid = focused_element_raw
            .get_process_id()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get PID for focused element: {}", e)))?;

        self.get_application_by_pid(pid as i32, Some(DEFAULT_FIND_TIMEOUT))
    }

    fn get_window_tree_by_title(&self, title: &str) -> Result<crate::UINode, AutomationError> {
        info!("Attempting to get FULL window tree by title: {}", title);
        let root_ele_os = self.automation.0.get_root_element().map_err(|e| {
            error!("Failed to get root element: {}", e);
            AutomationError::PlatformError(format!("Failed to get root element: {}", e))
        })?;

        // Use a more efficient approach: first find windows by control type, then filter by name
        // Search for both Window and Pane control types since some applications use panes as main containers
        let window_matcher = self
            .automation
            .0
            .create_matcher()
            .from_ref(&root_ele_os)
            .filter(Box::new(OrFilter {
                left: Box::new(ControlTypeFilter {
                    control_type: ControlType::Window,
                }),
                right: Box::new(ControlTypeFilter {
                    control_type: ControlType::Pane,
                }),
            }))
            .depth(3) // Limit search depth for performance
            .timeout(3000); // Reduce timeout to 3 seconds

        let windows = window_matcher.find_all().map_err(|e| {
            error!("Failed to find windows: {}", e);
            AutomationError::ElementNotFound(format!("Failed to find windows: {}", e))
        })?;

        info!("Found {} windows to search through", windows.len());

        // Filter windows by title after retrieval (more efficient than filter_fn)
        let title_lower = title.to_lowercase();
        let mut matching_window = None;
        let mut window_names = Vec::new(); // For debugging
        
        for window in windows {
            match window.get_name() {
                Ok(window_name) => {
                    window_names.push(window_name.clone());
                    if window_name.to_lowercase().contains(&title_lower) {
                        info!("Found matching window: '{}' for title: '{}'", window_name, title);
                        matching_window = Some(window);
                        break;
                    }
                }
                Err(e) => {
                    debug!("Failed to get name for window: {}", e);
                }
            }
        }

        let window_element_raw = matching_window.ok_or_else(|| {
            error!("No window found with title containing: '{}'", title);
            debug!("Available window names: {:?}", window_names);
            AutomationError::ElementNotFound(format!(
                "Window with title containing '{}' not found. Available windows: {:?}",
                title, window_names
            ))
        })?;

        info!(
            "Found window with title '{}', ID: {:?}",
            title,
            window_element_raw.get_runtime_id().ok()
        );

        // Wrap the raw OS element into our UIElement
        let window_element_wrapper = UIElement::new(Box::new(WindowsUIElement {
            element: ThreadSafeWinUIElement(Arc::new(window_element_raw)),
        }));

        // Build the FULL UI tree with optimized performance
        info!("Building FULL UI tree with performance optimizations");
        build_full_ui_node_tree_optimized(&window_element_wrapper)
    }

    fn get_window_tree_by_pid_and_title(&self, pid: u32, title: Option<&str>) -> Result<crate::UINode, AutomationError> {
        info!("Attempting to get FULL window tree by PID: {} and title: {:?}", pid, title);
        let root_ele_os = self.automation.0.get_root_element().map_err(|e| {
            error!("Failed to get root element: {}", e);
            AutomationError::PlatformError(format!("Failed to get root element: {}", e))
        })?;

        // First, find all windows for the given process ID
        // Search for both Window and Pane control types since some applications use panes as main containers
        let window_matcher = self
            .automation
            .0
            .create_matcher()
            .from_ref(&root_ele_os)
            .filter(Box::new(OrFilter {
                left: Box::new(ControlTypeFilter {
                    control_type: ControlType::Window,
                }),
                right: Box::new(ControlTypeFilter {
                    control_type: ControlType::Pane,
                }),
            }))
            .depth(3)
            .timeout(3000);

        let windows = window_matcher.find_all().map_err(|e| {
            error!("Failed to find windows: {}", e);
            AutomationError::ElementNotFound(format!("Failed to find windows: {}", e))
        })?;

        info!("Found {} total windows, filtering by PID: {}", windows.len(), pid);

        // Filter windows by process ID first
        let mut pid_matching_windows = Vec::new();
        let mut window_debug_info = Vec::new(); // For debugging

        for window in windows {
            match window.get_process_id() {
                Ok(window_pid) => {
                    let window_name = window.get_name().unwrap_or_else(|_| "Unknown".to_string());
                    window_debug_info.push(format!("PID: {}, Name: {}", window_pid, window_name));
                    
                    if window_pid == pid {
                        pid_matching_windows.push((window, window_name));
                    }
                }
                Err(e) => {
                    debug!("Failed to get process ID for window: {}", e);
                }
            }
        }

        if pid_matching_windows.is_empty() {
            error!("No windows found for PID: {}", pid);
            debug!("Available windows: {:?}", window_debug_info);
            return Err(AutomationError::ElementNotFound(format!(
                "No windows found for process ID {}. Available windows: {:?}",
                pid, window_debug_info
            )));
        }

        info!("Found {} windows for PID: {}", pid_matching_windows.len(), pid);

        // Now filter by title if provided
        let selected_window = if let Some(title) = title {
            let title_lower = title.to_lowercase();
            let mut title_matching_window = None;
            
            info!("Filtering by title: '{}'", title);
            for (window, window_name) in &pid_matching_windows {
                if window_name.to_lowercase().contains(&title_lower) {
                    info!("Found title match: '{}' contains '{}'", window_name, title);
                    title_matching_window = Some(window.clone());
                    break;
                }
            }

            // If title match found, use it; otherwise fall back to first window with matching PID
            match title_matching_window {
                Some(window) => {
                    info!("Using title-matched window");
                    window
                }
                None => {
                    warn!("No title match found for '{}', falling back to first window with PID {}", title, pid);
                    pid_matching_windows[0].0.clone()
                }
            }
        } else {
            info!("No title filter provided, using first window with PID {}", pid);
            pid_matching_windows[0].0.clone()
        };

        let selected_window_name = selected_window.get_name().unwrap_or_else(|_| "Unknown".to_string());
        info!("Selected window: '{}' for PID: {} (title filter: {:?})", 
              selected_window_name, pid, title);

        // Wrap the raw OS element into our UIElement
        let window_element_wrapper = UIElement::new(Box::new(WindowsUIElement {
            element: ThreadSafeWinUIElement(Arc::new(selected_window)),
        }));

        // Build the FULL UI tree with optimized performance
        info!("Building FULL UI tree with performance optimizations for PID: {}", pid);
        build_full_ui_node_tree_optimized(&window_element_wrapper)
    }
}


// Optimized function to build the FULL UI tree with performance improvements
fn build_full_ui_node_tree_optimized(element: &UIElement) -> Result<crate::UINode, AutomationError> {
    use std::time::Instant;
    
    let start_time = Instant::now();
    
    // Configuration for responsiveness (NO LIMITS on tree size/depth)
    let config = TreeBuildingConfig {
        timeout_per_operation_ms: 50,  // Much shorter timeout for faster operations
        yield_every_n_elements: 50,    // Yield more frequently for responsiveness
        prefer_cached_calls: true,      // Prioritize cached API calls
        batch_size: 50,                // Larger batches for efficiency
    };
    
    let mut context = TreeBuildingContext {
        config,
        elements_processed: 0,
        max_depth_reached: 0,
        cache_hits: 0,
        fallback_calls: 0,
        errors_encountered: 0,
    };
    
    info!("Starting FULL tree building (no limits) with caching optimization");
    
    let result = build_ui_node_tree_cached_first(element, 0, &mut context);
    
    let elapsed = start_time.elapsed();
    info!("FULL tree building completed in {:?}. Stats: elements={}, depth={}, cache_hits={}, fallbacks={}, errors={}", 
          elapsed, context.elements_processed, context.max_depth_reached, 
          context.cache_hits, context.fallback_calls, context.errors_encountered);
    
    result
}

// Streamlined configuration focused on performance, not limits
struct TreeBuildingConfig {
    timeout_per_operation_ms: u64,
    yield_every_n_elements: usize,
    prefer_cached_calls: bool,
    batch_size: usize,
}

// Context to track tree building progress (no limits)
struct TreeBuildingContext {
    config: TreeBuildingConfig,
    elements_processed: usize,
    max_depth_reached: usize,
    cache_hits: usize,
    fallback_calls: usize,
    errors_encountered: usize,
}

impl TreeBuildingContext {
    fn should_yield(&self) -> bool {
        self.elements_processed % self.config.yield_every_n_elements == 0 && self.elements_processed > 0
    }
    
    fn increment_element_count(&mut self) {
        self.elements_processed += 1;
    }
    
    fn update_max_depth(&mut self, depth: usize) {
        self.max_depth_reached = self.max_depth_reached.max(depth);
    }
    
    fn increment_cache_hit(&mut self) {
        self.cache_hits += 1;
    }
    
    fn increment_fallback(&mut self) {
        self.fallback_calls += 1;
    }
    
    fn increment_errors(&mut self) {
        self.errors_encountered += 1;
    }
}

fn build_ui_node_tree_cached_first(
    element: &UIElement,
    current_depth: usize,
    context: &mut TreeBuildingContext,
) -> Result<crate::UINode, AutomationError> {
    context.increment_element_count();
    context.update_max_depth(current_depth);
    
    // Yield CPU periodically to prevent freezing while processing everything
    if context.should_yield() {
        debug!("Yielding CPU after processing {} elements at depth {}", context.elements_processed, current_depth);
        thread::sleep(Duration::from_millis(1));
    }
    
    // Get element attributes with caching preference
    let attributes = get_element_attributes_cached_first(element, context)?;
    
    let mut children_nodes = Vec::new();
    
    // Get children with caching strategy
    match get_element_children_cached_first(element, context) {
        Ok(children_elements) => {
            debug!("Processing {} children at depth {} (using caching strategy)", children_elements.len(), current_depth);
            
            // Process children in efficient batches
            for batch in children_elements.chunks(context.config.batch_size) {
                for child_element in batch {
                    match build_ui_node_tree_cached_first(child_element, current_depth + 1, context) {
                        Ok(child_node) => children_nodes.push(child_node),
                        Err(e) => {
                            debug!("Failed to process child element: {}. Continuing with next child.", e);
                            context.increment_errors();
                            // Continue processing - we want the full tree
                        }
                    }
                }
                
                // Small yield between large batches to maintain responsiveness
                if batch.len() == context.config.batch_size && children_elements.len() > context.config.batch_size {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }
        Err(e) => {
            debug!("Failed to get children for element: {}. Proceeding with no children.", e);
            context.increment_errors();
        }
    }
    
    Ok(crate::UINode {
        attributes,
        children: children_nodes,
    })
}

// Get element attributes with caching strategy
fn get_element_attributes_cached_first(element: &UIElement, context: &mut TreeBuildingContext) -> Result<UIElementAttributes, AutomationError> {
    if context.config.prefer_cached_calls {
        // Try to get cached attributes first (much faster)
        match get_cached_attributes_fast(element) {
            Ok(attributes) => {
                context.increment_cache_hit();
                return Ok(attributes);
            }
            Err(_) => {
                // Fallback to regular attributes
                context.increment_fallback();
            }
        }
    }
    
    // Fallback: try regular call first, then timeout version
    match element.attributes() {
        attributes => {
            // Regular attributes call succeeded
            Ok(attributes)
        }
    }
}

// Fast cached attributes extraction
fn get_cached_attributes_fast(element: &UIElement) -> Result<UIElementAttributes, AutomationError> {
    // Try to get the underlying Windows element for direct cached access
    if let Some(win_element) = element.as_any().downcast_ref::<WindowsUIElement>() {
        let mut properties = HashMap::new();
        
        // Use cached getters when available (much faster than live calls)
        let role = win_element.element.0.get_cached_control_type()
            .or_else(|_| win_element.element.0.get_control_type())
            .map(|ct| ct.to_string())
            .unwrap_or_else(|_| "unknown".to_string());
            
        let name = win_element.element.0.get_cached_name()
            .or_else(|_| win_element.element.0.get_name())
            .ok()
            .filter(|s| !s.is_empty());
            
        let automation_id = win_element.element.0.get_cached_automation_id()
            .or_else(|_| win_element.element.0.get_automation_id())
            .ok()
            .filter(|s| !s.is_empty());
        
        let help_text = win_element.element.0.get_cached_help_text()
            .or_else(|_| win_element.element.0.get_help_text())
            .ok()
            .filter(|s| !s.is_empty());
        
        // Get value with cached preference
        let value = win_element.element.0.get_cached_property_value(UIProperty::ValueValue)
            .or_else(|_| win_element.element.0.get_property_value(UIProperty::ValueValue))
            .ok()
            .and_then(|v| v.get_string().ok())
            .filter(|s| !s.is_empty());
        
        // Get label (labeled-by element's name) with cached preference
        let label = win_element.element.0.get_cached_labeled_by()
            .or_else(|_| win_element.element.0.get_labeled_by())
            .ok()
            .and_then(|labeled_element| {
                labeled_element.get_cached_name()
                    .or_else(|_| labeled_element.get_name())
                    .ok()
                    .filter(|s| !s.is_empty())
            });
        
        // Get keyboard focusable with cached preference
        let is_keyboard_focusable = win_element.element.0.get_cached_property_value(UIProperty::IsKeyboardFocusable)
            .or_else(|_| win_element.element.0.get_property_value(UIProperty::IsKeyboardFocusable))
            .ok()
            .and_then(|v| v.try_into().ok());
        
        // Add automation ID to properties if available
        if let Some(aid) = automation_id {
            properties.insert("AutomationId".to_string(), Some(serde_json::Value::String(aid)));
        }
        
        // Add help text to properties if available  
        if let Some(ref ht) = help_text {
            properties.insert("HelpText".to_string(), Some(serde_json::Value::String(ht.clone())));
        }
        
        return Ok(UIElementAttributes {
            role,
            name,
            label,
            value,
            description: help_text,
            properties,
            is_keyboard_focusable,
        });
    }
    
    Err(AutomationError::PlatformError("Cannot access cached attributes".to_string()))
}

// Get element children with caching strategy  
fn get_element_children_cached_first(element: &UIElement, context: &mut TreeBuildingContext) -> Result<Vec<UIElement>, AutomationError> {
    if context.config.prefer_cached_calls {
        // Try cached children first (much faster)
        match get_cached_children_fast(element) {
            Ok(children) => {
                context.increment_cache_hit();
                debug!("Got {} cached children", children.len());
                return Ok(children);
            }
            Err(_) => {
                context.increment_fallback();
                debug!("Cache miss, falling back to live children enumeration");
            }
        }
    }
    
    // Fallback: try regular children call first, then timeout version if needed
    match element.children() {
        Ok(children) => Ok(children),
        Err(_) => {
            // Only use timeout version if regular call fails
            get_element_children_with_timeout(element, Duration::from_millis(context.config.timeout_per_operation_ms))
        }
    }
}

// Fast cached children extraction
fn get_cached_children_fast(element: &UIElement) -> Result<Vec<UIElement>, AutomationError> {
    if let Some(win_element) = element.as_any().downcast_ref::<WindowsUIElement>() {
        // Try cached children first
        match win_element.element.0.get_cached_children() {
            Ok(cached_children) => {
                debug!("Found {} cached children", cached_children.len());
                let ui_children = cached_children
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect();
                return Ok(ui_children);
            }
            Err(_) => {
                // Cache miss, will fallback
            }
        }
    }
    
    Err(AutomationError::PlatformError("Cannot access cached children".to_string()))
}



// Helper function to get element children with timeout
fn get_element_children_with_timeout(element: &UIElement, timeout: Duration) -> Result<Vec<UIElement>, AutomationError> {
    use std::sync::mpsc;
    use std::thread;
    
    let (sender, receiver) = mpsc::channel();
    let element_clone = element.clone();
    
    // Spawn a thread to get children
    thread::spawn(move || {
        let children_result = element_clone.children();
        let _ = sender.send(children_result);
    });
    
    // Wait for result with timeout
    match receiver.recv_timeout(timeout) {
        Ok(Ok(children)) => Ok(children),
        Ok(Err(e)) => Err(e),
        Err(_) => {
            debug!("Timeout getting element children after {:?}", timeout);
            Err(AutomationError::PlatformError("Timeout getting element children".to_string()))
        }
    }
}

// thread-safety
#[derive(Clone)]
pub struct ThreadSafeWinUIElement(Arc<uiautomation::UIElement>);

// send and sync for wrapper
unsafe impl Send for ThreadSafeWinUIElement {}
unsafe impl Sync for ThreadSafeWinUIElement {}

pub struct WindowsUIElement {
    element: ThreadSafeWinUIElement,
}

impl Debug for WindowsUIElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsUIElement").finish()
    }
}

impl UIElementImpl for WindowsUIElement {
    fn object_id(&self) -> usize {
        // Use the common function to generate ID
        generate_element_id(&self.element.0).unwrap_or(0)
    }

    fn id(&self) -> Option<String> {
        Some(self.object_id().to_string())
    }

    fn role(&self) -> String {
        self.element.0.get_control_type().unwrap_or(ControlType::Custom).to_string()
    }

    fn attributes(&self) -> UIElementAttributes {
        let mut properties = HashMap::new();
        // there are alot of properties, including neccessary ones
        // ref: https://docs.rs/uiautomation/0.16.1/uiautomation/types/enum.UIProperty.html
        let property_list = vec![
            UIProperty::HelpText,
            UIProperty::AutomationId,
        ];
        
        // Helper function to format property values properly
        fn format_property_value(value: &Variant) -> Option<serde_json::Value> {
            // First try to get as string
            if let Ok(s) = value.get_string() {
                if !s.is_empty() {
                    return Some(serde_json::Value::String(s));
                } else {
                    return None; // Empty string - don't include
                }
            }
            
            // If string conversion fails, we'll just skip this property
            // to avoid the STRING() issue
            None
        }
        
        for property in property_list {
            if let Ok(value) = self.element.0.get_property_value(property) {
                if let Some(formatted_value) = format_property_value(&value) {
                    properties.insert(format!("{:?}", property), Some(formatted_value));
                }
                // If format_property_value returns None, we don't insert anything
            }
        }
        
        // Helper function to filter empty strings
        fn filter_empty_string(s: Option<String>) -> Option<String> {
            s.filter(|s| !s.is_empty())
        }
        
        UIElementAttributes {
            role: self.role(),
            name: filter_empty_string(self.element.0.get_name().ok()),
            label: self
                .element
                .0
                .get_labeled_by()
                .ok()
                .and_then(|e| filter_empty_string(e.get_name().ok())),
            value: self
                .element
                .0
                .get_property_value(UIProperty::ValueValue)
                .ok()
                .and_then(|v| filter_empty_string(v.get_string().ok())),
            description: filter_empty_string(self.element.0.get_help_text().ok()),
            properties,
            is_keyboard_focusable: self.is_keyboard_focusable().ok(),
        }
    }

    fn children(&self) -> Result<Vec<UIElement>, AutomationError> {
        // Try getting cached children first
        let children_result = self.element.0.get_cached_children();

        let children = match children_result {
            Ok(cached_children) => {
                info!("Found {} cached children.", cached_children.len());
                cached_children
            }
            Err(cache_err) => {
                // Fallback logic (similar to explore_element_children)
                match uiautomation::UIAutomation::new() {
                    Ok(temp_automation) => {
                        match temp_automation.create_true_condition() {
                            Ok(true_condition) => {
                                self.element
                                    .0
                                    .find_all(uiautomation::types::TreeScope::Children, &true_condition)
                                    .map_err(|find_err| {
                                        error!(
                                            "Failed to get children via find_all fallback: CacheErr={}, FindErr={}",
                                            cache_err, find_err
                                        );
                                        AutomationError::PlatformError(format!(
                                            "Failed to get children (cached and non-cached): {}",
                                            find_err
                                        ))
                                    })? // Propagate error
                            }
                            Err(cond_err) => {
                                error!(
                                    "Failed to create true condition for child fallback: {}",
                                    cond_err
                                );
                                return Err(AutomationError::PlatformError(format!(
                                    "Failed to create true condition for fallback: {}",
                                    cond_err
                                )));
                            }
                        }
                    }
                    Err(auto_err) => {
                        error!(
                            "Failed to create temporary UIAutomation for child fallback: {}",
                            auto_err
                        );
                        return Err(AutomationError::PlatformError(format!(
                            "Failed to create temp UIAutomation for fallback: {}",
                            auto_err
                        )));
                    }
                }
            }
        };

        // Wrap the platform elements into our UIElement trait objects
        Ok(children
            .into_iter()
            .map(|ele| {
                UIElement::new(Box::new(WindowsUIElement {
                    element: ThreadSafeWinUIElement(Arc::new(ele)),
                }))
            })
            .collect())
    }

    fn parent(&self) -> Result<Option<UIElement>, AutomationError> {
        let parent = self.element.0.get_cached_parent();
        match parent {
            Ok(par) => {
                let par_ele = UIElement::new(Box::new(WindowsUIElement {
                    element: ThreadSafeWinUIElement(Arc::new(par)),
                }));
                Ok(Some(par_ele))
            }
            Err(e) => Err(AutomationError::ElementNotFound(e.to_string())),
        }
    }

    fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError> {
        let rect = self
            .element
            .0
            .get_bounding_rectangle()
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))?;
        Ok((
            rect.get_left() as f64,
            rect.get_top() as f64,
            rect.get_width() as f64,
            rect.get_height() as f64,
        ))
    }

    fn click(&self) -> Result<ClickResult, AutomationError> {
        self.element.0.try_focus();
        debug!("attempting to click element: {:?}", self.element.0);

        let click_result = self.element.0.click();

        if click_result.is_ok() {
            return Ok(ClickResult {
                method: "Single Click".to_string(),
                coordinates: None,
                details: "Clicked by Mouse".to_string(),
            });
        }
        // First try using the standard clickable point
        let click_result = self
            .element
            .0
            .get_clickable_point()
            .and_then(|maybe_point| {
                if let Some(point) = maybe_point {
                    debug!("using clickable point: {:?}", point);
                    let mouse = Mouse::default();
                    mouse.click(point).map(|_| ClickResult {
                        method: "Single Click (Clickable Point)".to_string(),
                        coordinates: Some((point.get_x() as f64, point.get_y() as f64)),
                        details: "Clicked by Mouse using element's clickable point".to_string(),
                    })
                } else {
                    Err(
                        AutomationError::PlatformError("No clickable point found".to_string())
                            .to_string()
                            .into(),
                    )
                }
            });

        // If first method fails, try using the bounding rectangle
        if let Err(_) = click_result {
            debug!("clickable point unavailable, falling back to bounding rectangle");
            if let Ok(rect) = self.element.0.get_bounding_rectangle() {
                println!("bounding rectangle: {:?}", rect);
                // Calculate center point of the element
                let center_x = rect.get_left() + rect.get_width() / 2;
                let center_y = rect.get_top() + rect.get_height() / 2;

                let point = Point::new(center_x, center_y);
                let mouse = Mouse::default();

                debug!("clicking at center point: ({}, {})", center_x, center_y);
                mouse
                    .click(point)
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

                return Ok(ClickResult {
                    method: "Single Click (Fallback)".to_string(),
                    coordinates: Some((center_x as f64, center_y as f64)),
                    details: "Clicked by Mouse using element's center coordinates".to_string(),
                });
            }
        }

        // Return the result of the first attempt or propagate the error
        click_result.map_err(|e| AutomationError::PlatformError(e.to_string()))
    }

    fn double_click(&self) -> Result<ClickResult, AutomationError> {
        self.element.0.try_focus();
        let point = self
            .element
            .0
            .get_clickable_point()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?
            .ok_or_else(|| {
                AutomationError::PlatformError("No clickable point found".to_string())
            })?;
        let mouse = Mouse::default();
        mouse
            .double_click(point)
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        Ok(ClickResult {
            method: "Double Click".to_string(),
            coordinates: Some((point.get_x() as f64, point.get_y() as f64)),
            details: "Clicked by Mouse".to_string(),
        })
    }

    fn right_click(&self) -> Result<(), AutomationError> {
        self.element.0.try_focus();
        let point = self
            .element
            .0
            .get_clickable_point()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?
            .ok_or_else(|| {
                AutomationError::PlatformError("No clickable point found".to_string())
            })?;
        let mouse = Mouse::default();
        mouse
            .right_click(point)
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        Ok(())
    }

    fn hover(&self) -> Result<(), AutomationError> {
        return Err(AutomationError::UnsupportedOperation(
            "`hover` doesn't not support".to_string(),
        ));
    }

    fn focus(&self) -> Result<(), AutomationError> {
        self.element
            .0
            .set_focus()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))
    }

    fn activate_window(&self) -> Result<(), AutomationError> {
        // On Windows, setting focus on an element within the window
        // typically brings the window to the foreground.
        debug!(
            "Activating window by focusing element: {:?}",
            self.element.0
        );
        self.focus()
    }

    fn type_text(&self, text: &str, use_clipboard: bool) -> Result<(), AutomationError> {
        let control_type = self
            .element
            .0
            .get_control_type()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        
        debug!("typing text with control_type: {:#?}, use_clipboard: {}", control_type, use_clipboard);

        if use_clipboard {
            self.element
                .0
                .send_text_by_clipboard(text)
                .map_err(|e| AutomationError::PlatformError(e.to_string()))
        } else {
            // Use standard typing method
            self.element
                .0
                .send_text(text, 10)
                .map_err(|e| AutomationError::PlatformError(e.to_string()))
        }
    }

    fn press_key(&self, key: &str) -> Result<(), AutomationError> {
        let control_type = self
            .element
            .0
            .get_control_type()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get control type: {:?}", e)))?;
        // check if element accepts input, similar :D
        debug!("pressing key with control_type: {:#?}", control_type);
        self.element
            .0
            .send_keys(key, 10)
            .map_err(|e| AutomationError::PlatformError(format!("Failed to press key: {:?}", e)))
    }

    fn get_text(&self, max_depth: usize) -> Result<String, AutomationError> {
        let mut all_texts = Vec::new();

        // Create a function to extract text recursively
        fn extract_text_from_element(
            element: &uiautomation::UIElement,
            texts: &mut Vec<String>,
            current_depth: usize,
            max_depth: usize,
        ) -> Result<(), AutomationError> {
            if current_depth > max_depth {
                return Ok(());
            }


            // Check Value property
            if let Ok(value) = element.get_property_value(UIProperty::ValueValue) {
                if let Ok(value_text) = value.get_string() {
                    if !value_text.is_empty() {
                        debug!("found text in value property: {:?}", &value_text);
                        texts.push(value_text);
                    }
                }
            }

            // Recursively process children
            let children_result = element.get_cached_children();

            let children_to_process = match children_result {
                Ok(cached_children) => {
                    info!(
                        "Found {} cached children for text extraction.",
                        cached_children.len()
                    );
                    cached_children
                }
                Err(cache_err) => {
                    debug!(
                        "Failed to get cached children for text extraction ({}), falling back to non-cached TreeScope::Children search.",
                        cache_err
                    );
                    // Need a UIAutomation instance to create conditions for find_all
                    // Create a temporary instance here for the fallback.
                    // Note: Creating a new UIAutomation instance here might be inefficient.
                    // Consider passing it down or finding another way if performance is critical.
                    match uiautomation::UIAutomation::new() {
                        Ok(temp_automation) => {
                            match temp_automation.create_true_condition() {
                                Ok(true_condition) => {
                                    // Perform the non-cached search for direct children
                                    match element.find_all(
                                        uiautomation::types::TreeScope::Children,
                                        &true_condition,
                                    ) {
                                        Ok(found_children) => {
                                            debug!(
                                                "Found {} non-cached children for text extraction via fallback.",
                                                found_children.len()
                                            );
                                            found_children
                                        }
                                        Err(find_err) => {
                                            error!(
                                                "Failed to get children via find_all fallback for text extraction: CacheErr={}, FindErr={}",
                                                cache_err, find_err
                                            );
                                            // Return an empty vec to avoid erroring out the whole text extraction
                                            vec![]
                                        }
                                    }
                                }
                                Err(cond_err) => {
                                    error!(
                                        "Failed to create true condition for child fallback in text extraction: {}",
                                        cond_err
                                    );
                                    vec![] // Return empty vec on condition creation error
                                }
                            }
                        }
                        Err(auto_err) => {
                            error!(
                                "Failed to create temporary UIAutomation for child fallback in text extraction: {}",
                                auto_err
                            );
                            vec![] // Return empty vec on automation creation error
                        }
                    }
                }
            };

            // Process the children (either cached or found via fallback)
            for child in children_to_process {
                let _ = extract_text_from_element(&child, texts, current_depth + 1, max_depth);
            }

            Ok(())
        }

        // Extract text from the element and its descendants
        extract_text_from_element(&self.element.0, &mut all_texts, 0, max_depth)?;

        // Join the texts with spaces
        Ok(all_texts.join(" "))
    }

    fn set_value(&self, value: &str) -> Result<(), AutomationError> {
        let value_par = self
            .element
            .0
            .get_pattern::<patterns::UIValuePattern>()
            .map_err(|e| AutomationError::PlatformError(e.to_string()));
        debug!(
            "setting value: {:#?} to ui element {:#?}",
            &value, &self.element.0
        );

        if let Ok(v) = value_par {
            v.set_value(value)
                .map_err(|e| AutomationError::PlatformError(e.to_string()))
        } else {
            Err(AutomationError::PlatformError(
                "`UIValuePattern` is not found".to_string(),
            ))
        }
    }

    fn is_enabled(&self) -> Result<bool, AutomationError> {
        self.element
            .0
            .is_enabled()
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))
    }

    fn is_visible(&self) -> Result<bool, AutomationError> {
        // offscreen means invisible, right?
        self.element
            .0
            .is_offscreen()
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))
    }

    fn is_focused(&self) -> Result<bool, AutomationError> {
        // The original implementation was inefficient:
        // It created a new WindowsEngine and compared the focused element's Arc pointer,
        // which is not reliable and very slow.
        // The uiautomation::UIElement provides a direct has_keyboard_focus() method.
        self.element.0.has_keyboard_focus().map_err(|e| AutomationError::PlatformError(format!("Failed to get keyboard focus state: {}", e)))
    }

    fn perform_action(&self, action: &str) -> Result<(), AutomationError> {
        // actions those don't take args
        match action {
            "focus" => self.focus(),
            "invoke" => {
                let invoke_pat = self
                    .element
                    .0
                    .get_pattern::<patterns::UIInvokePattern>()
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
                invoke_pat
                    .invoke()
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))
            }
            "click" => self.click().map(|_| ()),
            "double_click" => self.double_click().map(|_| ()),
            "right_click" => self.right_click().map(|_| ()),
            "toggle" => {
                let toggle_pattern = self
                    .element
                    .0
                    .get_pattern::<patterns::UITogglePattern>()
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
                toggle_pattern
                    .toggle()
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))
            }
            "expand_collapse" => {
                let expand_collapse_pattern = self
                    .element
                    .0
                    .get_pattern::<patterns::UIExpandCollapsePattern>()
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
                expand_collapse_pattern
                    .expand()
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))
            }
            _ => Err(AutomationError::UnsupportedOperation(format!(
                "action '{}' not supported",
                action
            ))),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn create_locator(&self, selector: Selector) -> Result<Locator, AutomationError> {
        let automation = WindowsEngine::new(false, false)
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

        let attrs = self.attributes();
        debug!(
            "creating locator for element: control_type={:#?}, label={:#?}",
            attrs.role, attrs.label
        );

        let self_element = UIElement::new(Box::new(WindowsUIElement {
            element: self.element.clone(),
        }));

        Ok(Locator::new(std::sync::Arc::new(automation), selector).within(self_element))
    }

    fn clone_box(&self) -> Box<dyn UIElementImpl> {
        Box::new(WindowsUIElement {
            element: self.element.clone(),
        })
    }

    fn scroll(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        // First try to focus the element
        self.focus().map_err(|e| AutomationError::PlatformError(format!("Failed to focus element: {:?}", e)))?;

        // Only support up/down directions
        match direction {
            "up" | "down" => {
                // Convert amount to number of key presses (round to nearest integer)
                let times = amount.abs().round() as usize;
                if times == 0 {
                    return Ok(());
                }

                // Send the appropriate key based on direction
                let key = if direction == "up" { "{page_up}" } else { "{page_down}" };
                for _ in 0..times {
                    self.press_key(key)?;
                }
            },
            _ => return Err(AutomationError::UnsupportedOperation(
                "Only 'up' and 'down' scroll directions are supported".to_string(),
            )),
        }
        Ok(())
    }

    fn is_keyboard_focusable(&self) -> Result<bool, AutomationError> {
        let variant = self
            .element
            .0
            .get_property_value(UIProperty::IsKeyboardFocusable)
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        variant.try_into().map_err(|e| AutomationError::PlatformError(format!("Failed to convert IsKeyboardFocusable to bool: {:?}", e)))
    }

    // New method for mouse drag
    fn mouse_drag(&self, start_x: f64, start_y: f64, end_x: f64, end_y: f64) -> Result<(), AutomationError> {
        use std::thread::sleep;
        use std::time::Duration;
        self.mouse_click_and_hold(start_x, start_y)?;
        sleep(Duration::from_millis(20));
        self.mouse_move(end_x, end_y)?;
        sleep(Duration::from_millis(20));
        self.mouse_release()?;
        Ok(())
    }

    // New mouse control methods
    fn mouse_click_and_hold(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_MOVE, MOUSEEVENTF_ABSOLUTE, MOUSEINPUT, SendInput,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        fn to_absolute(x: f64, y: f64) -> (i32, i32) {
            let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
            let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
            let abs_x = ((x / screen_w as f64) * 65535.0).round() as i32;
            let abs_y = ((y / screen_h as f64) * 65535.0).round() as i32;
            (abs_x, abs_y)
        }
        let (abs_x, abs_y) = to_absolute(x, y);
        let move_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: abs_x,
                    dy: abs_y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let down_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_LEFTDOWN,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[move_input], std::mem::size_of::<INPUT>() as i32);
            SendInput(&[down_input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }
    fn mouse_move(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_MOVE, MOUSEEVENTF_ABSOLUTE, MOUSEINPUT, SendInput,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        fn to_absolute(x: f64, y: f64) -> (i32, i32) {
            let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
            let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
            let abs_x = ((x / screen_w as f64) * 65535.0).round() as i32;
            let abs_y = ((y / screen_h as f64) * 65535.0).round() as i32;
            (abs_x, abs_y)
        }
        let (abs_x, abs_y) = to_absolute(x, y);
        let move_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: abs_x,
                    dy: abs_y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[move_input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }
    fn mouse_release(&self) -> Result<(), AutomationError> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTUP, MOUSEINPUT, SendInput,
        };
        let up_input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_LEFTUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[up_input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }


    fn application(&self) -> Result<Option<UIElement>, AutomationError> {
        // Get the process ID of the current element
        let pid = self.element.0.get_process_id().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get process ID for element: {}", e))
        })?;

        // Create a WindowsEngine instance to use its methods.
        // This follows the pattern in `create_locator` but might be inefficient if called frequently.
        let engine = WindowsEngine::new(false, false).map_err(|e| {
            AutomationError::PlatformError(format!("Failed to create WindowsEngine: {}", e))
        })?;

        // Get the application element by PID
        match engine.get_application_by_pid(pid as i32, Some(DEFAULT_FIND_TIMEOUT)) { // Cast pid to i32
            Ok(app_element) => Ok(Some(app_element)),
            Err(AutomationError::ElementNotFound(_)) => {
                // If the specific application element is not found by PID, return None.
                debug!("Application element not found for PID {}", pid);
                Ok(None)
            }
            Err(e) => Err(e), // Propagate other errors
        }
    }

    fn window(&self) -> Result<Option<UIElement>, AutomationError> {
        let mut current_element_arc = Arc::clone(&self.element.0); // Start with the current element's Arc<uiautomation::UIElement>
        const MAX_DEPTH: usize = 20; // Safety break for parent traversal

        for i in 0..MAX_DEPTH {
            // Check current element's control type
            match current_element_arc.get_control_type() {
                Ok(control_type) => {
                    if control_type == ControlType::Window {
                        // Found the window
                        let window_ui_element = WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::clone(&current_element_arc)),
                        };
                        return Ok(Some(UIElement::new(Box::new(window_ui_element))));
                    }
                }
                Err(e) => {
                    return Err(AutomationError::PlatformError(format!(
                        "Failed to get control type for element during window search (iteration {}): {}",
                        i, e
                    )));
                }
            }

            // Try to get the parent
            match current_element_arc.get_cached_parent() {
                Ok(parent_uia_element) => {
                    // Check if parent is same as current (e.g. desktop root's parent is itself)
                    // This requires getting runtime IDs, which can also fail.
                    let current_runtime_id = current_element_arc.get_runtime_id().map_err(|e| AutomationError::PlatformError(format!("Failed to get runtime_id for current element: {}", e)))?;
                    let parent_runtime_id = parent_uia_element.get_runtime_id().map_err(|e| AutomationError::PlatformError(format!("Failed to get runtime_id for parent element: {}", e)))?;

                    if parent_runtime_id == current_runtime_id {
                        debug!("Parent element has same runtime ID as current, stopping window search.");
                        break; // Reached the top or a cycle.
                    }
                    current_element_arc = Arc::new(parent_uia_element); // Move to the parent
                }
                Err(e) => {
                    // No cached parent found or error occurred.
                    debug!("No cached parent found or error during window search (iteration {}): {}. Stopping traversal.", i, e);
                    break;
                }
            }
        }
        // If loop finishes, no element with ControlType::Window was found.
        Ok(None)
    }

    fn highlight(&self, color: Option<u32>, duration: Option<std::time::Duration>) -> Result<(), AutomationError> {
        use windows::Win32::Graphics::Gdi::{
            GetDC, ReleaseDC, CreatePen, SelectObject, DeleteObject, Rectangle,
            PS_SOLID, NULL_BRUSH, GetStockObject, HGDIOBJ
        };
        use windows::Win32::Foundation::{COLORREF, POINT};
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        use std::time::Instant;

        self.element.0.try_focus();

        // Get the element's bounding rectangle
        let rect = self.element.0.get_bounding_rectangle().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get element bounds: {}", e))
        })?;

        // Helper function to get scale factor from cursor position
        fn get_scale_factor_from_cursor() -> f64 {
            let mut point = POINT { x: 0, y: 0 };
            unsafe {
                let _ = GetCursorPos(&mut point);
            }
            match xcap::Monitor::from_point(point.x, point.y) {
                Ok(monitor) => match monitor.scale_factor() {
                    Ok(factor) => factor as f64,
                    Err(e) => {
                        error!("Failed to get scale factor from cursor position: {}", e);
                        1.0 // Fallback to default scale factor
                    }
                },
                Err(e) => {
                    error!("Failed to get monitor from cursor position: {}", e);
                    1.0 // Fallback to default scale factor
                }
            }
        }

        // Helper function to get scale factor from focused window
        fn get_scale_factor_from_focused_window() -> Option<f64> {
            match xcap::Window::all() {
                Ok(windows) => {
                    windows.iter()
                        .find(|w| w.is_focused().unwrap_or(false))
                        .and_then(|focused_window| {
                            focused_window.current_monitor().ok()
                        })
                        .and_then(|monitor| {
                            monitor.scale_factor().ok().map(|factor| factor as f64)
                        })
                },
                Err(e) => {
                    error!("Failed to get windows: {}", e);
                    None
                }
            }
        }

        // Try to get scale factor from focused window first, fall back to cursor position
        let scale_factor = get_scale_factor_from_focused_window()
            .unwrap_or_else(get_scale_factor_from_cursor);

        // Constants for border appearance
        const BORDER_SIZE: i32 = 4;
        const DEFAULT_RED_COLOR: u32 = 0x0000FF; // Pure red in BGR format

        // Use provided color or default to red
        let highlight_color = color.unwrap_or(DEFAULT_RED_COLOR);

        // Scale the coordinates and dimensions
        let x = (rect.get_left() as f64 * scale_factor) as i32;
        let y = (rect.get_top() as f64 * scale_factor) as i32;
        let width = (rect.get_width() as f64 * scale_factor) as i32;
        let height = (rect.get_height() as f64 * scale_factor) as i32;

        // Spawn a thread to handle the highlighting
        thread::spawn(move || {
            let start_time = Instant::now();
            let duration = duration.unwrap_or(std::time::Duration::from_millis(500));

            while start_time.elapsed() < duration {
                // Get the screen DC
                let hdc = unsafe { GetDC(None) };
                if hdc.0.is_null() {
                    return;
                }

                unsafe {
                    // Create a pen for drawing with the specified color
                    let hpen = CreatePen(PS_SOLID, BORDER_SIZE, COLORREF(highlight_color));
                    if hpen.0.is_null() {
                        ReleaseDC(None, hdc);
                        return;
                    }

                    // Save current objects
                    let old_pen = SelectObject(hdc, HGDIOBJ(hpen.0));
                    let null_brush = GetStockObject(NULL_BRUSH);
                    let old_brush = SelectObject(hdc, null_brush);

                    // Draw the border rectangle
                    let _ = Rectangle(hdc, x, y, x + width, y + height);

                    // Restore original objects and clean up
                    SelectObject(hdc, old_brush);
                    SelectObject(hdc, old_pen);
                    let _ = DeleteObject(HGDIOBJ(hpen.0));
                    ReleaseDC(None, hdc);
                }
            }
        });

        Ok(())
    }
    fn process_id(&self) -> Result<u32, AutomationError> {
        self.element.0.get_process_id().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get process ID for element: {}", e))
        })
    }

    fn capture(&self) -> Result<ScreenshotResult, AutomationError> {
        // Get the raw UIAutomation bounds
        let rect = self.element.0.get_bounding_rectangle()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get bounding rectangle: {}", e)))?;

        // Get all monitors that intersect with the element
        let mut intersected_monitors = Vec::new();
        let monitors = xcap::Monitor::all()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitors: {}", e)))?;

        for monitor in monitors {
            let monitor_x = monitor.x()
                .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitor x: {}", e)))? as i32;
            let monitor_y = monitor.y()
                .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitor y: {}", e)))? as i32;
            let monitor_width = monitor.width()
                .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitor width: {}", e)))? as i32;
            let monitor_height = monitor.height()
                .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitor height: {}", e)))? as i32;

            // Check if element intersects with this monitor
            if rect.get_left() < monitor_x + monitor_width &&
               rect.get_left() + rect.get_width() as i32 > monitor_x &&
               rect.get_top() < monitor_y + monitor_height &&
               rect.get_top() + rect.get_height() as i32 > monitor_y {
                intersected_monitors.push(monitor);
            }
        }

        if intersected_monitors.is_empty() {
            return Err(AutomationError::PlatformError("Element is not visible on any monitor".to_string()));
        }

        // If element spans multiple monitors, capture from the primary monitor
        let monitor = &intersected_monitors[0];
        let scale_factor = monitor.scale_factor()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get scale factor: {}", e)))?;

        // Get monitor bounds
        let monitor_x = monitor.x()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitor x: {}", e)))? as u32;
        let monitor_y = monitor.y()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitor y: {}", e)))? as u32;
        let monitor_width = monitor.width()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitor width: {}", e)))? as u32;
        let monitor_height = monitor.height()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitor height: {}", e)))? as u32;

        // Calculate scaled coordinates
        let scaled_x = (rect.get_left() as f64 * scale_factor as f64) as u32;
        let scaled_y = (rect.get_top() as f64 * scale_factor as f64) as u32;
        let scaled_width = (rect.get_width() as f64 * scale_factor as f64) as u32;
        let scaled_height = (rect.get_height() as f64 * scale_factor as f64) as u32;

        // Convert to relative coordinates for capture_region
        let rel_x = if scaled_x >= monitor_x { scaled_x - monitor_x } else { 0 };
        let rel_y = if scaled_y >= monitor_y { scaled_y - monitor_y } else { 0 };
        
        // Ensure width and height don't exceed monitor bounds
        let rel_width = std::cmp::min(scaled_width, monitor_width - rel_x);
        let rel_height = std::cmp::min(scaled_height, monitor_height - rel_y);

        // Capture the screen region
        let capture = monitor.capture_region(
            rel_x,
            rel_y,
            rel_width,
            rel_height
        ).map_err(|e| AutomationError::PlatformError(format!("Failed to capture region: {}", e)))?;

        Ok(ScreenshotResult {
            image_data: capture.to_vec(),
            width: rel_width,
            height: rel_height,
        })
    } 
}

#[allow(dead_code)]
#[repr(i32)]
pub enum ActivateOptions {
    None = 0x00000000,
    DesignMode = 0x00000001,
    NoErrorUI = 0x00000002,
    NoSplashScreen = 0x00000004,
}

impl From<windows::core::Error> for AutomationError {
    fn from(error: windows::core::Error) -> Self {
        AutomationError::PlatformError(error.to_string())
    }
}

// Launches a UWP application and returns its UIElement
fn launch_uwp_app(engine: &WindowsEngine, uwp_app_name: &str) -> Result<UIElement, AutomationError> {
    // First try to get app info using Get-StartApps
    let (app_user_model_id, display_name) = match get_uwp_app_info_from_startapps(uwp_app_name) {
        Ok(info) => info,
        Err(_) => {
            // Fallback to AppX package approach
            debug!("Failed to find app in Get-StartApps, falling back to AppX package search");
            let package = get_uwp_package_info(uwp_app_name)?;
    
    // Get package full name and family name
            let (package_full_name, package_family_name) = get_package_info(&package)?;
    
    // Get the app ID and display name
            let (app_id, display_name) = get_uwp_info(&package_full_name)?;

    // Construct the app user model ID
            let app_user_model_id = format!(
                "{}!{}",
                package_family_name.trim(),
                app_id.trim()
            );
            (app_user_model_id, display_name)
        }
    };

    // Launch the UWP app using Windows API
    let pid = unsafe {
        // Initialize COM with proper error handling
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_err() && hr != HRESULT(0x80010106u32 as i32) {
            // Only return error if it's not the "already initialized" case
            return Err(AutomationError::PlatformError(format!("Failed to initialize COM: {}", hr)));
        }
        // If we get here, either initialization succeeded or it was already initialized
        if hr == HRESULT(0x80010106u32 as i32) {
            debug!("COM already initialized in this thread");
        }

        // Create the ApplicationActivationManager COM object
        let manager: IApplicationActivationManager = CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_ALL)
            .map_err(|e| AutomationError::PlatformError(format!("Failed to create ApplicationActivationManager: {}", e)))?;

        // Set options (e.g., NoSplashScreen)
        let options = ACTIVATEOPTIONS(ActivateOptions::None as i32);

        manager.ActivateApplication(
            &HSTRING::from(&app_user_model_id),
            &HSTRING::from(""), // no arguments
            options,
        ).map_err(|e| AutomationError::PlatformError(format!("Failed to launch UWP app: {}", e)))?
    };

    if pid > 0 {
        get_application_pid(engine, pid as i32, &display_name)
    } else {
        Err(Error::new(
            HRESULT(0x80004005u32 as i32),
            "Failed to launch the application"
        ).into())
    }
}

// Gets UWP app information using Get-StartApps
fn get_uwp_app_info_from_startapps(uwp_app_name: &str) -> Result<(String, String), AutomationError> {
    let command = format!(
        r#"Get-StartApps | Where-Object {{ $_.AppID -match '^[\w\.]+_[\w]+![\w\.]+$' }} | Select-Object Name, AppID | ConvertTo-Json"#
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "hidden", "-Command", &command])
        .output()
        .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AutomationError::PlatformError(format!(
            "Failed to get UWP apps list: {}",
            error_msg
        )));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let apps: Vec<Value> = serde_json::from_str(&output_str).map_err(|e| {
        AutomationError::PlatformError(format!("Failed to parse UWP apps list: {}", e))
    })?;

    // Search for matching app by name or AppID
    let search_term = uwp_app_name.to_lowercase();
    let matching_app = apps.iter().find(|app| {
        let name = app.get("Name").and_then(|n| n.as_str()).unwrap_or("").to_lowercase();
        let app_id = app.get("AppID").and_then(|id| id.as_str()).unwrap_or("").to_lowercase();
        name.contains(&search_term) || app_id.contains(&search_term)
    });

    match matching_app {
        Some(app) => {
            let display_name = app.get("Name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| AutomationError::PlatformError("Failed to get app name".to_string()))?;
            let app_id = app.get("AppID")
                .and_then(|id| id.as_str())
                .ok_or_else(|| AutomationError::PlatformError("Failed to get app ID".to_string()))?;
            Ok((app_id.to_string(), display_name.to_string()))
        }
        None => Err(AutomationError::PlatformError(format!(
            "No UWP app found matching '{}' in Get-StartApps list",
            uwp_app_name
        )))
    }
}

// Gets UWP package information by name
fn get_uwp_package_info(uwp_app_name: &str) -> Result<Value, AutomationError> {
    let command = format!(
        r#"Get-AppxPackage | Where-Object {{ -not $_.IsFramework }} | Where-Object {{ $_.Name -like "*{}*" }} | ConvertTo-Json -Depth 1"#,
        uwp_app_name
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "hidden", "-Command", &command])
        .output()
        .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AutomationError::PlatformError(format!(
            "Failed to find UWP package: {}",
            error_msg
        )));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let json_str = output_str.trim();
    if json_str.is_empty() {
        return Err(AutomationError::PlatformError(format!(
            "No UWP package found matching '{}'. The package may not be installed or the name is incorrect.",
            uwp_app_name
        )));
    }

    let packages: Value = serde_json::from_str(json_str).map_err(|e| {
        AutomationError::PlatformError(format!("Failed to parse package info: {}", e))
    })?;

    match packages {
        Value::Array(arr) => {
            if arr.is_empty() {
                return Err(AutomationError::PlatformError(format!(
                    "No UWP package found matching '{}'. The package may not be installed or the name is incorrect.",
                    uwp_app_name
                )));
            }
            if arr.len() > 1 {
                let package_names = arr
                    .iter()
                    .map(|p| p.get("Name").unwrap_or(&Value::Null).to_string())
                    .collect::<Vec<String>>()
                    .join("\n    • ");

                return Err(AutomationError::PlatformError(format!(
                    "Multiple UWP packages found matching '{}'.\nPlease be more specific. Found:\n    • {}",
                    uwp_app_name, package_names
                )));
            }
            Ok(arr[0].clone())
        }
        Value::Object(obj) => Ok(Value::Object(obj)),
        Value::Null => Err(AutomationError::PlatformError(format!(
            "No UWP package found matching '{}'. The package may not be installed or the name is incorrect.",
            uwp_app_name
        ))),
        _ => Err(AutomationError::PlatformError(
            "Invalid package info format".to_string(),
        )),
    }
}

// Gets the application ID for a UWP package
fn get_uwp_info(package_full_name: &str) -> Result<(String, String), AutomationError> {
    let command = format!(
        r#"$manifest = Get-AppxPackageManifest -Package "{}"
$manifest.Package.Applications.Application.Id
$manifest.Package.Properties.DisplayName"#,
        package_full_name
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "hidden", "-Command", &command])
        .output()
        .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AutomationError::PlatformError(format!(
            "Failed to get UWP app info: {}",
            error_msg
        )));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut lines = output_str.lines();

    let app_id = lines.next().ok_or_else(|| {
        AutomationError::PlatformError("Failed to get application ID".to_string())
    })?;

    let display_name = lines.next().ok_or_else(|| {
        AutomationError::PlatformError("Failed to get display name".to_string())
    })?;

    Ok((app_id.to_string(), display_name.to_string()))
}

// Gets package information from a UWP package value
fn get_package_info(package: &Value) -> Result<(String, String), AutomationError> {
    let package_full_name = package
        .get("PackageFullName")
        .and_then(|n| n.as_str())
        .ok_or_else(|| {
            AutomationError::PlatformError("Failed to get package full name".to_string())
        })?;

    let package_family_name = package
        .get("PackageFamilyName")
        .and_then(|n| n.as_str())
        .ok_or_else(|| {
            AutomationError::PlatformError("Failed to get package family name".to_string())
        })?;

    Ok((package_full_name.to_string(), package_family_name.to_string()))
}


// Helper function to get application by PID with fallback to child process and name
fn get_application_pid(engine: &WindowsEngine, pid: i32, app_name: &str) -> Result<UIElement, AutomationError> {
    unsafe {
        // Check if the process with this PID exists
        let mut pid_exists = false;
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(handle) => handle,
            Err(_) => {
                debug!("Failed to create process snapshot for PID existence check, falling back to name");
                let app = engine.get_application_by_name(app_name)?;
                app.activate_window()?;
                return Ok(app);
            }
        };
        if !snapshot.is_invalid() {
            let _guard = HandleGuard(snapshot);
            let mut process_entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            if Process32FirstW(snapshot, &mut process_entry).is_ok() {
                loop {
                    if process_entry.th32ProcessID == pid as u32 {
                        pid_exists = true;
                        break;
                    }
                    if Process32NextW(snapshot, &mut process_entry).is_err() {
                        break;
                    }
                }
            }
        }

        if pid_exists {
            match engine.get_application_by_pid(pid, Some(DEFAULT_FIND_TIMEOUT)) {
                Ok(app) => {
                    app.activate_window()?;
                    return Ok(app);
                }
                Err(_) => {
                    debug!("Failed to get application by PID, will try child PID logic");
                }
            }
        }

        // If PID does not exist or get_application_by_pid failed, try to find a child process with this as parent
        let parent_pid = pid as u32;
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(handle) => handle,
            Err(_) => {
                debug!("Failed to create process snapshot for child search, falling back to name");
                let app = engine.get_application_by_name(app_name)?;
                app.activate_window()?;
                return Ok(app);
            }
        };
        if snapshot.is_invalid() {
            debug!("Invalid snapshot handle for child search, falling back to name");
            let app = engine.get_application_by_name(app_name)?;
            app.activate_window()?;
            return Ok(app);
        }
        let _guard = HandleGuard(snapshot);
        let mut process_entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut found_child_pid: Option<u32> = None;
        if Process32FirstW(snapshot, &mut process_entry).is_ok() {
            loop {
                if process_entry.th32ParentProcessID == parent_pid {
                    found_child_pid = Some(process_entry.th32ProcessID);
                    break;
                }
                if Process32NextW(snapshot, &mut process_entry).is_err() {
                    break;
                }
            }
        }
        if let Some(child_pid) = found_child_pid {
            match engine.get_application_by_pid(child_pid as i32, Some(DEFAULT_FIND_TIMEOUT)) {
                Ok(app) => {
                    app.activate_window()?;
                    return Ok(app);
                }
                Err(_) => {
                    debug!("Failed to get application by child PID, falling back to name");
                }
            }
        }
        // If all else fails, try to find the application by name
        debug!("Failed to get application by PID and child PID, trying by name: {}", app_name);
        let app = engine.get_application_by_name(app_name)?;
        app.activate_window()?;
        Ok(app)
    }
}


// Launches a regular (non-UWP) Windows application
fn launch_regular_application(engine: &WindowsEngine, app_name: &str) -> Result<UIElement, AutomationError> {
    unsafe {
        // Convert app_name to wide string
        let mut app_name_wide: Vec<u16> = app_name.encode_utf16().chain(std::iter::once(0)).collect();
        
        // Prepare process startup info
        let mut startup_info = STARTUPINFOW::default();
        startup_info.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        
        // Prepare process info
        let mut process_info = PROCESS_INFORMATION::default();
        
        // Create the process
        let result = CreateProcessW(
            None, // Application name (null means use command line)
            Some(PWSTR::from_raw(app_name_wide.as_mut_ptr())), // Command line
            None, // Process security attributes
            None, // Thread security attributes
            false, // Inherit handles
            CREATE_NEW_CONSOLE, // Creation flags
            None, // Environment
            None, // Current directory
            &startup_info,
            &mut process_info,
        );

        if result.is_err() {
            return Err(AutomationError::PlatformError(
                format!("Failed to launch application '{}'", app_name)
            ));
        }

        // Close thread handle as we don't need it
        let _ = CloseHandle(process_info.hThread);
        
        // Store process handle in a guard to ensure it's closed
        let _process_handle = HandleGuard(process_info.hProcess);
        
        // Get the PID
        let pid = process_info.dwProcessId as i32;

        // Wait a bit for the application to start
        std::thread::sleep(std::time::Duration::from_millis(200));

        get_application_pid(engine, pid, app_name)
    }
}

// make easier to pass roles
fn map_generic_role_to_win_roles(role: &str) -> ControlType {
    match role.to_lowercase().as_str() {
        "pane" | "app" | "application" => ControlType::Pane,
        "window" | "dialog" => ControlType::Window,
        "button" => ControlType::Button,
        "checkbox" => ControlType::CheckBox,
        "menu" => ControlType::Menu,
        "menuitem" => ControlType::MenuItem,
        "text" => ControlType::Text,
        "tree" => ControlType::Tree,
        "treeitem" => ControlType::TreeItem,
        "data" | "dataitem" => ControlType::DataItem,
        "datagrid" => ControlType::DataGrid,
        "url" | "urlfield" => ControlType::Edit,
        "list" => ControlType::List,
        "image" => ControlType::Image,
        "title" => ControlType::TitleBar,
        "listitem" => ControlType::ListItem,
        "combobox" => ControlType::ComboBox,
        "tab" => ControlType::Tab,
        "tabitem" => ControlType::TabItem,
        "toolbar" => ControlType::ToolBar,
        "appbar" => ControlType::AppBar,
        "calendar" => ControlType::Calendar,
        "edit" => ControlType::Edit,
        "hyperlink" => ControlType::Hyperlink,
        "progressbar" => ControlType::ProgressBar,
        "radiobutton" => ControlType::RadioButton,
        "scrollbar" => ControlType::ScrollBar,
        "slider" => ControlType::Slider,
        "spinner" => ControlType::Spinner,
        "statusbar" => ControlType::StatusBar,
        "tooltip" => ControlType::ToolTip,
        "custom" => ControlType::Custom,
        "group" => ControlType::Group,
        "thumb" => ControlType::Thumb,
        "document" => ControlType::Document,
        "splitbutton" => ControlType::SplitButton,
        "header" => ControlType::Header,
        "headeritem" => ControlType::HeaderItem,
        "table" => ControlType::Table,
        "titlebar" => ControlType::TitleBar,
        "separator" => ControlType::Separator,
        "semanticzoom" => ControlType::SemanticZoom,
        _ => ControlType::Custom, // keep as it is for unknown roles
    }
}

fn get_pid_by_name(name: &str) -> Option<i32> {
    unsafe {
        // Create a snapshot of all processes
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(handle) => handle,
            Err(_) => return None,
        };
        
        if snapshot.is_invalid() {
            return None;
        }
        
        // Ensure we close the handle when done
        let _guard = HandleGuard(snapshot);
        
        let mut process_entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        
        // Get the first process
        if Process32FirstW(snapshot, &mut process_entry).is_err() {
            return None;
        }
        
        let search_name_lower = name.to_lowercase();
        
        // Iterate through processes to find one with matching name
        loop {
            // Convert the process name from wide string to String
            let name_slice = &process_entry.szExeFile;
            let name_len = name_slice.iter().position(|&c| c == 0).unwrap_or(name_slice.len());
            let process_name = String::from_utf16_lossy(&name_slice[..name_len]);
            
            // Remove .exe extension if present for comparison
            let clean_name = process_name
                .strip_suffix(".exe")
                .or_else(|| process_name.strip_suffix(".EXE"))
                .unwrap_or(&process_name);
            
            // Check if this process name contains our search term
            if clean_name.to_lowercase().contains(&search_name_lower) {
                // For processes with windows, we should check if they have a main window
                // This is a simple heuristic - in a more complete implementation,
                // you might want to use EnumWindows to check for actual windows
                return Some(process_entry.th32ProcessID as i32);
            }
            
            // Get the next process
            if Process32NextW(snapshot, &mut process_entry).is_err() {
                break;
            }
        }
        
        None
    }
}

// Add this function before the WindowsUIElement implementation
fn generate_element_id(element: &uiautomation::UIElement) -> Result<usize, AutomationError> {
    // Get stable properties that are less likely to change
    // Try cached versions first, fallback to live versions
    let control_type = element.get_cached_control_type()
        .or_else(|_| element.get_control_type())
        .map_err(|e| AutomationError::PlatformError(format!("Failed to get control type: {}", e)))?;
    let name = element.get_cached_name()
        .or_else(|_| element.get_name())
        .map_err(|e| AutomationError::PlatformError(format!("Failed to get name: {}", e)))?;
    let automation_id = element.get_cached_automation_id()
        .or_else(|_| element.get_automation_id())
        .map_err(|e| AutomationError::PlatformError(format!("Failed to get automation ID: {}", e)))?;
    let class_name = element.get_cached_classname()
        .or_else(|_| element.get_classname())
        .map_err(|e| AutomationError::PlatformError(format!("Failed to get classname: {}", e)))?;
    let bounds = element.get_cached_bounding_rectangle()
        .or_else(|_| element.get_bounding_rectangle())
        .map_err(|e| AutomationError::PlatformError(format!("Failed to get bounding rectangle: {}", e)))?;
    // runtime_id is fundamental and less likely to have a distinct cached vs. live fetch issue here
    // It's usually retrieved when the element handle is obtained.
    let runtime_id = element.get_runtime_id()
        .map_err(|e| AutomationError::PlatformError(format!("Failed to get runtime ID: {}", e)))?;
    let help_text = element.get_cached_help_text()
        .or_else(|_| element.get_help_text())
        .map_err(|e| AutomationError::PlatformError(format!("Failed to get help text: {}", e)))?;

    // Create a stable string representation
    let id_string = format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{:?}:{}",
        control_type,
        name,
        automation_id,
        class_name,
        bounds.get_left(),
        bounds.get_top(),
        bounds.get_width(),
        bounds.get_height(),
        runtime_id,
        help_text
    );
    
    // Generate a hash from the stable string
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    id_string.hash(&mut hasher);
    let hash = hasher.finish() as usize;
    
    Ok(hash)
}

// Add this function after the generate_element_id function and before the tests module
/// Converts a raw uiautomation::UIElement to a terminator UIElement
pub fn convert_uiautomation_element_to_terminator(element: uiautomation::UIElement) -> UIElement {
    let arc_element = ThreadSafeWinUIElement(Arc::new(element));
    UIElement::new(Box::new(WindowsUIElement {
        element: arc_element,
    }))
}

    
#[cfg(test)]
mod tests {
    use super::*;
    use std::process;
    use std::time::Instant;

    #[test]
    fn test_get_process_name_by_pid_current_process() {
        // Test with the current process PID
        let current_pid = process::id() as i32;
        let result = get_process_name_by_pid(current_pid);
        
        assert!(result.is_ok(), "Should be able to get current process name");
        let process_name = result.unwrap();
        
        // The process name should be a valid non-empty string
        assert!(!process_name.is_empty(), "Process name should not be empty");
        
        // Should not contain .exe extension
        assert!(!process_name.ends_with(".exe"), "Process name should not contain .exe extension");
        assert!(!process_name.ends_with(".EXE"), "Process name should not contain .EXE extension");
        
        // Should be a reasonable process name (alphanumeric, hyphens, underscores)
        assert!(process_name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'), 
               "Process name should contain only alphanumeric characters, hyphens, and underscores: {}", process_name);
        
        println!("Current process name: {}", process_name);
    }

    // Performance measurement utilities
    struct PerformanceMetrics {
        duration: std::time::Duration,
        elements_processed: usize,
        max_depth: usize,
        memory_usage_bytes: usize,
    }

    impl PerformanceMetrics {
        fn new() -> Self {
            Self {
                duration: std::time::Duration::ZERO,
                elements_processed: 0,
                max_depth: 0,
                memory_usage_bytes: 0,
            }
        }

        fn elements_per_second(&self) -> f64 {
            if self.duration.as_secs_f64() > 0.0 {
                self.elements_processed as f64 / self.duration.as_secs_f64()
            } else {
                0.0
            }
        }

        fn is_performance_acceptable(&self) -> bool {
            // Define acceptable performance criteria:
            // - Should complete within 5 seconds
            // - Should process at least 100 elements per second
            // - Should not use more than 100MB of memory
            self.duration.as_secs() < 5
                && self.elements_per_second() >= 100.0
                && self.memory_usage_bytes < 100 * 1024 * 1024
        }
    }

    fn measure_tree_building_performance(element: &UIElement, max_elements: Option<usize>) -> Result<PerformanceMetrics, AutomationError> {
        let start_time = Instant::now();
        let mut metrics = PerformanceMetrics::new();
        
        // Get memory usage before
        let memory_before = get_memory_usage();
        
        // Build tree with element counting
        let result = build_ui_tree_with_metrics(element, 0, max_elements.unwrap_or(10000), &mut metrics);
        
        // Calculate final metrics
        metrics.duration = start_time.elapsed();
        metrics.memory_usage_bytes = get_memory_usage().saturating_sub(memory_before);
        
        match result {
            Ok(_) => Ok(metrics),
            Err(e) => {
                println!("Tree building failed: {}", e);
                Ok(metrics) // Return partial metrics even on failure
            }
        }
    }

    fn build_ui_tree_with_metrics(
        element: &UIElement,
        current_depth: usize,
        max_elements: usize,
        metrics: &mut PerformanceMetrics,
    ) -> Result<crate::UINode, AutomationError> {
        // Update metrics
        metrics.elements_processed += 1;
        metrics.max_depth = metrics.max_depth.max(current_depth);

        // Safety limits
        if metrics.elements_processed >= max_elements {
            return Err(AutomationError::PlatformError("Element limit reached".to_string()));
        }

        if current_depth > 50 {
            return Err(AutomationError::PlatformError("Maximum depth reached".to_string()));
        }

        let attributes = element.attributes();
        let mut children_nodes = Vec::new();

        // Get children with timeout
        let children_start = Instant::now();
        let children_result = element.children();
        
        // If getting children takes too long, skip them
        if children_start.elapsed() > std::time::Duration::from_millis(500) {
            println!("Skipping children for element due to timeout");
            return Ok(crate::UINode {
                attributes,
                children: children_nodes,
            });
        }

        match children_result {
            Ok(children_elements) => {
                for child_element in children_elements.iter().take(20) { // Limit children processed
                    match build_ui_tree_with_metrics(child_element, current_depth + 1, max_elements, metrics) {
                        Ok(child_node) => children_nodes.push(child_node),
                        Err(e) => {
                            println!("Skipping child due to error: {}", e);
                            break; // Stop processing children on error
                        }
                    }
                    
                    // Check if we're taking too long
                    if metrics.duration.as_secs() > 5 {
                        println!("Breaking early due to time limit");
                        break;
                    }
                }
            }
            Err(e) => {
                println!("Failed to get children: {}", e);
            }
        }

        Ok(crate::UINode {
            attributes,
            children: children_nodes,
        })
    }

    fn get_memory_usage() -> usize {
        // Simple memory usage estimation - in a real implementation you might use
        // more sophisticated memory tracking
        
        // This is a simplified approach - for more accurate measurement,
        // you'd want to use platform-specific APIs or tools like `peak_alloc`
        0 // Placeholder - implement actual memory tracking if needed
    }

    #[test]
    fn test_tree_building_performance_simple_element() {
        // Test with a simple window to establish baseline performance
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping performance test");
                return;
            }
        };

        // Try to get any available window for testing
        let applications = match engine.get_applications() {
            Ok(apps) => apps,
            Err(_) => {
                println!("Cannot get applications, skipping performance test");
                return;
            }
        };

        if applications.is_empty() {
            println!("No applications available for performance testing");
            return;
        }

        let test_element = &applications[0];
        
        // Measure performance with limited elements
        let metrics = match measure_tree_building_performance(test_element, Some(100)) {
            Ok(metrics) => metrics,
            Err(e) => {
                println!("Performance measurement failed: {}", e);
                return;
            }
        };

        // Print detailed metrics
        println!("=== Tree Building Performance Metrics ===");
        println!("Duration: {:?}", metrics.duration);
        println!("Elements processed: {}", metrics.elements_processed);
        println!("Max depth: {}", metrics.max_depth);
        println!("Memory usage: {} bytes", metrics.memory_usage_bytes);
        println!("Elements per second: {:.2}", metrics.elements_per_second());
        println!("Performance acceptable: {}", metrics.is_performance_acceptable());

        // Basic assertions
        assert!(metrics.duration.as_secs() < 10, "Tree building should complete within 10 seconds");
        assert!(metrics.elements_processed > 0, "Should process at least one element");
    }

    #[test] 
    fn test_tree_building_performance_stress_test() {
        // This test validates full tree building (no limits) with caching optimization
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping stress test");
                return;
            }
        };

        // Try to find complex applications for full tree testing
        let applications = match engine.get_applications() {
            Ok(apps) => apps,
            Err(_) => {
                println!("Cannot get applications, skipping stress test");
                return;
            }
        };

        for app in applications.iter().take(2) { // Test up to 2 applications for full trees
            let app_name = app.attributes().name.unwrap_or_default();
            println!("Testing FULL tree building for application: {}", app_name);

            let start_time = Instant::now();
            
            // Measure FULL tree building with new optimized function
            let tree_result = build_full_ui_node_tree_optimized(app);
            let total_time = start_time.elapsed();
            
            match tree_result {
                Ok(tree) => {
                    let element_count = count_tree_elements(&tree);
                    let max_depth = calculate_tree_depth(&tree);
                    let elements_per_second = if total_time.as_secs_f64() > 0.0 {
                        element_count as f64 / total_time.as_secs_f64()
                    } else {
                        0.0
                    };
                    
                    println!("=== FULL Tree Building Results for {} ===", app_name);
                    println!("Total time: {:?}", total_time);
                    println!("Elements in full tree: {}", element_count);
                    println!("Maximum depth: {}", max_depth);
                    println!("Elements per second: {:.2}", elements_per_second);
                    
                    // Validate that we're getting comprehensive results
                    assert!(element_count > 0, "Should find elements in application tree");
                    assert!(max_depth > 0, "Should have some depth in application tree");
                    
                    // Performance should be reasonable (not computer-freezing)
                    assert!(total_time.as_secs() < 60, "Full tree building should complete within 60 seconds for {}", app_name);
                    
                    // Should maintain good throughput
                    if elements_per_second < 20.0 {
                        println!("WARNING: Lower than expected throughput for {}: {:.2} elements/sec", app_name, elements_per_second);
                    }
                }
                Err(e) => {
                    println!("Full tree building failed for {}: {}", app_name, e);
                }
            }
        }
    }

    // Helper function to count elements in a tree
    fn count_tree_elements(node: &crate::UINode) -> usize {
        1 + node.children.iter().map(|child| count_tree_elements(child)).sum::<usize>()
    }

    // Helper function to calculate maximum depth
    fn calculate_tree_depth(node: &crate::UINode) -> usize {
        if node.children.is_empty() {
            1
        } else {
            1 + node.children.iter().map(|child| calculate_tree_depth(child)).max().unwrap_or(0)
        }
    }

    #[test]
    fn test_tree_building_depth_limit() {
        // Test that depth limiting works correctly
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping depth limit test");
                return;
            }
        };

        let root = engine.get_root_element();
        let mut metrics = PerformanceMetrics::new();
        
        // Test with very limited depth
        let result = build_ui_tree_with_metrics(&root, 0, 50, &mut metrics);
        
        match result {
            Ok(_) => {
                println!("Depth limit test - Max depth reached: {}", metrics.max_depth);
                assert!(metrics.max_depth <= 50, "Should respect depth limit");
            }
            Err(e) => {
                println!("Depth limit test completed with controlled error: {}", e);
                // This is expected behavior when hitting limits
            }
        }
    }

    #[test]
    fn test_tree_building_element_limit() {
        // Test that element count limiting works correctly
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping element limit test");
                return;
            }
        };

        let root = engine.get_root_element();
        let mut metrics = PerformanceMetrics::new();
        
        // Test with very limited element count
        let result = build_ui_tree_with_metrics(&root, 0, 10, &mut metrics);
        
        match result {
            Ok(_) => {
                println!("Element limit test - Elements processed: {}", metrics.elements_processed);
                assert!(metrics.elements_processed <= 10, "Should respect element limit");
            }
            Err(e) => {
                println!("Element limit test completed with controlled error: {}", e);
                assert!(metrics.elements_processed <= 10, "Should not exceed element limit even on error");
            }
        }
    }

    #[test]
    fn test_get_process_name_by_pid_invalid_pid() {
        // Test with an invalid PID (very high number unlikely to exist)
        let invalid_pid = 999999;
        let result = get_process_name_by_pid(invalid_pid);
        
        assert!(result.is_err(), "Should fail for invalid PID");
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Process with PID"), "Error should mention the PID");
    }

    #[test]
    fn test_get_process_name_by_pid_system_process() {
        // Test with a known system process (PID 4 is usually System on Windows)
        let system_pid = 4;
        let result = get_process_name_by_pid(system_pid);
        
        // This might succeed or fail depending on permissions, but shouldn't panic
        match result {
            Ok(name) => {
                assert!(!name.is_empty(), "Process name should not be empty");
                println!("System process name: {}", name);
            }
            Err(e) => {
                println!("Expected: Could not access system process: {}", e);
            }
        }
    }

    #[test]
    fn test_get_pid_by_name_current_process() {
        // First get the current process name
        let current_pid = process::id() as i32;
        let current_name_result = get_process_name_by_pid(current_pid);
        
        if let Ok(current_name) = current_name_result {
            // Now try to find a PID by that name
            let found_pid = get_pid_by_name(&current_name);
            
            // Should find at least one process with this name
            assert!(found_pid.is_some(), "Should find a PID for current process name: {}", current_name);
            
            let found_pid = found_pid.unwrap();
            assert!(found_pid > 0, "PID should be positive");
            
            // The found PID might not be exactly the same as current PID if there are multiple
            // processes with the same name, but it should be valid
            println!("Current PID: {}, Found PID for '{}': {}", current_pid, current_name, found_pid);
        }
    }

    #[test]
    fn test_get_pid_by_name_nonexistent_process() {
        // Test with a process name that definitely doesn't exist
        let nonexistent_name = "definitely_nonexistent_process_12345";
        let result = get_pid_by_name(nonexistent_name);
        
        assert!(result.is_none(), "Should return None for nonexistent process");
    }

    #[test]
    fn test_get_pid_by_name_partial_match() {
        // Test partial matching - try to find any process containing "win"
        // This should match Windows system processes like "winlogon", "wininit", etc.
        let partial_name = "win";
        let result = get_pid_by_name(partial_name);
        
        // This might or might not find something, but shouldn't panic
        match result {
            Some(pid) => {
                assert!(pid > 0, "Found PID should be positive");
                println!("Found process with 'win' in name: PID {}", pid);
                
                // Verify we can get the name back
                if let Ok(name) = get_process_name_by_pid(pid) {
                    assert!(name.to_lowercase().contains("win"), 
                           "Found process name '{}' should contain 'win'", name);
                }
            }
            None => {
                println!("No process found containing 'win' - this is possible but unusual on Windows");
            }
        }
    }

    #[test]
    fn test_handle_guard_cleanup() {
        // Test that HandleGuard properly cleans up handles
        use windows::Win32::System::Diagnostics::ToolHelp::CreateToolhelp32Snapshot;
        
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).unwrap();
            assert!(!snapshot.is_invalid(), "Snapshot should be valid");
            
            // Create and immediately drop the guard
            {
                let _guard = HandleGuard(snapshot);
                // Guard should be alive here
            }
            // Guard should be dropped and handle closed here
            
            // We can't easily test that the handle was actually closed without
            // potentially causing issues, but the test verifies the code compiles
            // and the guard can be created/dropped without panicking
        }
    }

    #[test]
    fn test_process_name_extension_stripping() {
        // Test the extension stripping logic by checking current process
        let current_pid = process::id() as i32;
        let result = get_process_name_by_pid(current_pid);
        
        if let Ok(process_name) = result {
            // Verify no .exe extension
            assert!(!process_name.ends_with(".exe"), "Process name should not end with .exe");
            assert!(!process_name.ends_with(".EXE"), "Process name should not end with .EXE");
            
            // Verify it's not empty after stripping
            assert!(!process_name.is_empty(), "Process name should not be empty after extension stripping");
        }
    }

    #[test]
    fn test_multiple_process_enumeration() {
        // Test that we can enumerate multiple processes without issues
        let current_pid = process::id() as i32;
        
        // Try to get names for a range of PIDs around the current one
        let mut successful_lookups = 0;
        let mut failed_lookups = 0;
        
        for offset in -10..=10 {
            let test_pid = current_pid + offset;
            if test_pid > 0 {
                match get_process_name_by_pid(test_pid) {
                    Ok(name) => {
                        successful_lookups += 1;
                        assert!(!name.is_empty(), "Process name should not be empty");
                        println!("PID {}: {}", test_pid, name);
                    }
                    Err(_) => {
                        failed_lookups += 1;
                    }
                }
            }
        }
        
        // We should have at least found the current process
        assert!(successful_lookups >= 1, "Should find at least the current process");
        println!("Successful lookups: {}, Failed lookups: {}", successful_lookups, failed_lookups);
    }

    #[cfg(test)]
    mod integration_tests {
        use super::*;

        #[test]
        fn test_round_trip_pid_name_lookup() {
            // Test the round trip: PID -> Name -> PID
            let original_pid = process::id() as i32;
            
            // Get the process name
            let process_name = match get_process_name_by_pid(original_pid) {
                Ok(name) => name,
                Err(_) => {
                    println!("Could not get current process name, skipping round-trip test");
                    return;
                }
            };
            
            // Find a PID by that name
            let found_pid = get_pid_by_name(&process_name);
            
            assert!(found_pid.is_some(), "Should find a PID for process name: {}", process_name);
            let found_pid = found_pid.unwrap();
            
            // Verify the found PID corresponds to the same process name
            let verified_name = get_process_name_by_pid(found_pid).unwrap();
            assert_eq!(process_name, verified_name, 
                      "Round-trip should yield the same process name");
            
            println!("Round-trip successful: {} -> {} -> {}", 
                    original_pid, process_name, found_pid);
        }
    }

    #[test]
    fn test_open_regular_application() {
        // Test opening a regular Windows application (Notepad)
        let engine = WindowsEngine::new(false, false).unwrap();
        
        // Test successful case
        let result = engine.open_application("notepad.exe");
        assert!(result.is_ok(), "Should be able to open Notepad");
        let app = result.unwrap();
        assert_eq!(app.role(), "Window", "Opened application should be a window");
        
        // Test error case with non-existent application
        let result = engine.open_application("nonexistent_app.exe");
        assert!(result.is_err(), "Should fail to open non-existent application");
        if let Err(AutomationError::PlatformError(_)) = result {
            // Expected error type
        } else {
            panic!("Expected PlatformError for non-existent application");
        }
    }

    #[test]
    fn test_open_uwp_application() {
        // Test opening a UWP application (Calculator)
        let engine = WindowsEngine::new(false, false).unwrap();
        
        // Test successful case
        let result = engine.open_application("uwp:Microsoft.WindowsCalculator");
        assert!(result.is_ok(), "Should be able to open Calculator");
        let app = result.unwrap();
        assert_eq!(app.role(), "Window", "Opened UWP application should be a window");
        
        // Test error case with non-existent UWP app
        let result = engine.open_application("uwp:NonexistentUWPApp");
        assert!(result.is_err(), "Should fail to open non-existent UWP application");
        if let Err(AutomationError::PlatformError(_)) = result {
            // Expected error type
        } else {
            panic!("Expected PlatformError for non-existent UWP application");
        }
    }

    #[test]
    fn test_open_application_edge_cases() {
        let engine = WindowsEngine::new(false, false).unwrap();
        
        // Test empty application name
        let result = engine.open_application("");
        assert!(result.is_err(), "Should fail with empty application name");
        
        // Test application name with only whitespace
        let result = engine.open_application("   ");
        assert!(result.is_err(), "Should fail with whitespace-only application name");
        
        // Test application name with invalid characters
        let result = engine.open_application("app<with>invalid:chars");
        assert!(result.is_err(), "Should fail with invalid characters in application name");
    }

    #[test]
    fn test_open_application_with_path() {
        let engine = WindowsEngine::new(false, false).unwrap();
        
        // Test opening application with full path
        let result = engine.open_application("C:\\Windows\\System32\\notepad.exe");
        assert!(result.is_ok(), "Should be able to open Notepad with full path");
        let app = result.unwrap();
        assert_eq!(app.role(), "Window", "Opened application should be a window");
        
        // Test opening application with relative path
        let result = engine.open_application(".\\notepad.exe");
        assert!(result.is_err(), "Should fail with relative path");
    }

    #[test]
    fn test_open_application_with_arguments() {
        let engine = WindowsEngine::new(false, false).unwrap();
        
        // Test opening application with arguments
        let result = engine.open_application("notepad.exe test.txt");
        assert!(result.is_ok(), "Should be able to open Notepad with arguments");
        let app = result.unwrap();
        assert_eq!(app.role(), "Window", "Opened application should be a window");
    }

    #[test]
    fn test_open_application_with_special_characters() {
        let engine = WindowsEngine::new(false, false).unwrap();
        
        // Test with spaces in path
        let result = engine.open_application("C:\\Program Files\\Windows NT\\Accessories\\wordpad.exe");
        assert!(result.is_ok(), "Should handle paths with spaces");
        
        // Test with quotes
        let result = engine.open_application("\"C:\\Program Files\\Windows NT\\Accessories\\wordpad.exe\"");
        assert!(result.is_ok(), "Should handle paths with quotes");
    }

    #[test]
    fn test_open_application_permissions() {
        let engine = WindowsEngine::new(false, false).unwrap();
        
        // Test opening system application (should fail without admin rights)
        let result = engine.open_application("C:\\Windows\\System32\\cmd.exe /k net user");
        assert!(result.is_err(), "Should fail to open system command with restricted permissions");
        
        // Test opening application in protected directory
        let result = engine.open_application("C:\\Windows\\System32\\drivers\\etc\\hosts");
        assert!(result.is_err(), "Should fail to open file in protected directory");
    }

    #[test]
    fn test_open_application_concurrent() {
        let engine = WindowsEngine::new(false, false).unwrap();
        
        // Test opening multiple instances of the same application
        let result1 = engine.open_application("notepad.exe");
        assert!(result1.is_ok(), "Should open first instance");
        
        let result2 = engine.open_application("notepad.exe");
        assert!(result2.is_ok(), "Should open second instance");
        
        // Verify they are different instances
        let app1 = result1.unwrap();
        let app2 = result2.unwrap();
        assert_ne!(app1.id(), app2.id(), "Should be different application instances");
    }

    #[test]
    fn test_open_application_with_environment() {
        let engine = WindowsEngine::new(false, false).unwrap();
        
        // Test opening application that requires specific environment variables
        let result = engine.open_application("powershell.exe -NoProfile -Command \"$env:TEST_VAR='test'; notepad.exe\"");
        assert!(result.is_ok(), "Should handle applications with environment variables");
    }

    // Tests for the application() method and related functionality
    #[test]
    fn test_application_method_from_root_element() {
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping application test");
                return;
            }
        };

        let root = engine.get_root_element();
        
        // Test application method on root element
        match root.application() {
            Ok(Some(app)) => {
                println!("Found application from root element");
                
                // Verify it's a valid application element
                let attrs = app.attributes();
                println!("Application role: {}", attrs.role);
                println!("Application name: {:?}", attrs.name);
                
                // Should be either Window or Pane
                assert!(attrs.role == "Window" || attrs.role == "Pane", 
                       "Application should be Window or Pane, got: {}", attrs.role);
            }
            Ok(None) => {
                println!("No application found from root element");
            }
            Err(e) => {
                println!("Error getting application from root: {}", e);
                // This might happen in some environments, so don't fail the test
            }
        }
    }

    #[test]
    fn test_application_method_from_app_element() {
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping test");
                return;
            }
        };

        let applications = match engine.get_applications() {
            Ok(apps) => apps,
            Err(_) => {
                println!("Cannot get applications, skipping test");
                return;
            }
        };

        if applications.is_empty() {
            println!("No applications available for testing");
            return;
        }

        let app_element = &applications[0];
        println!("Testing application() method on application element");
        let app_attrs = app_element.attributes();
        println!("Original element - Role: {}, Name: {:?}", app_attrs.role, app_attrs.name);

        match app_element.application() {
            Ok(Some(found_app)) => {
                let found_attrs = found_app.attributes();
                println!("Found application - Role: {}, Name: {:?}", found_attrs.role, found_attrs.name);
                
                // Should find an application element
                assert!(found_attrs.role == "Window" || found_attrs.role == "Pane",
                       "Found application should be Window or Pane, got: {}", found_attrs.role);
                
                // Should have some identifiable attributes
                assert!(found_attrs.name.is_some() || !found_attrs.properties.is_empty(),
                       "Application should have some identifying attributes");
            }
            Ok(None) => {
                println!("No application found from app element - this might be expected");
            }
            Err(e) => {
                println!("Error getting application from app element: {}", e);
                // Don't fail test as this might be expected in some cases
            }
        }
    }

    #[test]
    fn test_application_method_from_child_element() {
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping test");
                return;
            }
        };

        let applications = match engine.get_applications() {
            Ok(apps) => apps,
            Err(_) => {
                println!("Cannot get applications, skipping test");
                return;
            }
        };

        if applications.is_empty() {
            println!("No applications available for testing");
            return;
        }

        let app_element = &applications[0];
        let children = match app_element.children() {
            Ok(children) => children,
            Err(_) => {
                println!("Cannot get child elements, skipping test");
                return;
            }
        };

        if children.is_empty() {
            println!("No child elements available for testing");
            return;
        }

        let child_element = &children[0];
        println!("Testing application() method on child element");
        let child_attrs = child_element.attributes();
        println!("Child element - Role: {}, Name: {:?}", child_attrs.role, child_attrs.name);

        match child_element.application() {
            Ok(Some(found_app)) => {
                let found_attrs = found_app.attributes();
                println!("Found application from child - Role: {}, Name: {:?}", found_attrs.role, found_attrs.name);
                
                // Should find an application element
                assert!(found_attrs.role == "Window" || found_attrs.role == "Pane",
                       "Found application should be Window or Pane, got: {}", found_attrs.role);
            }
            Ok(None) => {
                println!("No application found from child element");
            }
            Err(e) => {
                println!("Error getting application from child element: {}", e);
                // This is more likely to be a real error for child elements
            }
        }
    }

    #[test]
    fn test_window_method_from_various_elements() {
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping window test");
                return;
            }
        };

        // Test window method on root element
        let root = engine.get_root_element();
        println!("Testing window() method on root element");
        
        match root.window() {
            Ok(Some(window)) => {
                let window_attrs = window.attributes();
                println!("Found window from root - Role: {}, Name: {:?}", 
                        window_attrs.role, window_attrs.name);
                
                // Window should have Window control type
                assert_eq!(window_attrs.role, "Window",
                         "Window element should have Window role, got: {}", window_attrs.role);
            }
            Ok(None) => {
                println!("No window found from root element");
            }
            Err(e) => {
                println!("Error getting window from root element: {}", e);
            }
        }

        // Test window method on application elements if available
        if let Ok(applications) = engine.get_applications() {
            for (index, app) in applications.iter().take(2).enumerate() {
                println!("Testing window() method on application element {}", index + 1);
                
                match app.window() {
                    Ok(Some(window)) => {
                        let window_attrs = window.attributes();
                        println!("Found window from app {} - Role: {}, Name: {:?}", 
                                index + 1, window_attrs.role, window_attrs.name);
                        
                        // Window should have Window control type
                        assert_eq!(window_attrs.role, "Window",
                                 "Window element should have Window role, got: {}", window_attrs.role);
                    }
                    Ok(None) => {
                        println!("No window found from application element {}", index + 1);
                    }
                    Err(e) => {
                        println!("Error getting window from application element {}: {}", index + 1, e);
                    }
                }
            }
        }
    }

    #[test]
    fn test_application_vs_window_comparison() {
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping comparison test");
                return;
            }
        };

        let applications = match engine.get_applications() {
            Ok(apps) => apps,
            Err(_) => {
                println!("Cannot get applications, skipping comparison test");
                return;
            }
        };

        if applications.is_empty() {
            println!("No applications available for comparison test");
            return;
        }

        let app_element = &applications[0];
        let children = match app_element.children() {
            Ok(children) => children,
            Err(_) => {
                println!("Cannot get child elements, skipping comparison test");
                return;
            }
        };

        if children.is_empty() {
            println!("No child elements available for comparison test");
            return;
        }

        let child_element = &children[0];
        println!("Comparing application() vs window() results");

        let application_result = child_element.application();
        let window_result = child_element.window();

        match (application_result, window_result) {
            (Ok(Some(app)), Ok(Some(window))) => {
                let app_attrs = app.attributes();
                let window_attrs = window.attributes();
                
                println!("Application - Role: {}, Name: {:?}", app_attrs.role, app_attrs.name);
                println!("Window - Role: {}, Name: {:?}", window_attrs.role, window_attrs.name);
                
                // Application can be Window or Pane, Window should be Window
                assert!(app_attrs.role == "Window" || app_attrs.role == "Pane");
                assert_eq!(window_attrs.role, "Window");
                
                // If application is a Window, they might be the same
                if app_attrs.role == "Window" {
                    println!("Both application and window are Window type - they might be the same element");
                }
            }
            (Ok(app_opt), Ok(window_opt)) => {
                println!("Application result: {:?}", app_opt.is_some());
                println!("Window result: {:?}", window_opt.is_some());
            }
            (app_result, window_result) => {
                println!("Application result: {:?}", app_result.is_ok());
                println!("Window result: {:?}", window_result.is_ok());
            }
        }
    }

    #[test]
    fn test_application_method_error_handling() {
        // This test checks error handling in the application method
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping error handling test");
                return;
            }
        };

        let root = engine.get_root_element();
        
        // The application method should handle cases gracefully
        match root.application() {
            Ok(result) => {
                println!("Application method succeeded, result present: {}", result.is_some());
            }
            Err(e) => {
                println!("Application method returned error: {}", e);
                
                // Should be a PlatformError based on the implementation
                match e {
                    AutomationError::PlatformError(msg) => {
                        assert!(msg.contains("Failed to create UIAutomation") || 
                               msg.contains("Failed to get children for element"),
                               "Error message should be specific: {}", msg);
                    }
                    _ => {
                        panic!("Expected PlatformError, got: {:?}", e);
                    }
                }
            }
        }
    }

    #[test]
    fn test_application_method_performance() {
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping performance test");
                return;
            }
        };

        let applications = match engine.get_applications() {
            Ok(apps) => apps,
            Err(_) => {
                println!("Cannot get applications, skipping performance test");
                return;
            }
        };

        if applications.is_empty() {
            println!("No applications available for performance test");
            return;
        }

        let app_element = &applications[0];
        let children = match app_element.children() {
            Ok(children) => children,
            Err(_) => {
                println!("Cannot get child elements, skipping performance test");
                return;
            }
        };

        if children.is_empty() {
            println!("No child elements available for performance test");
            return;
        }

        let child_element = &children[0];
        println!("Testing application() method performance");

        let start_time = std::time::Instant::now();
        let mut successful_calls = 0;
        let mut failed_calls = 0;

        // Test multiple calls to see if performance is reasonable
        for i in 0..5 {
            match child_element.application() {
                Ok(_) => {
                    successful_calls += 1;
                    println!("Call {} succeeded", i + 1);
                }
                Err(e) => {
                    failed_calls += 1;
                    println!("Call {} failed: {}", i + 1, e);
                }
            }
        }

        let total_time = start_time.elapsed();
        let avg_time = total_time / 5;

        println!("Performance results:");
        println!("  Total time: {:?}", total_time);
        println!("  Average time per call: {:?}", avg_time);
        println!("  Successful calls: {}", successful_calls);
        println!("  Failed calls: {}", failed_calls);

        // Performance should be reasonable (less than 1 second per call)
        assert!(avg_time < Duration::from_secs(1), 
               "Average call time should be under 1 second, got: {:?}", avg_time);
    }

    #[test]
    fn test_related_navigation_methods() {
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping navigation test");
                return;
            }
        };

        let applications = match engine.get_applications() {
            Ok(apps) => apps,
            Err(_) => {
                println!("Cannot get applications, skipping navigation test");
                return;
            }
        };

        if applications.is_empty() {
            println!("No applications available for navigation test");
            return;
        }

        let app_element = &applications[0];
        let children = match app_element.children() {
            Ok(children) => children,
            Err(_) => {
                println!("Cannot get child elements, skipping navigation test");
                return;
            }
        };

        if children.is_empty() {
            println!("No child elements available for navigation test");
            return;
        }

        let child_element = &children[0];
        println!("Testing related navigation methods");

        // Test application()
        let app_result = child_element.application();
        println!("Application result: {:?}", app_result.is_ok());

        // Test window()
        let window_result = child_element.window();
        println!("Window result: {:?}", window_result.is_ok());

        // Test parent()
        let parent_result = child_element.parent();
        println!("Parent result: {:?}", parent_result.is_ok());

        // Test children()
        let children_result = child_element.children();
        match &children_result {
            Ok(children) => println!("Children count: {}", children.len()),
            Err(e) => println!("Children error: {}", e),
        }

        // At least one navigation method should work
        assert!(app_result.is_ok() || window_result.is_ok() || parent_result.is_ok() || children_result.is_ok(),
               "At least one navigation method should work");
    }

    #[test]
    fn test_application_method_with_focus() {
        let engine = match WindowsEngine::new(false, false) {
            Ok(engine) => engine,
            Err(_) => {
                println!("Cannot create WindowsEngine, skipping focus test");
                return;
            }
        };

        // Get focused element and test application method
        match engine.get_focused_element() {
            Ok(focused_element) => {
                println!("Testing application() on focused element");
                let focused_attrs = focused_element.attributes();
                println!("Focused element - Role: {}, Name: {:?}", 
                        focused_attrs.role, focused_attrs.name);

                match focused_element.application() {
                    Ok(Some(app)) => {
                        let app_attrs = app.attributes();
                        println!("Application from focused - Role: {}, Name: {:?}", 
                                app_attrs.role, app_attrs.name);
                        
                        assert!(app_attrs.role == "Window" || app_attrs.role == "Pane",
                               "Application should be Window or Pane");
                    }
                    Ok(None) => {
                        println!("No application found from focused element");
                    }
                    Err(e) => {
                        println!("Error getting application from focused element: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("Cannot get focused element: {}", e);
            }
        }
    }
}
