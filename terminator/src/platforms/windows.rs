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
use std::time::Duration;
use tokio::runtime::Runtime;
use tracing::debug;
use tracing::error;
use tracing::info;
use uiautomation::UIAutomation;
use uiautomation::controls::ControlType;
use uiautomation::filters::{ClassNameFilter, ControlTypeFilter, NameFilter, OrFilter};
use uiautomation::inputs::Mouse;
use uiautomation::patterns;
use uiautomation::types::{Point, TreeScope, UIProperty};
use uiautomation::variants::Variant;
use uni_ocr::{OcrEngine, OcrProvider};
use arboard::Clipboard;

// Define a default timeout duration
const DEFAULT_FIND_TIMEOUT: Duration = Duration::from_millis(5000);

// List of common browser process names (without .exe)
const KNOWN_BROWSER_PROCESS_NAMES: &[&str] = &[
    "chrome", "firefox", "msedge", "iexplore", "opera", "brave", "vivaldi", "browser", "arc"
];

// Helper function to get process name by PID using PowerShell
fn get_process_name_by_pid(pid: i32) -> Result<String, AutomationError> {
    let command = format!(
        "Get-Process -Id {} | Select-Object -ExpandProperty ProcessName",
        pid
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "hidden", "-Command", &command])
        .output()
        .map_err(|e| AutomationError::PlatformError(format!("Failed to execute PowerShell to get process name: {}", e)))?;

    if output.status.success() {
        let process_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if process_name.is_empty() {
            Err(AutomationError::PlatformError(format!(
                "Process name not found for PID {}",
                pid
            )))
        } else {
            Ok(process_name)
        }
    } else {
        let err_msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(AutomationError::PlatformError(format!(
            "PowerShell command failed to get process name for PID {}: {}",
            pid, err_msg
        )))
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

    fn get_application_by_pid(&self, pid: i32) -> Result<UIElement, AutomationError> {
        let root_ele = self.automation.0.get_root_element().unwrap();
        let condition = self
            .automation
            .0
            .create_property_condition(UIProperty::ProcessId, Variant::from(pid), None)
            .unwrap();
        let ele = root_ele
            .find_first(TreeScope::Subtree, &condition)
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))?;
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
            // Step 1: Find the UWP package and get basic info
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

            let packages = match packages {
                Value::Array(arr) => arr,
                Value::Object(obj) => vec![Value::Object(obj)],
                Value::Null => {
                    return Err(AutomationError::PlatformError(format!(
                        "No UWP package found matching '{}'. The package may not be installed or the name is incorrect.",
                        uwp_app_name
                    )));
                }
                _ => {
                    return Err(AutomationError::PlatformError(
                        "Invalid package info format".to_string(),
                    ));
                }
            };

            if packages.is_empty() {
                return Err(AutomationError::PlatformError(format!(
                    "No UWP package found matching '{}'. The package may not be installed or the name is incorrect.",
                    uwp_app_name
                )));
            }

            if packages.len() > 1 {
                let package_names = packages
                    .iter()
                    .map(|p| p.get("Name").unwrap_or(&Value::Null).to_string())
                    .collect::<Vec<String>>()
                    .join("\n    • ");

                return Err(AutomationError::PlatformError(format!(
                    "Multiple UWP packages found matching '{}'.\nPlease be more specific. Found:\n    • {}",
                    uwp_app_name, package_names
                )));
            }

            let package = &packages[0];
            let package_full_name = package
                .get("PackageFullName")
                .and_then(|n| n.as_str())
                .ok_or_else(|| {
                    AutomationError::PlatformError("Failed to get package full name".to_string())
                })?;

            let install_location = package
                .get("InstallLocation")
                .and_then(|n| n.as_str())
                .ok_or_else(|| {
                    AutomationError::PlatformError("Failed to get install location".to_string())
                })?;

            let package_family_name = package
                .get("PackageFamilyName")
                .and_then(|n| n.as_str())
                .ok_or_else(|| {
                    AutomationError::PlatformError("Failed to get package family name".to_string())
                })?;

            // Step 2: Get the app ID and executable name
            let command = format!(
                r#"$manifest = Get-AppxPackageManifest -Package "{}"
$manifest.Package.Applications.Application.Id
$manifest.Package.Applications.Application.Executable
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

            let executable_name = lines.next().ok_or_else(|| {
                AutomationError::PlatformError("Failed to get executable name".to_string())
            })?;

            let display_name = lines.next().ok_or_else(|| {
                AutomationError::PlatformError("Failed to get display name".to_string())
            })?;

            // Step 3: Launch the UWP app
            let command = format!(
                r#"$appsFolderPath = "shell:appsFolder\{}!{}"
explorer $appsFolderPath"#,
                package_family_name.trim(),
                app_id.trim()
            );

            let output = std::process::Command::new("powershell")
                .args(["-NoProfile", "-WindowStyle", "hidden", "-Command", &command])
                .output()
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

            if !output.status.success() {
                let error_msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
                return Err(AutomationError::PlatformError(format!(
                    "Failed to launch UWP application: {}",
                    error_msg
                )));
            }

            // Wait for the app to start
            std::thread::sleep(std::time::Duration::from_millis(2000));

            // Step 4: Get the process ID
            let command = format!(
                r#"$executablePath = "{}\{}"
$processes = Get-WmiObject Win32_Process | Where-Object {{ $_.ExecutablePath -like "$executablePath" }}
$latestProcess = $processes | Sort-Object CreationDate -Descending | Select-Object -First 1
if (-not $latestProcess) {{
    Write-Error "Failed to find process for UWP application"
    exit 1
}}
$latestProcess.ProcessId"#,
                install_location.trim(),
                executable_name.trim()
            );

            let output = match std::process::Command::new("powershell")
                .args(["-NoProfile", "-WindowStyle", "hidden", "-Command", &command])
                .output()
            {
                Ok(output) => output,
                Err(_) => {
                    // Fallback to finding by name when process ID lookup fails
                    let app = self.get_application_by_name(display_name)?;
                    app.activate_window()?;
                    return Ok(app);
                }
            };

            let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if pid_str.is_empty() {
                // Fallback to finding by name when no PID is returned
                let app = self.get_application_by_name(display_name)?;
                app.activate_window()?;
                return Ok(app);
            }

            let pid = match pid_str.parse::<i32>() {
                Ok(pid) => pid,
                Err(_) => {
                    // Fallback to finding by name when PID parsing fails
                    let app = self.get_application_by_name(display_name)?;
                    app.activate_window()?;
                    return Ok(app);
                }
            };

            // Get the application using the PID, with fallback to name
            let app = match self.get_application_by_pid(pid) {
                Ok(app) => app,
                Err(_) => {
                    // Fallback to finding by name
                    self.get_application_by_name(display_name)?
                }
            };
            app.activate_window()?;
            Ok(app)
        } else {
            // Handle regular application
            let output = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-WindowStyle",
                    "hidden",
                    "-Command",
                    &format!(
                        "Start-Process '{}' -PassThru | Select-Object -ExpandProperty Id",
                        app_name
                    ),
                ])
                .output()
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

            if !output.status.success() {
                return Err(AutomationError::PlatformError(
                    "Failed to open application".to_string(),
                ));
            }

            let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if pid_str.is_empty() {
                // Fallback to finding by name when no PID is returned
                let app = self.get_application_by_name(app_name)?;
                app.activate_window()?;
                return Ok(app);
            }

            let pid = match pid_str.parse::<i32>() {
                Ok(pid) => pid,
                Err(_) => {
                    // Fallback to finding by name when PID parsing fails
                    let app = self.get_application_by_name(app_name)?;
                    app.activate_window()?;
                    return Ok(app);
                }
            };

            // Wait a bit for the application to start
            std::thread::sleep(std::time::Duration::from_millis(200));

            // Get the application using the PID, with fallback to name
            let app = match self.get_application_by_pid(pid) {
                Ok(app) => app,
                Err(_) => {
                    // Fallback to finding by name
                    self.get_application_by_name(app_name)?
                }
            };
            app.activate_window()?;
            Ok(app)
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
                match self.get_application_by_pid(pid as i32) {
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
        self.element.0.get_control_type().unwrap().to_string()
    }

    fn attributes(&self) -> UIElementAttributes {
        let mut properties = HashMap::new();
        // there are alot of properties, including neccessary ones
        // ref: https://docs.rs/uiautomation/0.16.1/uiautomation/types/enum.UIProperty.html
        let property_list = vec![
            UIProperty::Name,
            UIProperty::HelpText,
            UIProperty::LabeledBy,
            UIProperty::ValueValue,
            UIProperty::ControlType,
            UIProperty::AutomationId,
            UIProperty::FullDescription,
            UIProperty::IsKeyboardFocusable, // Added for attributes
        ];
        for property in property_list {
            if let Ok(value) = self.element.0.get_property_value(property) {
                properties.insert(
                    format!("{:?}", property),
                    Some(serde_json::to_value(value.to_string()).unwrap_or_default()),
                );
            } else {
                properties.insert(format!("{:?}", property), None);
            }
        }
        UIElementAttributes {
            role: self.role(),
            name: self.element.0.get_name().ok(),
            label: self
                .element
                .0
                .get_labeled_by()
                .ok().map(|e| e.get_name().unwrap_or_default()),
            value: self
                .element
                .0
                .get_property_value(UIProperty::ValueValue)
                .ok()
                .and_then(|v| v.get_string().ok()),
            description: self.element.0.get_help_text().ok(),
            properties,
            is_keyboard_focusable: self.is_keyboard_focusable().ok(), // Added field
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
                info!(
                    "Failed to get cached children for text extraction ({}), falling back to non-cached TreeScope::Children search.",
                    cache_err
                );
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
            // Save current clipboard content
            let original_clipboard = Clipboard::new()
                .map_err(|e| AutomationError::PlatformError(format!("Failed to access clipboard: {}", e)))?
                .get_text()
                .map_err(|e| AutomationError::PlatformError(format!("Failed to get clipboard content: {}", e)))?;

            // Set new text to clipboard
            Clipboard::new()
                .map_err(|e| AutomationError::PlatformError(format!("Failed to access clipboard: {}", e)))?
                .set_text(text)
                .map_err(|e| AutomationError::PlatformError(format!("Failed to set clipboard content: {}", e)))?;

            // Focus the element
            self.focus()?;

            // Send Ctrl+V to paste
            self.press_key("{ctrl}v")?;

            // Restore original clipboard content
            if !original_clipboard.is_empty() {
                Clipboard::new()
                    .map_err(|e| AutomationError::PlatformError(format!("Failed to access clipboard: {}", e)))?
                    .set_text(&original_clipboard)
                    .map_err(|e| AutomationError::PlatformError(format!("Failed to restore clipboard content: {}", e)))?;
            }

            Ok(())
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

            // Check Name property 
            // TOdo: i dont think we should include the name in text
            // if let Ok(name) = element.get_property_value(UIProperty::Name) {
            //     if let Ok(name_text) = name.get_string() {
            //         if !name_text.is_empty() {
            //             debug!("found text in name property: {:?}", &name_text);
            //             texts.push(name_text);
            //         }
            //     }
            // }

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
                    info!(
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
                                            info!(
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

        /* Original implementation commented out
        // Try to get the scroll pattern first
        let scroll_pattern = self.element.0.get_pattern::<patterns::UIScrollPattern>();
        let scroll_item_pattern = self.element.0.get_pattern::<patterns::UIScrollItemPattern>();
        
        if let Ok(scroll_pattern) = scroll_pattern {
            // If we have a scroll pattern, use it
            let scroll_amount = if amount > 0.0 {
                ScrollAmount::SmallIncrement
            } else if amount < 0.0 {
                ScrollAmount::SmallDecrement
            } else {
                ScrollAmount::NoAmount
            };

            let times = amount.abs() as usize;
            for _ in 0..times {
                match direction {
                    "up" => scroll_pattern
                        .scroll(ScrollAmount::NoAmount, scroll_amount)
                        .map_err(|e| AutomationError::PlatformError(format!("Failed to scroll up: {:?}", e))),
                    "down" => scroll_pattern
                        .scroll(ScrollAmount::NoAmount, scroll_amount)
                        .map_err(|e| AutomationError::PlatformError(format!("Failed to scroll down: {:?}", e))),
                    "left" => scroll_pattern
                        .scroll(scroll_amount, ScrollAmount::NoAmount)
                        .map_err(|e| AutomationError::PlatformError(format!("Failed to scroll left: {:?}", e))),
                    "right" => scroll_pattern
                        .scroll(scroll_amount, ScrollAmount::NoAmount)
                        .map_err(|e| AutomationError::PlatformError(format!("Failed to scroll right: {:?}", e))),
                    _ => Err(AutomationError::UnsupportedOperation(
                        "Invalid scroll direction".to_string(),
                    )),
                }?;
            }
        } else if let Ok(scroll_item_pattern) = scroll_item_pattern {
            // If we have a scroll item pattern, use it
            let times = amount.abs() as usize;
            for _ in 0..times {
                match direction {
                    "up" => scroll_item_pattern
                        .scroll_into_view()
                        .map_err(|e| AutomationError::PlatformError(format!("Failed to scroll item up: {:?}", e))),
                    "down" => scroll_item_pattern
                        .scroll_into_view()
                        .map_err(|e| AutomationError::PlatformError(format!("Failed to scroll item down: {:?}", e))),
                    "left" => scroll_item_pattern
                        .scroll_into_view()
                        .map_err(|e| AutomationError::PlatformError(format!("Failed to scroll item left: {:?}", e))),
                    "right" => scroll_item_pattern
                        .scroll_into_view()
                        .map_err(|e| AutomationError::PlatformError(format!("Failed to scroll item right: {:?}", e))),
                    _ => Err(AutomationError::UnsupportedOperation(
                        "Invalid scroll direction".to_string(),
                    )),
                }?;
            }
        } else {
            // If no scroll patterns available, fall back to mouse wheel simulation
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput,
            };
            use std::thread::sleep;
            use std::time::Duration;

            // Get the element's bounds to calculate center point
            let rect = self.element.0.get_bounding_rectangle()
                .map_err(|e| AutomationError::PlatformError(format!("Failed to get element bounds: {:?}", e)))?;
            
            let center_x = rect.get_left() + rect.get_width() / 2;
            let center_y = rect.get_top() + rect.get_height() / 2;

            // Move mouse to center of element
            let mi = MOUSEINPUT {
                dx: center_x,
                dy: center_y,
                mouseData: if amount > 0.0 { 120u32 } else { 0xFFFF_FF88u32 }, // 120 for up, -120 (as unsigned) for down
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: 0,
            };
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 { mi },
            };

            let times = amount.abs() as usize;
            // Simulate wheel clicks
            for _ in 0..times {
                unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
                sleep(Duration::from_millis(50)); // Small delay between wheel events
            }
        }
        */
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
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_ABSOLUTE, MOUSEINPUT, SendInput,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        use std::thread::sleep;
        use std::time::Duration;

        fn to_absolute(x: f64, y: f64) -> (i32, i32) {
            let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
            let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
            let abs_x = ((x / screen_w as f64) * 65535.0).round() as i32;
            let abs_y = ((y / screen_h as f64) * 65535.0).round() as i32;
            (abs_x, abs_y)
        }

        fn mouse_move_abs(x: f64, y: f64) {
            let (abs_x, abs_y) = to_absolute(x, y);
            let mi = MOUSEINPUT {
                dx: abs_x,
                dy: abs_y,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                time: 0,
                dwExtraInfo: 0,
            };
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 { mi },
            };
            unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        }

        fn click_and_hold() {
            let mi = MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_LEFTDOWN,
                time: 0,
                dwExtraInfo: 0,
            };
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 { mi },
            };
            unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        }

        fn release_mouse() {
            let mi = MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_LEFTUP,
                time: 0,
                dwExtraInfo: 0,
            };
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 { mi },
            };
            unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        }

        // Move to start
        mouse_move_abs(start_x, start_y);
        sleep(Duration::from_millis(20));
        click_and_hold();
        sleep(Duration::from_millis(20));
        mouse_move_abs(end_x, end_y);
        sleep(Duration::from_millis(20));
        release_mouse();
        Ok(())
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
    // window title shouldn't be empty
    let command = format!(
        "Get-Process | Where-Object {{ $_.MainWindowTitle -ne '' -and $_.Name -like '*{}*' }} | ForEach-Object {{ $_.Id }}",
        name
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "hidden", "-Command", &command])
        .output()
        .expect("Failed to execute PowerShell script");

    if output.status.success() {
        // return only parent pid
        let pid_str = String::from_utf8_lossy(&output.stdout);
        pid_str.lines().next()?.trim().parse().ok()
    } else {
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
    
    // debug!(
    //     "Generated element ID: hash={}, control_type={}, name={}, automation_id={}, class_name={}, bounds=({},{},{},{}), help_text={}",
    //     hash,
    //     control_type,
    //     name,
    //     automation_id,
    //     class_name,
    //     bounds.get_left(),
    //     bounds.get_top(),
    //     bounds.get_width(),
    //     bounds.get_height(),
    //     help_text
    // );
    
    Ok(hash)
}
