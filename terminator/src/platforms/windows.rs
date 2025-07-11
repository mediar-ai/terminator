#![allow(clippy::arc_with_non_send_sync)]

use crate::element::UIElementImpl;
use crate::platforms::AccessibilityEngine;
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
use tracing::{debug, error, info, warn};
use uiautomation::controls::ControlType;
use uiautomation::filters::{ClassNameFilter, ControlTypeFilter, NameFilter, OrFilter};
use uiautomation::inputs::Mouse;
use uiautomation::patterns;
use uiautomation::types::{Point, TreeScope, UIProperty};
use uiautomation::variants::Variant;
use uiautomation::UIAutomation;
use uni_ocr::{OcrEngine, OcrProvider};

// windows imports
use windows::core::{Error, HRESULT, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HINSTANCE, HWND};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::HKEY;
use windows::Win32::System::Threading::GetProcessId;
use windows::Win32::UI::Shell::{
    ApplicationActivationManager, IApplicationActivationManager, ShellExecuteExW, ShellExecuteW,
    ACTIVATEOPTIONS, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

// Define a default timeout duration
const DEFAULT_FIND_TIMEOUT: Duration = Duration::from_millis(5000);

// List of common browser process names (without .exe)
const KNOWN_BROWSER_PROCESS_NAMES: &[&str] = &[
    "chrome", "firefox", "msedge", "edge", "iexplore", "opera", "brave", "vivaldi", "browser",
    "arc", "explorer",
];

// Helper function to get process name by PID using native Windows API
pub fn get_process_name_by_pid(pid: i32) -> Result<String, AutomationError> {
    unsafe {
        // Create a snapshot of all processes
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|e| {
            AutomationError::PlatformError(format!("Failed to create process snapshot: {e}"))
        })?;

        if snapshot.is_invalid() {
            return Err(AutomationError::PlatformError(
                "Invalid snapshot handle".to_string(),
            ));
        }

        // Ensure we close the handle when done
        let _guard = HandleGuard(snapshot);

        let mut process_entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        // Get the first process
        if Process32FirstW(snapshot, &mut process_entry).is_err() {
            return Err(AutomationError::PlatformError(
                "Failed to get first process".to_string(),
            ));
        }

        // Iterate through processes to find the one with matching PID
        loop {
            if process_entry.th32ProcessID == pid as u32 {
                // Convert the process name from wide string to String
                let name_slice = &process_entry.szExeFile;
                let name_len = name_slice
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(name_slice.len());
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
            "Process with PID {pid} not found"
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
        // Initialize COM in multithreaded mode for thread safety
        unsafe {
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            if hr.is_err() && hr != HRESULT(0x80010106u32 as i32) {
                // Only return error if it's not the "already initialized" case
                return Err(AutomationError::PlatformError(format!(
                    "Failed to initialize COM in multithreaded mode: {hr}"
                )));
            }
            // If we get here, either initialization succeeded or it was already initialized
            if hr == HRESULT(0x80010106u32 as i32) {
                debug!("COM already initialized in this thread");
            }
        }

        let automation = UIAutomation::new_direct()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        let arc_automation = ThreadSafeWinUIAutomation(Arc::new(automation));
        Ok(Self {
            automation: arc_automation,
            use_background_apps,
            activate_app,
        })
    }

    /// Extract browser-specific information from window titles
    pub fn extract_browser_info(title: &str) -> (bool, Vec<String>) {
        let title_lower = title.to_lowercase();
        let is_browser = KNOWN_BROWSER_PROCESS_NAMES
            .iter()
            .any(|&browser| title_lower.contains(browser));

        if is_browser {
            let mut parts = Vec::new();

            // Split by common browser title separators
            for separator in &[" - ", " — ", " | ", " • "] {
                if title.contains(separator) {
                    parts.extend(title.split(separator).map(|s| s.trim().to_string()));
                    break;
                }
            }

            // If no separators found, use the whole title
            if parts.is_empty() {
                parts.push(title.trim().to_string());
            }

            (true, parts)
        } else {
            (false, vec![title.to_string()])
        }
    }

    /// Calculate similarity score between two strings with various matching strategies
    pub fn calculate_similarity(text1: &str, text2: &str) -> f64 {
        let text1_lower = text1.to_lowercase();
        let text2_lower = text2.to_lowercase();

        // Exact match
        if text1_lower == text2_lower {
            return 1.0;
        }

        // Contains match - favor longer matches
        if text1_lower.contains(&text2_lower) || text2_lower.contains(&text1_lower) {
            let shorter = text1_lower.len().min(text2_lower.len());
            let longer = text1_lower.len().max(text2_lower.len());
            return shorter as f64 / longer as f64 * 0.9; // Slight penalty for partial match
        }

        // Word-based similarity for longer texts
        let words1: Vec<&str> = text1_lower.split_whitespace().collect();
        let words2: Vec<&str> = text2_lower.split_whitespace().collect();

        if words1.is_empty() || words2.is_empty() {
            return 0.0;
        }

        let mut common_words = 0;
        for word1 in &words1 {
            for word2 in &words2 {
                if word1 == word2 || word1.contains(word2) || word2.contains(word1) {
                    common_words += 1;
                    break;
                }
            }
        }

        // Calculate Jaccard similarity with word overlap
        let total_unique_words = words1.len() + words2.len() - common_words;
        if total_unique_words > 0 {
            common_words as f64 / total_unique_words as f64
        } else {
            0.0
        }
    }

    /// Enhanced title matching that handles browser windows and fuzzy matching
    fn find_best_title_match(
        &self,
        windows: &[(uiautomation::UIElement, String)],
        target_title: &str,
    ) -> Option<(uiautomation::UIElement, f64)> {
        let title_lower = target_title.to_lowercase();
        let mut best_match: Option<uiautomation::UIElement> = None;
        let mut best_score = 0.0f64;

        for (window, window_name) in windows {
            // Strategy 1: Direct contains match (highest priority)
            if window_name.to_lowercase().contains(&title_lower) {
                info!(
                    "Found exact title match: '{}' contains '{}'",
                    window_name, target_title
                );
                return Some((window.clone(), 1.0));
            }

            // Strategy 2: Browser-aware matching
            let (is_browser_window, window_parts) = Self::extract_browser_info(window_name);
            let (is_target_browser, target_parts) = Self::extract_browser_info(target_title);

            if is_browser_window && is_target_browser {
                let mut max_part_similarity = 0.0f64;

                for window_part in &window_parts {
                    for target_part in &target_parts {
                        let similarity = Self::calculate_similarity(window_part, target_part);
                        max_part_similarity = max_part_similarity.max(similarity);

                        debug!(
                            "Comparing '{}' vs '{}' = {:.2}",
                            window_part, target_part, similarity
                        );
                    }
                }

                if max_part_similarity > 0.6 && max_part_similarity > best_score {
                    info!(
                        "Found browser match: '{}' vs '{}' (similarity: {:.2})",
                        window_name, target_title, max_part_similarity
                    );
                    best_score = max_part_similarity;
                    best_match = Some(window.clone());
                }
            }

            // Strategy 3: General fuzzy matching as fallback
            if best_score < 0.6 {
                let similarity = Self::calculate_similarity(window_name, target_title);
                if similarity > 0.5 && similarity > best_score {
                    debug!(
                        "Potential fuzzy match: '{}' vs '{}' (similarity: {:.2})",
                        window_name, target_title, similarity
                    );
                    best_score = similarity;
                    best_match = Some(window.clone());
                }
            }
        }

        best_match.map(|window| (window, best_score))
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

        // OPTIMIZATION: Use Children scope instead of Subtree to avoid deep tree traversal
        // Most applications are direct children of the desktop
        let condition_win = self
            .automation
            .0
            .create_property_condition(
                UIProperty::ControlType,
                Variant::from(ControlType::Window as i32),
                None,
            )
            .unwrap();

        let condition_pane = self
            .automation
            .0
            .create_property_condition(
                UIProperty::ControlType,
                Variant::from(ControlType::Pane as i32),
                None,
            )
            .unwrap();

        let condition = self
            .automation
            .0
            .create_or_condition(condition_win, condition_pane)
            .unwrap();

        let elements = root
            .find_all(TreeScope::Children, &condition)
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))?;

        // OPTIMIZATION: Filter out windows with same pid to reduce processing
        let mut seen_pids = std::collections::HashSet::new();
        let filtered_elements: Vec<uiautomation::UIElement> = elements
            .into_iter()
            .filter(|ele| {
                // include windows with names, this way we'd all the opened applications
                if let Ok(pid) = ele.get_process_id() {
                    if seen_pids.insert(pid) {
                        // include only elements with unique PIDs
                        if let Ok(name) = ele.get_name() {
                            !name.is_empty()
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .collect();

        debug!("Found '{}' application windows", filtered_elements.len());

        let arc_elements: Vec<UIElement> = filtered_elements
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
            .or_else(|| name.strip_suffix(".EXE"))
            .unwrap_or(name);

        let search_name_lower = search_name.to_lowercase();
        let is_browser = KNOWN_BROWSER_PROCESS_NAMES
            .iter()
            .any(|&browser| search_name_lower.contains(browser));

        // For non-browsers, try fast PID lookup first
        if !is_browser {
            if let Some(pid) = get_pid_by_name(search_name) {
                debug!(
                    "Found process PID {} for non-browser app: {}",
                    pid, search_name
                );

                let condition = self
                    .automation
                    .0
                    .create_property_condition(UIProperty::ProcessId, Variant::from(pid), None)
                    .unwrap();
                let root_ele = self.automation.0.get_root_element().unwrap();

                // Try direct window lookup by PID
                if let Ok(ele) = root_ele.find_first(TreeScope::Children, &condition) {
                    debug!("Found application window for PID {}", pid);
                    let arc_ele = ThreadSafeWinUIElement(Arc::new(ele));
                    return Ok(UIElement::new(Box::new(WindowsUIElement {
                        element: arc_ele,
                    })));
                }
            }
        }

        // For browsers and fallback: Use window title search
        debug!("Using window title search for: {}", search_name);
        let root_ele = self.automation.0.get_root_element().unwrap();

        let matcher = self
            .automation
            .0
            .create_matcher()
            .control_type(ControlType::Window)
            .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                let window_name = e.get_name().unwrap_or_default();
                let window_name_lower = window_name.to_lowercase();

                // Enhanced browser matching logic with better detection
                let matches = match search_name_lower.as_str() {
                    "chrome" => {
                        window_name_lower.contains("chrome")
                            || window_name_lower.contains("google chrome")
                            || (window_name_lower.contains("google")
                                && window_name_lower.contains("browser"))
                    }
                    "firefox" => {
                        window_name_lower.contains("firefox")
                            || window_name_lower.contains("mozilla")
                            || window_name_lower.contains("mozilla firefox")
                    }
                    "msedge" | "edge" => {
                        // Enhanced Edge detection
                        if window_name_lower.contains("edge")
                            || window_name_lower.contains("microsoft edge")
                            || window_name_lower.contains("microsoft")
                        {
                            true
                        } else if let Ok(pid) = e.get_process_id() {
                            get_process_name_by_pid(pid as i32)
                                .map(|p| {
                                    let proc_name = p.to_lowercase();
                                    proc_name == "msedge" || proc_name == "edge"
                                })
                                .unwrap_or(false)
                        } else {
                            false
                        }
                    }
                    "brave" => {
                        window_name_lower.contains("brave")
                            || window_name_lower.contains("brave browser")
                    }
                    "opera" => {
                        window_name_lower.contains("opera")
                            || window_name_lower.contains("opera browser")
                    }
                    "vivaldi" => {
                        window_name_lower.contains("vivaldi")
                            || window_name_lower.contains("vivaldi browser")
                    }
                    "arc" => {
                        window_name_lower.contains("arc")
                            || window_name_lower.contains("arc browser")
                    }
                    _ => {
                        // For non-browsers, use more flexible matching
                        window_name_lower.contains(&search_name_lower)
                            || search_name_lower.contains(&window_name_lower)
                    }
                };
                Ok(matches)
            }))
            .from_ref(&root_ele)
            .depth(3)
            .timeout(3000);

        let ele = matcher.find_first().map_err(|e| {
            AutomationError::PlatformError(format!("No window found for application '{name}': {e}"))
        })?;

        debug!("Found window: {}", ele.get_name().unwrap_or_default());
        let arc_ele = ThreadSafeWinUIElement(Arc::new(ele));
        Ok(UIElement::new(Box::new(WindowsUIElement {
            element: arc_ele,
        })))
    }

    fn get_application_by_pid(
        &self,
        pid: i32,
        timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError> {
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
                "Application with PID {pid} not found within {timeout_ms}ms timeout: {e}"
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
                    win_control_type,
                    role,
                    name,
                    depth,
                    timeout_ms,
                    root_ele.get_name().unwrap_or_default()
                );

                let actual_depth = depth.unwrap_or(50) as u32;

                let mut matcher_builder = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .control_type(win_control_type)
                    .depth(actual_depth)
                    .timeout(timeout_ms as u64);

                if let Some(name) = name {
                    // use contains_name, its undetermined right now
                    // wheather we should use `name` or `contains_name`
                    matcher_builder = matcher_builder.contains_name(name);
                }

                let elements = matcher_builder.find_all().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "Role: '{role}' (mapped to {win_control_type:?}), Name: {name:?}, Err: {e}"
                    ))
                })?;

                debug!(
                    "found {} elements with role: {} (mapped to {:?}), name_filter: {:?}",
                    elements.len(),
                    role,
                    win_control_type,
                    name
                );

                Ok(elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect())
            }
            Selector::Id(id) => {
                debug!("Searching for element with ID: {}", id);
                // Clone id to move into the closure
                let target_id = id.strip_prefix('#').unwrap_or(id).to_string();
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .depth(depth.unwrap_or(50) as u32)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        // Use the common function to generate ID
                        match generate_element_id(e)
                            .map(|id| id.to_string().chars().take(6).collect::<String>())
                        {
                            Ok(calculated_id) => {
                                let matches = calculated_id == target_id;
                                if matches {
                                    debug!("Found matching element with ID: {}", calculated_id);
                                }
                                Ok(matches)
                            }
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
                    AutomationError::ElementNotFound(format!("ID: '{id}', Err: {e}"))
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

                Ok(collected_elements)
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
                    AutomationError::ElementNotFound(format!("Name: '{name}', Err: {e}"))
                })?;

                Ok(elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect())
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
                    AutomationError::ElementNotFound(format!("Text: '{text}', Err: {e}"))
                })?;

                Ok(elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect())
            }
            Selector::TextExact(text) => {
                // Exact, case-sensitive name match
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .filter(Box::new(NameFilter {
                        value: String::from(text),
                        casesensitive: true,
                        partial: false,
                    }))
                    .depth(depth.unwrap_or(50) as u32)
                    .timeout(timeout_ms as u64);

                let elements = matcher.find_all().map_err(|e| {
                    AutomationError::ElementNotFound(format!("TextExact: '{text}', Err: {e}"))
                })?;

                Ok(elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect())
            }
            Selector::Path(_) => Err(AutomationError::UnsupportedOperation(
                "`Path` selector not supported".to_string(),
            )),
            Selector::NativeId(automation_id) => {
                // for windows passing `UIProperty::AutomationID` as `NativeId`
                debug!(
                    "searching for elements using AutomationId: {}",
                    automation_id
                );

                let ele_id = automation_id.clone();
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .depth(depth.unwrap_or(50) as u32)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        match e.get_automation_id() {
                            Ok(id) => {
                                let matches = id == ele_id;
                                if matches {
                                    debug!(
                                        "found matching elements with AutomationID : {}",
                                        ele_id
                                    );
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
                        "AutomationId: '{automation_id}', Err: {e}"
                    ))
                })?;

                debug!(
                    "found {} elements matching AutomationID: {}",
                    elements.len(),
                    automation_id
                );
                let collected_elements: Vec<UIElement> = elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect();
                Ok(collected_elements)
            }
            Selector::Attributes(_attributes) => Err(AutomationError::UnsupportedOperation(
                "`Attributes` selector not supported".to_string(),
            )),
            Selector::Filter(_filter) => Err(AutomationError::UnsupportedOperation(
                "`Filter` selector not supported".to_string(),
            )),
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
                        let found_elements =
                            self.find_elements(selector, root_element.as_ref(), timeout, depth)?;

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
                Ok(current_roots.into_iter().flatten().collect())
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
                    AutomationError::ElementNotFound(format!("ClassName: '{classname}', Err: {e}"))
                })?;
                Ok(elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect())
            }
            Selector::Visible(visibility) => {
                let visibility = *visibility;
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .depth(depth.unwrap_or(50) as u32)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        match e.is_offscreen() {
                            Ok(is_offscreen) => Ok(is_offscreen != visibility),
                            Err(e) => {
                                debug!("failed to get visibility: {}", e);
                                Ok(false)
                            }
                        }
                    }))
                    .timeout(timeout_ms as u64);
                let elements = matcher.find_all().map_err(|e| {
                    AutomationError::ElementNotFound(format!("Visible: '{visibility}', Err: {e}"))
                })?;
                Ok(elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect())
            }
            Selector::LocalizedRole(localized_role) => {
                debug!("searching elements by localized role: {}", localized_role);
                let lr = localized_role.clone();
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .depth(depth.unwrap_or(50) as u32)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        match e.get_localized_control_type() {
                            Ok(lct) => Ok(lct == lr),
                            Err(_) => Ok(false),
                        }
                    }))
                    .depth(depth.unwrap_or(50) as u32)
                    .timeout(timeout_ms as u64);

                let elements = matcher.find_all().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "LocalizedRole: '{localized_role}', Err: {e}"
                    ))
                })?;

                Ok(elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect())
            }
            Selector::Position(_x, _y) => {
                // not implemented
                Err(AutomationError::UnsupportedOperation(
                    "`Position` selector not supported".to_string(),
                ))
            }
            Selector::Invalid(reason) => Err(AutomationError::InvalidSelector(reason.clone())),
            Selector::Has(inner_selector) => {
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .depth(depth.unwrap_or(50) as u32)
                    .timeout(timeout_ms as u64);

                let elements = matcher.find_all().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "Error traversing elements for Has selector: {e}"
                    ))
                })?;

                let mut matched = Vec::new();
                for ele in elements {
                    let ui_el = UIElement::new(Box::new(WindowsUIElement {
                        element: ThreadSafeWinUIElement(Arc::new(ele.clone())),
                    }));
                    if let Ok(desc) = self.find_elements(
                        inner_selector,
                        Some(&ui_el),
                        Some(Duration::from_millis(500)),
                        Some(depth.unwrap_or(50)),
                    ) {
                        if !desc.is_empty() {
                            matched.push(ui_el);
                        }
                    }
                }
                if matched.is_empty() {
                    return Err(AutomationError::ElementNotFound(
                        "No elements found matching has selector".into(),
                    ));
                }
                Ok(matched)
            }
            Selector::DataTestId(value) => {
                let target = value.clone();
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .depth(depth.unwrap_or(50) as u32)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        // Fetch HelpText or Name or all properties
                        let attrs = match convert_uiautomation_element_to_terminator(e.clone())
                            .attributes()
                            .properties
                            .get("data-testid")
                        {
                            Some(Some(v)) => v.as_str().unwrap_or("") == target,
                            _ => false,
                        };
                        Ok(attrs)
                    }))
                    .timeout(timeout_ms as u64);
                let elements = matcher.find_all().map_err(|e| {
                    AutomationError::ElementNotFound(format!("data-testid: '{value}', Err: {e}"))
                })?;
                Ok(elements
                    .into_iter()
                    .map(|ele| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(ele)),
                        }))
                    })
                    .collect())
            }
            Selector::State { name, value } => {
                let state_name = name.clone();
                let desired = *value;
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .depth(depth.unwrap_or(50) as u32)
                    .filter_fn(Box::new(
                        move |e: &uiautomation::UIElement| match state_name.as_str() {
                            "focused" => e.has_keyboard_focus().map(|v| v == desired).or(Ok(false)),
                            "checked" => {
                                if let Ok(toggled) = e.get_toggle_state() {
                                    Ok((toggled != 0) == desired)
                                } else {
                                    Ok(false)
                                }
                            }
                            "expanded" => {
                                if let Ok(exp_state) = e.get_expand_collapse_state() {
                                    Ok((exp_state == uiautomation::ExpandCollapseState::Expanded)
                                        == desired)
                                } else {
                                    Ok(false)
                                }
                            }
                            "pressed" => {
                                if let Ok(toggled) = e.get_toggle_state() {
                                    Ok((toggled != 0) == desired)
                                } else {
                                    Ok(false)
                                }
                            }
                            "readonly" => match e.get_is_read_only() {
                                Ok(is_ro) => Ok(is_ro == desired),
                                Err(_) => Ok(false),
                            },
                            "required" => match e.get_is_required_for_form() {
                                Ok(is_req) => Ok(is_req == desired),
                                Err(_) => Ok(false),
                            },
                            _ => Ok(false),
                        },
                    ))
                    .timeout(timeout_ms as u64);
                let elements = matcher.find_all().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "state:{}={}, Err: {}",
                        state_name, desired, e
                    ))
                })?;
                return Ok(elements
                    .into_iter()
                    .map(|el| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(el)),
                        }))
                    })
                    .collect());
            }
        }
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
                    win_control_type,
                    role,
                    name,
                    timeout_ms,
                    root_ele.get_name().unwrap_or_default()
                );

                let mut matcher_builder = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .control_type(win_control_type)
                    .depth(50) // Default depth for find_element
                    .timeout(timeout_ms as u64);

                if let Some(name) = name {
                    // use contains_name, its undetermined right now
                    // wheather we should use `name` or `contains_name`
                    matcher_builder = matcher_builder.filter(Box::new(NameFilter {
                        value: name.clone(),
                        casesensitive: false,
                        partial: true,
                    }));
                }

                let element = matcher_builder.find_first().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "Role: '{role}' (mapped to {win_control_type:?}), Name: {name:?}, Root: {root:?}, Err: {e}"
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
                let target_id = id.strip_prefix('#').unwrap_or(id).to_string();
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .depth(50)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        // Use the common function to generate ID
                        match generate_element_id(e)
                            .map(|id| id.to_string().chars().take(6).collect::<String>())
                        {
                            Ok(calculated_id) => {
                                let matches = calculated_id == target_id;
                                if matches {
                                    debug!("Found matching element with ID: {}", calculated_id);
                                }
                                Ok(matches)
                            }
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
                    AutomationError::ElementNotFound(format!("ID: '{id}', Err: {e}"))
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
                    AutomationError::ElementNotFound(format!("Name: '{name}', Err: {e}"))
                })?;

                let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
                Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_ele,
                })))
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
                        "Text: '{text}', Root: {root:?}, Err: {e}"
                    ))
                })?;

                let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
                Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_ele,
                })))
            }
            Selector::TextExact(text) => {
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .filter(Box::new(NameFilter {
                        value: String::from(text),
                        casesensitive: true,
                        partial: false,
                    }))
                    .depth(50)
                    .timeout(timeout_ms as u64);

                let element = matcher.find_first().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "TextExact: '{text}', Root: {root:?}, Err: {e}"
                    ))
                })?;

                let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
                Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_ele,
                })))
            }
            Selector::Path(_) => Err(AutomationError::UnsupportedOperation(
                "`Path` selector not supported".to_string(),
            )),
            Selector::NativeId(automation_id) => {
                // for windows passing `UIProperty::AutomationID` as `NativeId`
                debug!(
                    "searching for element using AutomationId: {}",
                    automation_id
                );

                let ele_id = automation_id.clone();
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .depth(50) // Add depth limit
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
                        "AutomationId: '{automation_id}', Err: {e}"
                    ))
                })?;

                let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
                Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_ele,
                })))
            }
            Selector::Attributes(_attributes) => Err(AutomationError::UnsupportedOperation(
                "`Attributes` selector not supported".to_string(),
            )),
            Selector::Filter(_filter) => Err(AutomationError::UnsupportedOperation(
                "`Filter` selector not supported".to_string(),
            )),
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
                current_element.ok_or_else(|| {
                    AutomationError::ElementNotFound(
                        "Element not found after traversing chain".to_string(),
                    )
                })
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
                    AutomationError::ElementNotFound(format!("ClassName: '{classname}', Err: {e}"))
                })?;
                let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
                Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_ele,
                })))
            }
            Selector::Visible(visibility) => {
                let visibility = *visibility;
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .depth(50)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        match e.is_offscreen() {
                            Ok(is_offscreen) => Ok(is_offscreen != visibility),
                            Err(e) => {
                                debug!("failed to get visibility: {}", e);
                                Ok(false)
                            }
                        }
                    }))
                    .timeout(timeout_ms as u64);
                let element = matcher.find_first().map_err(|e| {
                    AutomationError::ElementNotFound(format!("Visible: '{visibility}', Err: {e}"))
                })?;
                Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: ThreadSafeWinUIElement(Arc::new(element)),
                })))
            }
            Selector::LocalizedRole(localized_role) => {
                debug!("searching element by localized role: {}", localized_role);
                let lr = localized_role.clone();
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        match e.get_localized_control_type() {
                            Ok(lct) => Ok(lct == lr),
                            Err(_) => Ok(false),
                        }
                    }))
                    .depth(50)
                    .timeout(timeout_ms as u64);
                let element = matcher.find_first().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "LocalizedRole: '{localized_role}', Err: {e}"
                    ))
                })?;
                let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
                Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: arc_ele,
                })))
            }
            Selector::Position(x, y) => {
                debug!("searching element at position: ({}, {})", x, y);
                let point = uiautomation::types::Point::new(*x, *y);
                let element = self.automation.0.element_from_point(point).map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "No element found at position ({x}, {y}): {e}"
                    ))
                })?;
                Ok(convert_uiautomation_element_to_terminator(element))
            }
            Selector::Invalid(reason) => Err(AutomationError::InvalidSelector(reason.clone())),
            Selector::Has(inner_selector) => {
                let elements = self.find_elements(
                    &Selector::Has(inner_selector.clone()),
                    root,
                    timeout,
                    Some(50),
                )?;
                elements.into_iter().next().ok_or_else(|| {
                    AutomationError::ElementNotFound("Element not found for has selector".into())
                })
            }
            Selector::DataTestId(value) => {
                let target = value.clone();
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .depth(50)
                    .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                        let attrs = match convert_uiautomation_element_to_terminator(e.clone())
                            .attributes()
                            .properties
                            .get("data-testid")
                        {
                            Some(Some(v)) => v.as_str().unwrap_or("") == target,
                            _ => false,
                        };
                        Ok(attrs)
                    }))
                    .timeout(timeout_ms as u64);
                let element = matcher.find_first().map_err(|e| {
                    AutomationError::ElementNotFound(format!("data-testid: '{value}', Err: {e}"))
                })?;
                Ok(UIElement::new(Box::new(WindowsUIElement {
                    element: ThreadSafeWinUIElement(Arc::new(element)),
                })))
            }
            Selector::State { name, value } => {
                let state_name = name.clone();
                let desired = *value;
                let matcher = self
                    .automation
                    .0
                    .create_matcher()
                    .from_ref(root_ele)
                    .depth(depth.unwrap_or(50) as u32)
                    .filter_fn(Box::new(
                        move |e: &uiautomation::UIElement| match state_name.as_str() {
                            "focused" => e.has_keyboard_focus().map(|v| v == desired).or(Ok(false)),
                            "checked" => {
                                if let Ok(toggled) = e.get_toggle_state() {
                                    Ok((toggled != 0) == desired)
                                } else {
                                    Ok(false)
                                }
                            }
                            "expanded" => {
                                if let Ok(exp_state) = e.get_expand_collapse_state() {
                                    Ok((exp_state == uiautomation::ExpandCollapseState::Expanded)
                                        == desired)
                                } else {
                                    Ok(false)
                                }
                            }
                            "pressed" => {
                                if let Ok(toggled) = e.get_toggle_state() {
                                    Ok((toggled != 0) == desired)
                                } else {
                                    Ok(false)
                                }
                            }
                            "readonly" => match e.get_is_read_only() {
                                Ok(is_ro) => Ok(is_ro == desired),
                                Err(_) => Ok(false),
                            },
                            "required" => match e.get_is_required_for_form() {
                                Ok(is_req) => Ok(is_req == desired),
                                Err(_) => Ok(false),
                            },
                            _ => Ok(false),
                        },
                    ))
                    .timeout(timeout_ms as u64);
                let elements = matcher.find_all().map_err(|e| {
                    AutomationError::ElementNotFound(format!(
                        "state:{}={}, Err: {}",
                        state_name, desired, e
                    ))
                })?;
                return Ok(elements
                    .into_iter()
                    .map(|el| {
                        UIElement::new(Box::new(WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::new(el)),
                        }))
                    })
                    .collect());
            }
        }
    }

    fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError> {
        info!("Opening application on Windows: {}", app_name);

        // Handle modern ms-settings apps
        if app_name.starts_with("ms-settings:") {
            info!("Launching ms-settings URI: {}", app_name);
            unsafe {
                let app_name_hstring = HSTRING::from(app_name);
                let verb_hstring = HSTRING::from("open");
                let result = ShellExecuteW(
                    None,
                    PCWSTR(verb_hstring.as_ptr()),
                    PCWSTR(app_name_hstring.as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                );
                // A value > 32 indicates success for ShellExecuteW
                if result.0 as isize <= 32 {
                    return Err(AutomationError::PlatformError(format!(
                        "Failed to open ms-settings URI: {}. Error code: {:?}",
                        app_name, result.0
                    )));
                }
            }
            // After launching, wait a bit for the app to initialize.
            std::thread::sleep(Duration::from_secs(2));
            // The window name for settings is just "Settings"
            return self.get_application_by_name("Settings");
        }

        // Try to get app info from StartApps first
        if let Ok((app_id, display_name)) = get_app_info_from_startapps(app_name) {
            return launch_app(self, &app_id, &display_name);
        }

        // If it's not a start menu app, assume it's a legacy executable
        warn!(
            "Could not find '{}' in StartApps, attempting to launch as executable.",
            app_name
        );
        launch_legacy_app(self, app_name)
    }

    fn open_url(
        &self,
        url: &str,
        browser: Option<crate::Browser>,
    ) -> Result<UIElement, AutomationError> {
        info!("Opening URL on Windows: {} (browser: {:?})", url, browser);

        let (browser_exe, browser_search_name) = match browser.as_ref() {
            Some(crate::Browser::Chrome) => (Some("chrome.exe"), "chrome"),
            Some(crate::Browser::Firefox) => (Some("firefox.exe"), "firefox"),
            Some(crate::Browser::Edge) => (Some("msedge.exe"), "msedge"),
            Some(crate::Browser::Brave) => (Some("brave.exe"), "brave"),
            Some(crate::Browser::Opera) => (Some("opera.exe"), "opera"),
            Some(crate::Browser::Vivaldi) => (Some("vivaldi.exe"), "vivaldi"),
            Some(crate::Browser::Arc) => (Some("Arc.exe"), "Arc"),
            Some(crate::Browser::Custom(path)) => {
                let path_str: &str = path;
                (Some(path_str), path_str.trim_end_matches(".exe"))
            }
            Some(crate::Browser::Default) | None => (None, ""),
        };

        let url_hstring = HSTRING::from(url);
        let verb_hstring = HSTRING::from("open");
        let verb_pcwstr = PCWSTR(verb_hstring.as_ptr());

        let hinstance = if let Some(exe_name) = browser_exe {
            // Open with a specific browser
            let exe_hstring = HSTRING::from(exe_name);
            unsafe {
                ShellExecuteW(
                    None,
                    verb_pcwstr,
                    PCWSTR(exe_hstring.as_ptr()),
                    PCWSTR(url_hstring.as_ptr()),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                )
            }
        } else {
            // Open with default browser
            unsafe {
                ShellExecuteW(
                    None,
                    verb_pcwstr,
                    PCWSTR(url_hstring.as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                )
            }
        };

        // HINSTANCE returned by ShellExecuteW is not a real HRESULT, but a value > 32 on success.
        if hinstance.0 as i32 <= 32 {
            return Err(AutomationError::PlatformError(format!(
                "Failed to open URL. ShellExecuteW returned error code: {:?}",
                hinstance.0 as i32
            )));
        }

        // Enhanced polling for browser window with better reliability
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(10000); // Increased to 10 seconds
        let initial_poll_interval = std::time::Duration::from_millis(500); // Start with longer interval
        let fast_poll_interval = std::time::Duration::from_millis(100); // Faster polling after initial delay

        // Give browser more time to start up initially
        std::thread::sleep(std::time::Duration::from_millis(1000));

        if browser_search_name.is_empty() {
            // For default browser, we assume the foreground window is the browser that just opened.
            info!("No specific browser requested, getting the current focused application.");
            // This is a synchronous block inside an async-compatible function.
            // We need to get the current application. Since `get_current_application` is async,
            // we will replicate its logic here in a sync way.
            let focused_element_raw = self.automation.0.get_focused_element().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get focused element: {e}"))
            })?;

            let pid = focused_element_raw.get_process_id().map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to get PID for focused element: {e}"
                ))
            })?;

            self.get_application_by_pid(pid as i32, Some(Duration::from_millis(5000)))
        } else {
            // For specific browser, poll with more patience and better error handling
            info!("Polling for {} browser to appear", browser_search_name);

            loop {
                if start_time.elapsed() > timeout {
                    // Before giving up, try a broader search
                    for &fallback_name in KNOWN_BROWSER_PROCESS_NAMES {
                        if fallback_name.contains(browser_search_name)
                            || browser_search_name.contains(fallback_name)
                        {
                            if let Ok(app) = self.get_application_by_name(fallback_name) {
                                info!("Found browser using fallback name: {}", fallback_name);
                                return Ok(app);
                            }
                        }
                    }

                    return Err(AutomationError::ElementNotFound(format!(
                        "Timeout waiting for {} browser to open. Available browsers: {:?}",
                        browser_search_name,
                        KNOWN_BROWSER_PROCESS_NAMES
                            .iter()
                            .filter_map(|&name| self
                                .get_application_by_name(name)
                                .ok()
                                .map(|_| name))
                            .collect::<Vec<_>>()
                    )));
                }

                // Try to find the browser window with better error handling
                match self.get_application_by_name(browser_search_name) {
                    Ok(app) => {
                        info!("Found {} browser window, returning.", browser_search_name);
                        return Ok(app);
                    }
                    Err(e) => {
                        debug!(
                            "{} browser not found yet: {}, continuing poll",
                            browser_search_name, e
                        );
                    }
                }

                // Use adaptive polling
                let poll_interval = if start_time.elapsed() < std::time::Duration::from_millis(2000)
                {
                    initial_poll_interval
                } else {
                    fast_poll_interval
                };

                std::thread::sleep(poll_interval);
            }
        }
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
                "Failed to open file '{file_path}' using Invoke-Item. Error: {stderr}"
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
        let monitors = xcap::Monitor::all()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitors: {e}")))?;
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
                        "Error checking monitor primary status: {e}"
                    )));
                }
            }
        }
        let primary_monitor = primary_monitor.ok_or_else(|| {
            AutomationError::PlatformError("Could not find primary monitor".to_string())
        })?;

        let image = primary_monitor.capture_image().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to capture screen: {e}"))
        })?;

        Ok(ScreenshotResult {
            image_data: image.to_vec(),
            width: image.width(),
            height: image.height(),
            monitor: None,
        })
    }

    async fn get_active_monitor_name(&self) -> Result<String, AutomationError> {
        // Get all windows
        let windows = xcap::Window::all()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get windows: {e}")))?;

        // Find the focused window
        let focused_window = windows
            .iter()
            .find(|w| w.is_focused().unwrap_or(false))
            .ok_or_else(|| {
                AutomationError::ElementNotFound("No focused window found".to_string())
            })?;

        // Get the monitor name for the focused window
        let monitor = focused_window.current_monitor().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get current monitor: {e}"))
        })?;

        let monitor_name = monitor.name().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor name: {e}"))
        })?;

        Ok(monitor_name)
    }

    async fn capture_monitor_by_name(
        &self,
        name: &str,
    ) -> Result<ScreenshotResult, AutomationError> {
        let monitors = xcap::Monitor::all()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitors: {e}")))?;
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
                        "Error getting monitor name: {e}"
                    )));
                }
            }
        }
        let target_monitor = target_monitor.ok_or_else(|| {
            AutomationError::ElementNotFound(format!("Monitor '{name}' not found"))
        })?;

        let image = target_monitor.capture_image().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to capture monitor '{name}': {e}"))
        })?;

        Ok(ScreenshotResult {
            image_data: image.to_vec(),
            width: image.width(),
            height: image.height(),
            monitor: None,
        })
    }

    // ============== NEW MONITOR ABSTRACTIONS ==============

    async fn list_monitors(&self) -> Result<Vec<crate::Monitor>, AutomationError> {
        let monitors = xcap::Monitor::all()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitors: {e}")))?;

        let mut result = Vec::new();
        for (index, monitor) in monitors.iter().enumerate() {
            let name = monitor.name().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor name: {e}"))
            })?;

            let is_primary = monitor.is_primary().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to check primary status: {e}"))
            })?;

            let width = monitor.width().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor width: {e}"))
            })?;

            let height = monitor.height().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor height: {e}"))
            })?;

            let x = monitor.x().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor x position: {e}"))
            })?;

            let y = monitor.y().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor y position: {e}"))
            })?;

            let scale_factor = monitor.scale_factor().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor scale factor: {e}"))
            })? as f64;

            result.push(crate::Monitor {
                id: format!("monitor_{index}"),
                name,
                is_primary,
                width,
                height,
                x,
                y,
                scale_factor,
            });
        }

        Ok(result)
    }

    async fn get_primary_monitor(&self) -> Result<crate::Monitor, AutomationError> {
        let monitors = self.list_monitors().await?;
        monitors
            .into_iter()
            .find(|m| m.is_primary)
            .ok_or_else(|| AutomationError::PlatformError("No primary monitor found".to_string()))
    }

    async fn get_active_monitor(&self) -> Result<crate::Monitor, AutomationError> {
        // Get all windows
        let windows = xcap::Window::all()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get windows: {e}")))?;

        // Find the focused window
        let focused_window = windows
            .iter()
            .find(|w| w.is_focused().unwrap_or(false))
            .ok_or_else(|| {
                AutomationError::ElementNotFound("No focused window found".to_string())
            })?;

        // Get the monitor for the focused window
        let xcap_monitor = focused_window.current_monitor().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get current monitor: {e}"))
        })?;

        // Convert to our Monitor struct
        let name = xcap_monitor.name().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor name: {e}"))
        })?;

        let is_primary = xcap_monitor.is_primary().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to check primary status: {e}"))
        })?;

        // Find the monitor index for ID generation
        let monitors = xcap::Monitor::all()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitors: {e}")))?;

        let monitor_index = monitors
            .iter()
            .position(|m| m.name().map(|n| n == name).unwrap_or(false))
            .unwrap_or(0);

        let width = xcap_monitor.width().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor width: {e}"))
        })?;

        let height = xcap_monitor.height().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor height: {e}"))
        })?;

        let x = xcap_monitor.x().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor x position: {e}"))
        })?;

        let y = xcap_monitor.y().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor y position: {e}"))
        })?;

        let scale_factor = xcap_monitor.scale_factor().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor scale factor: {e}"))
        })? as f64;

        Ok(crate::Monitor {
            id: format!("monitor_{monitor_index}"),
            name,
            is_primary,
            width,
            height,
            x,
            y,
            scale_factor,
        })
    }

    async fn get_monitor_by_id(&self, id: &str) -> Result<crate::Monitor, AutomationError> {
        let monitors = self.list_monitors().await?;
        monitors.into_iter().find(|m| m.id == id).ok_or_else(|| {
            AutomationError::ElementNotFound(format!("Monitor with ID '{id}' not found"))
        })
    }

    async fn get_monitor_by_name(&self, name: &str) -> Result<crate::Monitor, AutomationError> {
        let monitors = self.list_monitors().await?;
        monitors
            .into_iter()
            .find(|m| m.name == name)
            .ok_or_else(|| AutomationError::ElementNotFound(format!("Monitor '{name}' not found")))
    }

    async fn capture_monitor_by_id(
        &self,
        id: &str,
    ) -> Result<crate::ScreenshotResult, AutomationError> {
        let monitor = self.get_monitor_by_id(id).await?;

        // Find the xcap monitor by name
        let monitors = xcap::Monitor::all()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitors: {e}")))?;

        let xcap_monitor = monitors
            .into_iter()
            .find(|m| m.name().map(|n| n == monitor.name).unwrap_or(false))
            .ok_or_else(|| {
                AutomationError::ElementNotFound(format!("Monitor '{}' not found", monitor.name))
            })?;

        let image = xcap_monitor.capture_image().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to capture monitor '{}': {}",
                monitor.name, e
            ))
        })?;

        Ok(ScreenshotResult {
            image_data: image.to_vec(),
            width: image.width(),
            height: image.height(),
            monitor: Some(monitor),
        })
    }

    // ============== END NEW MONITOR ABSTRACTIONS ==============

    async fn ocr_image_path(&self, image_path: &str) -> Result<String, AutomationError> {
        // Create a Tokio runtime to run the async OCR operation
        let rt = Runtime::new().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to create Tokio runtime: {e}"))
        })?;

        // Run the async code block on the runtime
        rt.block_on(async {
            let engine = OcrEngine::new(OcrProvider::Auto).map_err(|e| {
                AutomationError::PlatformError(format!("Failed to create OCR engine: {e}"))
            })?;

            let (text, _language, _confidence) = engine // Destructure the tuple
                .recognize_file(image_path)
                .await
                .map_err(|e| {
                    AutomationError::PlatformError(format!("OCR recognition failed: {e}"))
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
            AutomationError::PlatformError(format!("Failed to create OCR engine: {e}"))
        })?;

        let (text, _language, _confidence) = engine
            .recognize_image(&dynamic_image) // Use recognize_image
            .await // << Directly await here
            .map_err(|e| AutomationError::PlatformError(format!("OCR recognition failed: {e}")))?;

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
                AutomationError::PlatformError(format!("Failed to get root element: {e}"))
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
            AutomationError::PlatformError(format!("Failed to find top-level windows: {e}"))
        })?;

        // TODO: focus part does not work (at least in browser firefox)
        // If find_first succeeds, 'window' is the UIElement. Now try to focus it.
        window.set_focus().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to set focus on window/tab: {e}"))
        })?; // Map focus error

        Ok(()) // If focus succeeds, return Ok
    }

    async fn get_current_browser_window(&self) -> Result<UIElement, AutomationError> {
        info!("Attempting to get the current focused browser window.");
        let focused_element_raw = self.automation.0.get_focused_element().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get focused element: {e}"))
        })?;

        let pid = focused_element_raw.get_process_id().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to get process ID for focused element: {e}"
            ))
        })?;

        let process_name_raw = get_process_name_by_pid(pid as i32)?;
        let process_name = process_name_raw.to_lowercase(); // Compare lowercase

        info!(
            "Focused element belongs to process: {} (PID: {})",
            process_name, pid
        );

        if KNOWN_BROWSER_PROCESS_NAMES
            .iter()
            .any(|&browser_name| process_name.contains(browser_name))
        {
            // First try to get the focused element's parent chain to find a tab
            let mut current_element = focused_element_raw.clone();
            let mut found_tab = false;

            // Walk up the parent chain looking for a TabItem
            for _ in 0..10 {
                // Limit depth to prevent infinite loops
                if let Ok(control_type) = current_element.get_control_type() {
                    debug!(
                        "get_current_browser_window, control_type: {:?}",
                        control_type
                    );
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
                        error!(
                            "Failed to get application window by PID {} for browser {}: {}. Falling back to focused element.",
                            pid, process_name, e
                        );
                        // Fallback to returning the originally focused element
                        let arc_focused_element =
                            ThreadSafeWinUIElement(Arc::new(focused_element_raw));
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
                "Failed to set focus on application window '{app_name}': {e}"
            ))
        })
    }

    async fn get_current_window(&self) -> Result<UIElement, AutomationError> {
        info!("Attempting to get the current focused window.");
        let focused_element_raw = self.automation.0.get_focused_element().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get focused element: {e}"))
        })?;

        let mut current_element_arc = Arc::new(focused_element_raw);

        for _ in 0..20 {
            // Max depth to prevent infinite loops
            match current_element_arc.get_control_type() {
                Ok(control_type) => {
                    if control_type == ControlType::Window || control_type == ControlType::Pane {
                        let window_ui_element = WindowsUIElement {
                            element: ThreadSafeWinUIElement(Arc::clone(&current_element_arc)),
                        };
                        return Ok(UIElement::new(Box::new(window_ui_element)));
                    }
                }
                Err(e) => {
                    return Err(AutomationError::PlatformError(format!(
                        "Failed to get control type during window search: {e}"
                    )));
                }
            }

            match current_element_arc.get_cached_parent() {
                Ok(parent_uia_element) => {
                    // Check if parent is same as current (e.g. desktop root's parent is itself)
                    let current_runtime_id = current_element_arc.get_runtime_id().map_err(|e| {
                        AutomationError::PlatformError(format!(
                            "Failed to get runtime_id for current element: {e}"
                        ))
                    })?;
                    let parent_runtime_id = parent_uia_element.get_runtime_id().map_err(|e| {
                        AutomationError::PlatformError(format!(
                            "Failed to get runtime_id for parent element: {e}"
                        ))
                    })?;

                    if parent_runtime_id == current_runtime_id {
                        debug!(
                            "Parent element has same runtime ID as current, stopping window search."
                        );
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
        let focused_element_raw = self.automation.0.get_focused_element().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get focused element: {e}"))
        })?;

        let pid = focused_element_raw.get_process_id().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get PID for focused element: {e}"))
        })?;

        self.get_application_by_pid(pid as i32, Some(DEFAULT_FIND_TIMEOUT))
    }

    fn get_window_tree(
        &self,
        pid: u32,
        title: Option<&str>,
        config: crate::platforms::TreeBuildConfig,
    ) -> Result<crate::UINode, AutomationError> {
        info!(
            "Getting window tree for PID: {} and title: {:?} with config: {:?}",
            pid, title, config
        );
        let root_ele_os = self.automation.0.get_root_element().map_err(|e| {
            error!("Failed to get root element: {}", e);
            AutomationError::PlatformError(format!("Failed to get root element: {e}"))
        })?;

        // Find all windows for the given process ID
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
            AutomationError::ElementNotFound(format!("Failed to find windows: {e}"))
        })?;

        info!(
            "Found {} total windows, filtering by PID: {}",
            windows.len(),
            pid
        );

        // Filter windows by process ID first
        let mut pid_matching_windows = Vec::new();
        let mut window_debug_info = Vec::new(); // For debugging

        for window in windows {
            match window.get_process_id() {
                Ok(window_pid) => {
                    let window_name = window.get_name().unwrap_or_else(|_| "Unknown".to_string());
                    window_debug_info.push(format!("PID: {window_pid}, Name: {window_name}"));

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
                "No windows found for process ID {pid}. Available windows: {window_debug_info:?}"
            )));
        }

        info!(
            "Found {} windows for PID: {}",
            pid_matching_windows.len(),
            pid
        );

        // Enhanced title matching logic for PID-based search
        let selected_window = if let Some(title) = title {
            info!(
                "Filtering {} windows by title: '{}'",
                pid_matching_windows.len(),
                title
            );

            // Use the enhanced title matching helper
            match self.find_best_title_match(&pid_matching_windows, title) {
                Some((window, score)) => {
                    if score < 1.0 {
                        info!(
                            "Using best match with similarity {:.2} for PID {}: '{}'",
                            score,
                            pid,
                            window.get_name().unwrap_or_default()
                        );
                    }
                    window
                }
                None => {
                    let window_names: Vec<&String> =
                        pid_matching_windows.iter().map(|(_, name)| name).collect();
                    warn!(
                        "No good title match found for '{}' in PID {}, falling back to first window. Available: {:?}",
                        title, pid, window_names
                    );
                    pid_matching_windows[0].0.clone()
                }
            }
        } else {
            info!(
                "No title filter provided, using first window with PID {}",
                pid
            );
            pid_matching_windows[0].0.clone()
        };

        let selected_window_name = selected_window
            .get_name()
            .unwrap_or_else(|_| "Unknown".to_string());
        info!(
            "Selected window: '{}' for PID: {} (title filter: {:?})",
            selected_window_name, pid, title
        );

        // Wrap the raw OS element into our UIElement
        let window_element_wrapper = UIElement::new(Box::new(WindowsUIElement {
            element: ThreadSafeWinUIElement(Arc::new(selected_window)),
        }));

        // Build the UI tree with configurable performance optimizations
        info!("Building UI tree with config: {:?}", config);

        // Use configured tree building approach
        let mut context = TreeBuildingContext {
            config: TreeBuildingConfig {
                timeout_per_operation_ms: config.timeout_per_operation_ms.unwrap_or(50),
                yield_every_n_elements: config.yield_every_n_elements.unwrap_or(50),
                batch_size: config.batch_size.unwrap_or(50),
            },
            property_mode: config.property_mode.clone(),
            elements_processed: 0,
            max_depth_reached: 0,
            cache_hits: 0,
            fallback_calls: 0,
            errors_encountered: 0,
        };

        let result = build_ui_node_tree_configurable(&window_element_wrapper, 0, &mut context)?;

        info!(
            "Tree building completed for PID: {}. Stats: elements={}, depth={}, cache_hits={}, fallbacks={}, errors={}",
            pid,
            context.elements_processed,
            context.max_depth_reached,
            context.cache_hits,
            context.fallback_calls,
            context.errors_encountered
        );

        Ok(result)
    }

    fn press_key(&self, key: &str) -> Result<(), AutomationError> {
        let focused_element = self.get_focused_element()?;
        focused_element.press_key(key)
    }

    fn zoom_in(&self, level: u32) -> Result<(), AutomationError> {
        for _ in 0..level {
            self.press_key("{Ctrl}=")?;
        }
        Ok(())
    }

    fn zoom_out(&self, level: u32) -> Result<(), AutomationError> {
        for _ in 0..level {
            self.press_key("{Ctrl}-")?;
        }
        Ok(())
    }

    fn set_zoom(&self, percentage: u32) -> Result<(), AutomationError> {
        // Fallback approach using keyboard shortcuts. This works for most browsers and many applications.
        // NOTE: This method is imprecise because browser zoom levels are not always linear (e.g., 90%, 100%, 110%, 125%).
        // It avoids using Ctrl+0 to reset zoom, as that can trigger unwanted website-specific shortcuts.
        // Instead, it zooms out fully to a known minimum state and then zooms in to the target level.

        const ZOOM_STEP: u32 = 10; // Assumed average step for zoom changes.
        const MIN_ZOOM: u32 = 25; // Assumed minimum zoom level for most browsers.
        const MAX_ZOOM_OUT_STEPS: u32 = 50; // A high number of steps to ensure we reach the minimum zoom.

        // Zoom out completely to reach a known state (minimum zoom).
        self.zoom_out(MAX_ZOOM_OUT_STEPS)?;

        // A small delay to allow the UI to process the zoom changes.
        std::thread::sleep(std::time::Duration::from_millis(100));

        if percentage <= MIN_ZOOM {
            // The target is at or below the assumed minimum, so we're done.
            return Ok(());
        }

        // From the minimum zoom, calculate how many steps to zoom in.
        // We add half of ZOOM_STEP for rounding.
        let steps_to_zoom_in = (percentage.saturating_sub(MIN_ZOOM) + ZOOM_STEP / 2) / ZOOM_STEP;

        if steps_to_zoom_in > 0 {
            self.zoom_in(steps_to_zoom_in)?;
        }

        Ok(())
    }

    /// Enable downcasting to concrete engine types
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Streamlined configuration focused on performance, not limits
struct TreeBuildingConfig {
    timeout_per_operation_ms: u64,
    yield_every_n_elements: usize,
    batch_size: usize,
}

// Context to track tree building progress (no limits)
struct TreeBuildingContext {
    config: TreeBuildingConfig,
    property_mode: crate::platforms::PropertyLoadingMode,
    elements_processed: usize,
    max_depth_reached: usize,
    cache_hits: usize,
    fallback_calls: usize,
    errors_encountered: usize,
}

impl TreeBuildingContext {
    fn should_yield(&self) -> bool {
        self.elements_processed % self.config.yield_every_n_elements == 0
            && self.elements_processed > 0
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

// Safe element children access
fn get_element_children_safe(
    element: &UIElement,
    context: &mut TreeBuildingContext,
) -> Result<Vec<UIElement>, AutomationError> {
    // Primarily use the standard children method
    match element.children() {
        Ok(children) => {
            context.increment_cache_hit(); // Count this as successful
            Ok(children)
        }
        Err(_) => {
            context.increment_fallback();
            // Only use timeout version if regular call fails
            get_element_children_with_timeout(
                element,
                Duration::from_millis(context.config.timeout_per_operation_ms),
            )
        }
    }
}

// Helper function to get element children with timeout
fn get_element_children_with_timeout(
    element: &UIElement,
    timeout: Duration,
) -> Result<Vec<UIElement>, AutomationError> {
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
            Err(AutomationError::PlatformError(
                "Timeout getting element children".to_string(),
            ))
        }
    }
}

// thread-safety
#[derive(Clone)]
pub(crate) struct ThreadSafeWinUIElement(pub(crate) Arc<uiautomation::UIElement>);

// send and sync for wrapper
unsafe impl Send for ThreadSafeWinUIElement {}
unsafe impl Sync for ThreadSafeWinUIElement {}

pub struct WindowsUIElement {
    pub(crate) element: ThreadSafeWinUIElement,
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
        Some(self.object_id().to_string().chars().take(6).collect())
    }

    fn role(&self) -> String {
        self.element
            .0
            .get_control_type()
            .map(|ct| ct.to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    }

    fn attributes(&self) -> UIElementAttributes {
        // OPTIMIZATION: Use cached properties first to avoid expensive UI automation calls
        // This significantly reduces the number of cross-process calls to the UI automation system

        let mut properties = HashMap::new();

        // Helper function to filter empty strings
        fn filter_empty_string(s: Option<String>) -> Option<String> {
            s.filter(|s| !s.is_empty())
        }

        // OPTIMIZATION: Try cached properties first, fallback to live properties only if needed
        let role = self
            .element
            .0
            .get_cached_control_type()
            .or_else(|_| self.element.0.get_control_type())
            .map(|ct| ct.to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        // OPTIMIZATION: Use cached name first
        let name = filter_empty_string(
            self.element
                .0
                .get_cached_name()
                .or_else(|_| self.element.0.get_name())
                .ok(),
        );

        // OPTIMIZATION: Only load automation ID if name is empty (fallback identifier)
        // This reduces unnecessary property lookups for most elements
        let automation_id_for_properties = if name.is_none() {
            self.element
                .0
                .get_cached_automation_id()
                .or_else(|_| self.element.0.get_automation_id())
                .ok()
                .and_then(|aid| {
                    if !aid.is_empty() {
                        Some(serde_json::Value::String(aid.clone()))
                    } else {
                        None
                    }
                })
        } else {
            None
        };

        if let Some(aid_value) = automation_id_for_properties {
            properties.insert("AutomationId".to_string(), Some(aid_value));
        }

        // OPTIMIZATION: Defer all other expensive properties:
        // - Skip label lookup (get_labeled_by + get_name chain)
        // - Skip value lookup (UIProperty::ValueValue)
        // - Skip description lookup (get_help_text)
        // - Skip keyboard focusable lookup (UIProperty::IsKeyboardFocusable)
        // - Skip additional property enumeration
        // These can be loaded on-demand when specifically requested

        // Return minimal attribute set for maximum performance
        UIElementAttributes {
            role,
            name,
            label: None,                 // Deferred - load on demand
            value: None,                 // Deferred - load on demand
            description: None,           // Deferred - load on demand
            properties,                  // Minimal properties only
            is_keyboard_focusable: None, // Deferred - load on demand
            is_focused: None,            // Deferred - load on demand
            bounds: None, // Will be populated by get_configurable_attributes if focusable
            text: None,
            enabled: None,
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
            Err(_) => {
                let temp_automation = create_ui_automation_with_com_init()?;
                let true_condition = temp_automation.create_true_condition().map_err(|e| {
                    AutomationError::PlatformError(format!(
                        "Failed to create true condition for child fallback: {e}"
                    ))
                })?;
                self.element
                    .0
                    .find_all(uiautomation::types::TreeScope::Children, &true_condition)
                    .map_err(|find_err| {
                        AutomationError::PlatformError(format!(
                            "Failed to get children (cached and non-cached): {find_err}"
                        ))
                    })? // Propagate error
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
        if click_result.is_err() {
            debug!("clickable point unavailable, falling back to bounding rectangle");
            if let Ok(rect) = self.element.0.get_bounding_rectangle() {
                println!("bounding rectangle: {rect:?}");
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
        Err(AutomationError::UnsupportedOperation(
            "`hover` doesn't not support".to_string(),
        ))
    }

    fn focus(&self) -> Result<(), AutomationError> {
        self.element
            .0
            .set_focus()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))
    }

    fn invoke(&self) -> Result<(), AutomationError> {
        let invoke_pat = self
            .element
            .0
            .get_pattern::<patterns::UIInvokePattern>()
            .map_err(|e| {
                let error_str = e.to_string();
                if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                    AutomationError::UnsupportedOperation(format!(
                        "Element does not support InvokePattern. This typically happens with custom controls, groups, or non-standard buttons. Try using 'click_element' instead. Error: {error_str}"
                    ))
                } else {
                    AutomationError::PlatformError(format!("Failed to get InvokePattern: {e}"))
                }
            })?;
        invoke_pat
            .invoke()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))
    }

    fn activate_window(&self) -> Result<(), AutomationError> {
        use windows::Win32::UI::WindowsAndMessaging::{
            BringWindowToTop, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
        };

        debug!(
            "Activating window by focusing element: {:?}",
            self.element.0
        );

        // First try to get the native window handle
        let hwnd = match self.element.0.get_native_window_handle() {
            Ok(handle) => handle,
            Err(_) => {
                // Fallback to just setting focus if we can't get the window handle
                debug!("Could not get native window handle, falling back to set_focus");
                return self.focus();
            }
        };

        unsafe {
            let hwnd_param: windows::Win32::Foundation::HWND = hwnd.into();

            // Check if the window is minimized and restore it if needed
            if IsIconic(hwnd_param).as_bool() {
                debug!("Window is minimized, restoring it");
                let _ = ShowWindow(hwnd_param, SW_RESTORE);
            }

            // Bring the window to the top of the Z order
            let _ = BringWindowToTop(hwnd_param);

            // Set as the foreground window (this is the key method for activation)
            let result = SetForegroundWindow(hwnd_param);

            if !result.as_bool() {
                debug!("SetForegroundWindow failed, but continuing");
                // Note: SetActiveWindow is not available in the current Windows crate version
                // The SetForegroundWindow should be sufficient for most cases
            }

            // Finally, set focus to the specific element
            let _ = self.element.0.set_focus();
        }

        debug!("Window activation completed");
        Ok(())
    }

    fn minimize_window(&self) -> Result<(), AutomationError> {
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_MINIMIZE};

        debug!("Minimizing window for element: {:?}", self.element.0);

        // First try to get the native window handle
        let hwnd = match self.element.0.get_native_window_handle() {
            Ok(handle) => handle,
            Err(_) => {
                return Err(AutomationError::PlatformError(
                    "Could not get native window handle for minimize operation".to_string(),
                ));
            }
        };

        unsafe {
            let hwnd_param: windows::Win32::Foundation::HWND = hwnd.into();

            // Minimize the window
            let result = ShowWindow(hwnd_param, SW_MINIMIZE);

            if result.as_bool() {
                debug!("Window minimized successfully");
            } else {
                debug!("Window was already minimized or minimize operation had no effect");
            }
        }

        debug!("Window minimize operation completed");
        Ok(())
    }

    fn maximize_window(&self) -> Result<(), AutomationError> {
        debug!("Maximizing window for element: {:?}", self.element.0);

        // First try using the WindowPattern which is the preferred method
        if let Ok(window_pattern) = self.element.0.get_pattern::<patterns::UIWindowPattern>() {
            debug!("Using WindowPattern to maximize window");
            window_pattern
                .set_window_visual_state(uiautomation::types::WindowVisualState::Maximized)
                .map_err(|e| {
                    AutomationError::PlatformError(format!(
                        "Failed to maximize window using WindowPattern: {e}"
                    ))
                })?;
            debug!("Window maximized successfully using WindowPattern");
            return Ok(());
        }

        // Fallback to native Windows API if WindowPattern is not available
        debug!("WindowPattern not available, falling back to native Windows API");
        let hwnd = match self.element.0.get_native_window_handle() {
            Ok(handle) => handle,
            Err(_) => {
                return Err(AutomationError::PlatformError(
                    "Could not get native window handle for maximize operation".to_string(),
                ));
            }
        };

        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_MAXIMIZE};

        unsafe {
            let hwnd_param: windows::Win32::Foundation::HWND = hwnd.into();

            // Maximize the window
            let result = ShowWindow(hwnd_param, SW_MAXIMIZE);

            if result.as_bool() {
                debug!("Window maximized successfully using native API");
            } else {
                debug!("Window was already maximized or maximize operation had no effect");
            }
        }

        debug!("Window maximize operation completed");
        Ok(())
    }

    fn type_text(&self, text: &str, use_clipboard: bool) -> Result<(), AutomationError> {
        let control_type = self
            .element
            .0
            .get_control_type()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

        debug!(
            "typing text with control_type: {:#?}, use_clipboard: {}",
            control_type, use_clipboard
        );

        if use_clipboard {
            // Try clipboard typing first
            match self.element.0.send_text_by_clipboard(text) {
                Ok(()) => Ok(()),
                Err(e) => {
                    // Clipboard method failed, fall back to key-by-key typing
                    debug!(
                        "Clipboard typing returned error: {:?}. Using key-by-key input instead.",
                        e
                    );
                    self.element
                        .0
                        .send_text(text, 10)
                        .map_err(|e| AutomationError::PlatformError(e.to_string()))
                }
            }
        } else {
            // Use standard typing method
            self.element
                .0
                .send_text(text, 10)
                .map_err(|e| AutomationError::PlatformError(e.to_string()))
        }
    }

    fn press_key(&self, key: &str) -> Result<(), AutomationError> {
        let control_type = self.element.0.get_control_type().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get control type: {e:?}"))
        })?;
        // check if element accepts input, similar :D
        debug!("pressing key with control_type: {:#?}", control_type);
        self.element
            .0
            .send_keys(key, 10)
            .map_err(|e| AutomationError::PlatformError(format!("Failed to press key: {e:?}")))
    }

    fn get_text(&self, max_depth: usize) -> Result<String, AutomationError> {
        let mut all_texts = Vec::new();
        let automation = create_ui_automation_with_com_init()?;

        // Create a function to extract text recursively
        fn extract_text_from_element(
            automation: &UIAutomation,
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
                Err(_) => {
                    match automation.create_true_condition() {
                        Ok(true_condition) => {
                            // Perform the non-cached search for direct children
                            element
                                .find_all(uiautomation::types::TreeScope::Children, &true_condition)
                                .unwrap_or_default()
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
            };

            // Process the children (either cached or found via fallback)
            for child in children_to_process {
                let _ = extract_text_from_element(
                    automation,
                    &child,
                    texts,
                    current_depth + 1,
                    max_depth,
                );
            }

            Ok(())
        }

        // Extract text from the element and its descendants
        extract_text_from_element(&automation, &self.element.0, &mut all_texts, 0, max_depth)?;

        // Join the texts with spaces
        Ok(all_texts.join(" "))
    }

    fn set_value(&self, value: &str) -> Result<(), AutomationError> {
        debug!(
            "setting value: {:#?} to ui element {:#?}",
            &value, &self.element.0
        );

        let value_par = self
            .element
            .0
            .get_pattern::<patterns::UIValuePattern>()
            .map_err(|e| {
                let error_str = e.to_string();
                if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                    AutomationError::UnsupportedOperation(format!(
                        "Element does not support ValuePattern. This control cannot have its value set directly. Try using 'type_into_element' for text input, or 'select_option' for dropdowns. Error: {error_str}"
                    ))
                } else {
                    AutomationError::PlatformError(format!("Failed to get ValuePattern: {e}"))
                }
            })?;

        value_par
            .set_value(value)
            .map_err(|e| AutomationError::PlatformError(e.to_string()))
    }

    fn is_enabled(&self) -> Result<bool, AutomationError> {
        self.element
            .0
            .is_enabled()
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))
    }

    fn is_visible(&self) -> Result<bool, AutomationError> {
        self.element
            .0
            .is_offscreen()
            .map(|is_offscreen| !is_offscreen)
            .map_err(|e| AutomationError::ElementNotFound(e.to_string()))
    }

    fn is_focused(&self) -> Result<bool, AutomationError> {
        self.element.0.has_keyboard_focus().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get keyboard focus state: {e}"))
        })
    }

    fn perform_action(&self, action: &str) -> Result<(), AutomationError> {
        // actions those don't take args
        match action {
            "focus" => self.focus(),
            "invoke" => self.invoke(),
            "click" => self.click().map(|_| ()),
            "double_click" => self.double_click().map(|_| ()),
            "right_click" => self.right_click().map(|_| ()),
            "toggle" => {
                let toggle_pattern = self
                    .element
                    .0
                    .get_pattern::<patterns::UITogglePattern>()
                    .map_err(|e| {
                        let error_str = e.to_string();
                        if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                            AutomationError::UnsupportedOperation(format!(
                                "Element does not support TogglePattern. This is not a toggleable control (checkbox, switch, etc.). Try using 'click' instead. Error: {error_str}"
                            ))
                        } else {
                            AutomationError::PlatformError(format!("Failed to get TogglePattern: {e}"))
                        }
                    })?;
                toggle_pattern
                    .toggle()
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))
            }
            "expand_collapse" => {
                let expand_collapse_pattern = self
                    .element
                    .0
                    .get_pattern::<patterns::UIExpandCollapsePattern>()
                    .map_err(|e| {
                        let error_str = e.to_string();
                        if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                            AutomationError::UnsupportedOperation(format!(
                                "Element does not support ExpandCollapsePattern. This is not an expandable control (tree item, dropdown, etc.). Try using 'click' to interact with it. Error: {error_str}"
                            ))
                        } else {
                            AutomationError::PlatformError(format!("Failed to get ExpandCollapsePattern: {e}"))
                        }
                    })?;
                expand_collapse_pattern
                    .expand()
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))
            }
            _ => Err(AutomationError::UnsupportedOperation(format!(
                "action '{action}' not supported"
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
        // 1. Find a scrollable parent (or self)
        let mut scrollable_element: Option<uiautomation::UIElement> = None;
        let mut current_element_arc = self.element.0.clone();

        for _ in 0..7 {
            // Search up to 7 levels up the tree
            if let Ok(_pattern) = current_element_arc.get_pattern::<patterns::UIScrollPattern>() {
                // Element supports scrolling, we found our target
                scrollable_element = Some(current_element_arc.as_ref().clone());
                break;
            }

            // Move to parent
            if let Ok(parent) = current_element_arc.get_cached_parent() {
                // Check if we've hit the root or a cycle
                if let (Ok(cur_id), Ok(par_id)) = (
                    current_element_arc.get_runtime_id(),
                    parent.get_runtime_id(),
                ) {
                    if cur_id == par_id {
                        break;
                    }
                }
                current_element_arc = Arc::new(parent);
            } else {
                // No more parents
                break;
            }
        }

        let target_element = scrollable_element.ok_or_else(|| {
            AutomationError::UnsupportedOperation(
                "No scrollable container found for this element".to_string(),
            )
        })?;

        // 2. Use ScrollPattern to scroll
        if let Ok(scroll_pattern) = target_element.get_pattern::<patterns::UIScrollPattern>() {
            let h_amount = uiautomation::types::ScrollAmount::NoAmount;
            let v_amount = match direction {
                "up" => uiautomation::types::ScrollAmount::LargeDecrement,
                "down" => uiautomation::types::ScrollAmount::LargeIncrement,
                _ => {
                    return Err(AutomationError::InvalidArgument(
                        "Invalid scroll direction. Only 'up' or 'down' are supported.".to_string(),
                    ))
                }
            };

            let num_scrolls = amount.round().max(1.0) as usize;
            for i in 0..num_scrolls {
                if scroll_pattern.scroll(h_amount, v_amount).is_err() {
                    // If pattern fails, break and try the key press fallback
                    warn!(
                        "ScrollPattern failed on iteration {}. Attempting key-press fallback.",
                        i
                    );
                    return self.scroll_with_fallback(direction, amount);
                }
                // Small delay between programmatic scrolls to allow UI to catch up
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            return Ok(());
        }

        // 3. If ScrollPattern fails, fall back to key presses on the original element
        self.scroll_with_fallback(direction, amount)
    }

    fn is_keyboard_focusable(&self) -> Result<bool, AutomationError> {
        let variant = self
            .element
            .0
            .get_property_value(UIProperty::IsKeyboardFocusable)
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        variant.try_into().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to convert IsKeyboardFocusable to bool: {e:?}"
            ))
        })
    }

    // New method for mouse drag
    fn mouse_drag(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> Result<(), AutomationError> {
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
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
            MOUSEEVENTF_MOVE, MOUSEINPUT,
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
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
            MOUSEINPUT,
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
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTUP, MOUSEINPUT,
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
            AutomationError::PlatformError(format!("Failed to get process ID for element: {e}"))
        })?;

        // Create a WindowsEngine instance to use its methods.
        // This follows the pattern in `create_locator` but might be inefficient if called frequently.
        let engine = WindowsEngine::new(false, false).map_err(|e| {
            AutomationError::PlatformError(format!("Failed to create WindowsEngine: {e}"))
        })?;

        // Get the application element by PID
        match engine.get_application_by_pid(pid as i32, Some(DEFAULT_FIND_TIMEOUT)) {
            // Cast pid to i32
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
                        "Failed to get control type for element during window search (iteration {i}): {e}"
                    )));
                }
            }

            // Try to get the parent
            match current_element_arc.get_cached_parent() {
                Ok(parent_uia_element) => {
                    // Check if parent is same as current (e.g. desktop root's parent is itself)
                    // This requires getting runtime IDs, which can also fail.
                    let current_runtime_id = current_element_arc.get_runtime_id().map_err(|e| {
                        AutomationError::PlatformError(format!(
                            "Failed to get runtime_id for current element: {e}"
                        ))
                    })?;
                    let parent_runtime_id = parent_uia_element.get_runtime_id().map_err(|e| {
                        AutomationError::PlatformError(format!(
                            "Failed to get runtime_id for parent element: {e}"
                        ))
                    })?;

                    if parent_runtime_id == current_runtime_id {
                        debug!(
                            "Parent element has same runtime ID as current, stopping window search."
                        );
                        break; // Reached the top or a cycle.
                    }
                    current_element_arc = Arc::new(parent_uia_element); // Move to the parent
                }
                Err(_) => {
                    break;
                }
            }
        }
        // If loop finishes, no element with ControlType::Window was found.
        Ok(None)
    }

    fn highlight(
        &self,
        color: Option<u32>,
        duration: Option<std::time::Duration>,
    ) -> Result<(), AutomationError> {
        use std::time::Instant;
        use windows::Win32::Foundation::{COLORREF, POINT};
        use windows::Win32::Graphics::Gdi::{
            CreatePen, DeleteObject, GetDC, GetStockObject, Rectangle, ReleaseDC, SelectObject,
            HGDIOBJ, NULL_BRUSH, PS_SOLID,
        };
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

        self.element.0.try_focus();

        // Get the element's bounding rectangle
        let rect = self.element.0.get_bounding_rectangle().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get element bounds: {e}"))
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
                Ok(windows) => windows
                    .iter()
                    .find(|w| w.is_focused().unwrap_or(false))
                    .and_then(|focused_window| focused_window.current_monitor().ok())
                    .and_then(|monitor| monitor.scale_factor().ok().map(|factor| factor as f64)),
                Err(e) => {
                    error!("Failed to get windows: {}", e);
                    None
                }
            }
        }

        // Try to get scale factor from focused window first, fall back to cursor position
        let scale_factor =
            get_scale_factor_from_focused_window().unwrap_or_else(get_scale_factor_from_cursor);

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
            AutomationError::PlatformError(format!("Failed to get process ID for element: {e}"))
        })
    }

    fn close(&self) -> Result<(), AutomationError> {
        // Check the control type to determine if this element is closable
        let control_type = self.element.0.get_control_type().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get control type: {e}"))
        })?;

        match control_type {
            ControlType::Window | ControlType::Pane => {
                // For windows and panes, try to close them

                // First try using the WindowPattern to close the window
                if let Ok(window_pattern) =
                    self.element.0.get_pattern::<patterns::UIWindowPattern>()
                {
                    debug!("Attempting to close window using WindowPattern");
                    let close_result = window_pattern.close();
                    match close_result {
                        Ok(()) => return Ok(()),
                        Err(e) => {
                            let error_str = e.to_string();
                            if error_str.contains("not support")
                                || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE")
                            {
                                // Window doesn't support WindowPattern, try Alt+F4
                                debug!("WindowPattern not supported, falling back to Alt+F4");
                                self.element.0.try_focus();
                                return self.element
                                    .0
                                    .send_keys("%{F4}", 10) // Alt+F4
                                    .map_err(|e2| {
                                        AutomationError::PlatformError(format!(
                                            "Failed to close window: WindowPattern not supported and Alt+F4 failed: {e2}"
                                        ))
                                    });
                            } else {
                                return Err(AutomationError::PlatformError(format!(
                                    "Failed to close window: {e}"
                                )));
                            }
                        }
                    }
                }

                // Fallback: try to send Alt+F4 to close the window
                debug!("WindowPattern not available, trying Alt+F4 as fallback");
                self.element.0.try_focus(); // Focus first
                self.element
                    .0
                    .send_keys("%{F4}", 10) // Alt+F4
                    .map_err(|e| {
                        AutomationError::PlatformError(format!("Failed to send Alt+F4: {e}"))
                    })
            }
            ControlType::Button => {
                // For buttons, check if it's a close button by name/text
                let name = self.element.0.get_name().unwrap_or_default().to_lowercase();
                if name.contains("close")
                    || name.contains("×")
                    || name.contains("✕")
                    || name.contains("x")
                {
                    debug!("Clicking close button: {}", name);
                    self.click().map(|_| ())
                } else {
                    // Regular button - not a close action
                    debug!("Button '{}' is not a close button", name);
                    Err(AutomationError::UnsupportedOperation(format!(
                        "Button '{name}' is not a close button. Only windows, dialogs, and close buttons can be closed."
                    )))
                }
            }
            _ => {
                // For other control types (text, edit, etc.), closing is not supported
                debug!("Element type {:?} is not closable", control_type);
                Err(AutomationError::UnsupportedOperation(format!(
                    "Element of type '{control_type}' cannot be closed. Only windows, dialogs, and close buttons support the close operation."
                )))
            }
        }
    }

    fn capture(&self) -> Result<ScreenshotResult, AutomationError> {
        // Get the raw UIAutomation bounds
        let rect = self.element.0.get_bounding_rectangle().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get bounding rectangle: {e}"))
        })?;

        // Get all monitors that intersect with the element
        let mut intersected_monitors = Vec::new();
        let monitors = xcap::Monitor::all()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitors: {e}")))?;

        for monitor in monitors {
            let monitor_x = monitor.x().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor x: {e}"))
            })?;
            let monitor_y = monitor.y().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor y: {e}"))
            })?;
            let monitor_width = monitor.width().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor width: {e}"))
            })? as i32;
            let monitor_height = monitor.height().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor height: {e}"))
            })? as i32;

            // Check if element intersects with this monitor
            if rect.get_left() < monitor_x + monitor_width
                && rect.get_left() + rect.get_width() > monitor_x
                && rect.get_top() < monitor_y + monitor_height
                && rect.get_top() + rect.get_height() > monitor_y
            {
                intersected_monitors.push(monitor);
            }
        }

        if intersected_monitors.is_empty() {
            return Err(AutomationError::PlatformError(
                "Element is not visible on any monitor".to_string(),
            ));
        }

        // If element spans multiple monitors, capture from the primary monitor
        let monitor = &intersected_monitors[0];
        let scale_factor = monitor.scale_factor().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get scale factor: {e}"))
        })?;

        // Get monitor bounds
        let monitor_x = monitor
            .x()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitor x: {e}")))?
            as u32;
        let monitor_y = monitor
            .y()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get monitor y: {e}")))?
            as u32;
        let monitor_width = monitor.width().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor width: {e}"))
        })?;
        let monitor_height = monitor.height().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor height: {e}"))
        })?;

        // Calculate scaled coordinates
        let scaled_x = (rect.get_left() as f64 * scale_factor as f64) as u32;
        let scaled_y = (rect.get_top() as f64 * scale_factor as f64) as u32;
        let scaled_width = (rect.get_width() as f64 * scale_factor as f64) as u32;
        let scaled_height = (rect.get_height() as f64 * scale_factor as f64) as u32;

        // Convert to relative coordinates for capture_region
        let rel_x = scaled_x.saturating_sub(monitor_x);
        let rel_y = scaled_y.saturating_sub(monitor_y);

        // Ensure width and height don't exceed monitor bounds
        let rel_width = std::cmp::min(scaled_width, monitor_width - rel_x);
        let rel_height = std::cmp::min(scaled_height, monitor_height - rel_y);

        // Capture the screen region
        let capture = monitor
            .capture_region(rel_x, rel_y, rel_width, rel_height)
            .map_err(|e| {
                AutomationError::PlatformError(format!("Failed to capture region: {e}"))
            })?;

        Ok(ScreenshotResult {
            image_data: capture.to_vec(),
            width: rel_width,
            height: rel_height,
            monitor: None,
        })
    }

    fn set_transparency(&self, percentage: u8) -> Result<(), AutomationError> {
        // Convert percentage (0-100) to alpha (0-255)
        let alpha = ((percentage as f32 / 100.0) * 255.0) as u8;

        // Get the window handle
        let hwnd = self.element.0.get_native_window_handle().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to get native window handle of element: {e}"
            ))
        })?;

        // Set the window to be layered
        unsafe {
            let style = windows::Win32::UI::WindowsAndMessaging::GetWindowLongW(
                hwnd.into(),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX(-20), // GWL_EXSTYLE
            );
            if style == 0 {
                return Err(AutomationError::PlatformError(
                    "Failed to get window style".to_string(),
                ));
            }
            let new_style = style | 0x00080000; // WS_EX_LAYERED
            if windows::Win32::UI::WindowsAndMessaging::SetWindowLongW(
                hwnd.into(),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX(-20), // GWL_EXSTYLE
                new_style,
            ) == 0
            {
                return Err(AutomationError::PlatformError(
                    "Failed to set window style".to_string(),
                ));
            }
        }

        // Set the transparency
        unsafe {
            let result = windows::Win32::UI::WindowsAndMessaging::SetLayeredWindowAttributes(
                hwnd.into(),
                windows::Win32::Foundation::COLORREF(0), // crKey - not used with LWA_ALPHA
                alpha,
                windows::Win32::UI::WindowsAndMessaging::LAYERED_WINDOW_ATTRIBUTES_FLAGS(
                    0x00000002,
                ), // LWA_ALPHA
            );
            if result.is_err() {
                return Err(AutomationError::PlatformError(
                    "Failed to set window transparency".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn url(&self) -> Option<String> {
        let automation = match create_ui_automation_with_com_init() {
            Ok(a) => a,
            Err(e) => {
                debug!(
                    "Failed to create UIAutomation instance for URL detection: {}",
                    e
                );
                return None;
            }
        };

        // Find the root window for the element.
        let search_root = if let Ok(Some(window)) = self.window() {
            window
                .as_any()
                .downcast_ref::<WindowsUIElement>()
                .map(|win_el| win_el.element.0.clone())
                .unwrap_or_else(|| self.element.0.clone())
        } else {
            self.element.0.clone()
        };

        debug!(
            "URL search root: {}",
            search_root.get_name().unwrap_or_default()
        );

        // Try to find address bar using a more flexible filter function.
        let address_bar_keywords = ["address", "location", "url", "website", "search", "go to"];

        let matcher = automation
            .create_matcher()
            .from_ref(&search_root)
            .control_type(ControlType::Edit)
            .filter_fn(Box::new(move |e: &uiautomation::UIElement| {
                if let Ok(name) = e.get_name() {
                    let name_lower = name.to_lowercase();
                    if address_bar_keywords
                        .iter()
                        .any(|&keyword| name_lower.contains(keyword))
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }))
            .timeout(200) // Quick search for the best case
            .depth(10);

        if let Ok(element) = matcher.find_first() {
            if let Ok(value_pattern) = element.get_pattern::<patterns::UIValuePattern>() {
                if let Ok(value) = value_pattern.get_value() {
                    debug!("Found URL via keyword search for address bar: {}", value);
                    return Some(value);
                }
            }
        }

        // Fallback: If no specifically named address bar is found,
        // search for ANY edit control with a URL in it, as a broader but still constrained search.
        // This can help with non-standard browsers or updated UI.
        let edit_condition = automation
            .create_property_condition(
                UIProperty::ControlType,
                Variant::from(ControlType::Edit as i32),
                None,
            )
            .unwrap();
        if let Ok(candidates) = search_root.find_all(TreeScope::Descendants, &edit_condition) {
            for candidate in candidates {
                if let Ok(value_pattern) = candidate.get_pattern::<patterns::UIValuePattern>() {
                    if let Ok(url) = value_pattern.get_value() {
                        if url.starts_with("http") {
                            debug!("Found URL in fallback search of Edit controls: {}", url);
                            return Some(url);
                        }
                    }
                }
            }
        }

        debug!("Could not find URL in any address bar candidate.");
        None
    }

    fn select_option(&self, option_name: &str) -> Result<(), AutomationError> {
        // Expand the dropdown/combobox first
        if let Ok(expand_collapse_pattern) = self
            .element
            .0
            .get_pattern::<patterns::UIExpandCollapsePattern>()
        {
            expand_collapse_pattern.expand().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to expand element: {e}"))
            })?;
        }

        // Wait a moment for options to appear
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Find the specific option by name
        let automation = UIAutomation::new_direct()
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
        let option_element = self
            .element
            .0
            .find_first(
                TreeScope::Descendants,
                &automation
                    .create_property_condition(
                        uiautomation::types::UIProperty::Name,
                        option_name.into(),
                        None,
                    )
                    .unwrap(),
            )
            .map_err(|e| {
                AutomationError::ElementNotFound(format!(
                    "Option '{option_name}' not found in dropdown. Make sure the dropdown is expanded and the option name is exact. Error: {e}"
                ))
            })?;

        // Select the option
        if let Ok(selection_item_pattern) =
            option_element.get_pattern::<patterns::UISelectionItemPattern>()
        {
            selection_item_pattern.select().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to select option: {e}"))
            })?;
        } else {
            // Fallback to click if selection pattern is not available
            debug!(
                "SelectionItemPattern not available for option '{}', falling back to click",
                option_name
            );
            option_element.click().map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to click option '{option_name}': {e}"
                ))
            })?;
        }

        // Try to collapse the dropdown again
        if let Ok(expand_collapse_pattern) = self
            .element
            .0
            .get_pattern::<patterns::UIExpandCollapsePattern>()
        {
            let _ = expand_collapse_pattern.collapse();
        }

        Ok(())
    }

    fn list_options(&self) -> Result<Vec<String>, AutomationError> {
        let mut options = Vec::new();
        // Ensure the element is expanded to reveal options
        if let Ok(expand_collapse_pattern) = self
            .element
            .0
            .get_pattern::<patterns::UIExpandCollapsePattern>()
        {
            let state_variant = self
                .element
                .0
                .get_property_value(UIProperty::ExpandCollapseExpandCollapseState)
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

            let state_val: i32 = state_variant.try_into().map_err(|_| {
                AutomationError::PlatformError(
                    "Failed to convert expand/collapse state variant to i32".to_string(),
                )
            })?;
            let state = match state_val {
                0 => uiautomation::types::ExpandCollapseState::Collapsed,
                1 => uiautomation::types::ExpandCollapseState::Expanded,
                2 => uiautomation::types::ExpandCollapseState::PartiallyExpanded,
                3 => uiautomation::types::ExpandCollapseState::LeafNode,
                _ => uiautomation::types::ExpandCollapseState::Collapsed, // Default case
            };

            if state != uiautomation::types::ExpandCollapseState::Expanded {
                expand_collapse_pattern.expand().map_err(|e| {
                    AutomationError::PlatformError(format!(
                        "Failed to expand element to list options: {e}"
                    ))
                })?;
                std::thread::sleep(std::time::Duration::from_millis(200)); // Wait for animation
            }
        } else {
            debug!("Element does not support ExpandCollapsePattern, attempting to list visible children directly");
        }

        // Search for ListItem children
        let children = self.children()?;
        for child in children {
            let role = child.role();
            if role == "ListItem" || role == "MenuItem" || role == "Option" {
                if let Some(name) = child.name() {
                    options.push(name);
                }
            }
        }

        if options.is_empty() {
            debug!("No options found. The element might not be a dropdown/list, or options might have different roles");
        }

        Ok(options)
    }

    fn is_toggled(&self) -> Result<bool, AutomationError> {
        let toggle_pattern = self.element.0.get_pattern::<patterns::UITogglePattern>();

        if let Ok(pattern) = toggle_pattern {
            let state = pattern.get_toggle_state().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get toggle state: {e}"))
            })?;
            return Ok(state == uiautomation::types::ToggleState::On);
        }

        // Fallback: Check SelectionItemPattern as some controls might use it
        if let Ok(selection_pattern) = self
            .element
            .0
            .get_pattern::<patterns::UISelectionItemPattern>()
        {
            if let Ok(is_selected) = selection_pattern.is_selected() {
                return Ok(is_selected);
            }
        }

        // Fallback: Check name for keywords if no pattern is definitive
        if let Ok(name) = self.element.0.get_name() {
            let name_lower = name.to_lowercase();
            if name_lower.contains("checked")
                || name_lower.contains("selected")
                || name_lower.contains("toggled")
            {
                return Ok(true);
            }
            if name_lower.contains("unchecked") || name_lower.contains("not selected") {
                return Ok(false);
            }
        }

        Err(AutomationError::UnsupportedOperation(format!(
            "Element '{}' does not support TogglePattern or provide state information. This element is not a toggleable control. Use 'is_selected' for selection states.",
            self.element.0.get_name().unwrap_or_default()
        )))
    }

    fn set_toggled(&self, state: bool) -> Result<(), AutomationError> {
        let current_state = self.is_toggled()?;
        if current_state == state {
            return Ok(()); // Already in the desired state
        }

        let toggle_pattern = self
            .element
            .0
            .get_pattern::<patterns::UITogglePattern>()
            .map_err(|e| {
                let error_str = e.to_string();
                if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                    AutomationError::UnsupportedOperation(format!(
                        "Element does not support TogglePattern. This is not a toggleable control (checkbox, switch, etc.). For checkboxes, try 'click_element'. For radio buttons, use 'set_selected'. Error: {error_str}"
                    ))
                } else {
                    AutomationError::PlatformError(format!("Failed to get TogglePattern: {e}"))
                }
            })?;

        toggle_pattern
            .toggle()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to toggle: {e}")))
    }

    fn get_range_value(&self) -> Result<f64, AutomationError> {
        let range_pattern = self
            .element
            .0
            .get_pattern::<patterns::UIRangeValuePattern>()
            .map_err(|e| {
                let error_str = e.to_string();
                if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                    AutomationError::UnsupportedOperation(format!(
                        "Element does not support RangeValuePattern. This is not a range control (slider, progress bar, etc.). Error: {error_str}"
                    ))
                } else {
                    AutomationError::PlatformError(format!("Failed to get RangeValuePattern: {e}"))
                }
            })?;
        range_pattern
            .get_value()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get range value: {e}")))
    }

    fn set_range_value(&self, value: f64) -> Result<(), AutomationError> {
        self.focus()?; // Always focus first for keyboard interaction

        let range_pattern = self
            .element
            .0
            .get_pattern::<patterns::UIRangeValuePattern>()
            .map_err(|e| {
                let error_str = e.to_string();
                if error_str.contains("not support") || error_str.contains("UIA_E_ELEMENTNOTAVAILABLE") {
                    AutomationError::UnsupportedOperation(format!(
                        "Element does not support RangeValuePattern. This is not a range control (slider, progress bar, etc.). Try using keyboard arrows or mouse drag for custom sliders. Error: {error_str}"
                    ))
                } else {
                    AutomationError::PlatformError(format!("Failed to get RangeValuePattern: {e}"))
                }
            })?;

        // Try setting value directly first, as it's the most efficient method.
        if range_pattern.set_value(value).is_ok() {
            // Optional: Short sleep to allow UI to update.
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Ok(new_value) = range_pattern.get_value() {
                // Use a tolerance for floating-point comparison.
                if (new_value - value).abs() < 1.0 {
                    debug!("Direct set_value for RangeValuePattern succeeded.");
                    return Ok(());
                }
                debug!(
                    "Direct set_value was inaccurate, new value: {}. Expected: {}",
                    new_value, value
                );
            }
        }

        // Fallback to keyboard simulation.
        debug!("Direct set_value for RangeValuePattern failed or was inaccurate, falling back to keyboard simulation.");

        let min_value = range_pattern
            .get_minimum()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get min value: {e}")))?;
        let max_value = range_pattern
            .get_maximum()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get max value: {e}")))?;

        let mut small_change = range_pattern.get_small_change().unwrap_or(0.0);

        if small_change <= 0.0 {
            debug!("Slider small_change is not positive, calculating fallback step.");
            let range = max_value - min_value;
            if range > 0.0 {
                // Use 1% of the range as a reasonable step, or a minimum of 1.0
                small_change = (range / 100.0).max(1.0);
            } else {
                // If range is zero or negative, we can't do much.
                return Err(AutomationError::PlatformError(
                    "Slider range is zero or negative, cannot use keyboard fallback.".to_string(),
                ));
            }
        }

        // Clamp the target value to be within the allowed range.
        let target_value = value.clamp(min_value, max_value);

        debug!(
            "Slider properties: min={}, max={}, small_change={}, target={}",
            min_value, max_value, small_change, target_value
        );

        // Decide whether to move from min or max.
        let from_min_dist = (target_value - min_value).abs();
        let from_max_dist = (max_value - target_value).abs();

        if from_min_dist <= from_max_dist {
            // Go to min and step up.
            debug!("Moving from min. Resetting to HOME.");
            self.press_key("{home}")?;
            std::thread::sleep(std::time::Duration::from_millis(50));
            let num_steps = (from_min_dist / small_change).round() as u32;
            debug!(
                "Pressing RIGHT {} times to reach {}",
                num_steps, target_value
            );
            for i in 0..num_steps {
                self.press_key("{right}")?;
                std::thread::sleep(std::time::Duration::from_millis(10));
                debug!("Step {}/{}: Pressed RIGHT", i + 1, num_steps);
            }
        } else {
            // Go to max and step down.
            debug!("Moving from max. Resetting to END.");
            self.press_key("{end}")?;
            std::thread::sleep(std::time::Duration::from_millis(50));
            let num_steps = (from_max_dist / small_change).round() as u32;
            debug!(
                "Pressing LEFT {} times to reach {}",
                num_steps, target_value
            );
            for i in 0..num_steps {
                self.press_key("{left}")?;
                std::thread::sleep(std::time::Duration::from_millis(10));
                debug!("Step {}/{}: Pressed LEFT", i + 1, num_steps);
            }
        }

        Ok(())
    }

    fn is_selected(&self) -> Result<bool, AutomationError> {
        // First, try SelectionItemPattern, which is the primary meaning of "selected".
        if let Ok(selection_item_pattern) = self
            .element
            .0
            .get_pattern::<patterns::UISelectionItemPattern>()
        {
            if selection_item_pattern.is_selected().unwrap_or(false) {
                return Ok(true);
            }
        }

        // As a fallback for convenience, check if it's a "toggled" control like a checkbox.
        if let Ok(toggle_pattern) = self.element.0.get_pattern::<patterns::UITogglePattern>() {
            if let Ok(state) = toggle_pattern.get_toggle_state() {
                if state == uiautomation::types::ToggleState::On {
                    return Ok(true);
                }
            }
        }

        // Final fallback: for some controls (like calendar dates), selection is indicated by focus.
        if self.is_focused().unwrap_or(false) {
            return Ok(true);
        }

        // If we've reached here, none of the positive checks passed.
        // Return false if any of the patterns were supported, otherwise error.
        if self
            .element
            .0
            .get_pattern::<patterns::UISelectionItemPattern>()
            .is_ok()
            || self
                .element
                .0
                .get_pattern::<patterns::UITogglePattern>()
                .is_ok()
        {
            Ok(false)
        } else {
            // Fallback: Check name for keywords if no pattern is definitive
            if let Ok(name) = self.element.0.get_name() {
                let name_lower = name.to_lowercase();
                if name_lower.contains("checked") || name_lower.contains("selected") {
                    return Ok(true);
                }
                if name_lower.contains("unchecked") || name_lower.contains("not selected") {
                    return Ok(false);
                }
            }
            Err(AutomationError::UnsupportedOperation(
                "Element supports neither SelectionItemPattern nor TogglePattern, and is not focused."
                    .to_string(),
            ))
        }
    }

    fn set_selected(&self, state: bool) -> Result<(), AutomationError> {
        // First, try SelectionItemPattern, which is the primary meaning of "selected".
        if let Ok(selection_item_pattern) = self
            .element
            .0
            .get_pattern::<patterns::UISelectionItemPattern>()
        {
            let is_currently_selected = selection_item_pattern.is_selected().unwrap_or(false);

            if state && !is_currently_selected {
                // If we need to select it, and it's not selected yet.
                return selection_item_pattern.select().map_err(|e| {
                    AutomationError::PlatformError(format!("Failed to select item: {e}"))
                });
            } else if !state && is_currently_selected {
                // If we need to deselect it, and it's currently selected.
                // This is for multi-select controls; for single-select this may fail.
                return selection_item_pattern.remove_from_selection().map_err(|e| {
                    AutomationError::PlatformError(format!(
                        "Failed to remove item from selection. This might be a single-select control that doesn't support deselection: {e}"
                    ))
                });
            }
            return Ok(()); // Already in the desired state.
        }

        // As a fallback for convenience, check if it's a "toggled" control like a checkbox.
        if self
            .element
            .0
            .get_pattern::<patterns::UITogglePattern>()
            .is_ok()
        {
            debug!("Element doesn't support SelectionItemPattern, falling back to TogglePattern");
            return self.set_toggled(state);
        }

        // Final fallback: if we want to select, try clicking.
        if state {
            debug!("Element supports neither SelectionItemPattern nor TogglePattern, falling back to click");
            return self.click().map(|_| ());
        }

        Err(AutomationError::UnsupportedOperation(
            "Element cannot be deselected as it supports neither SelectionItemPattern nor TogglePattern. For radio buttons and list items, deselection typically happens by selecting another item.".to_string(),
        ))
    }
}

trait ScrollFallback {
    fn scroll_with_fallback(&self, direction: &str, amount: f64) -> Result<(), AutomationError>;
}

impl ScrollFallback for WindowsUIElement {
    fn scroll_with_fallback(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        warn!(
            "Using key-press scroll fallback for element: {:?}",
            self.element.0.get_name().unwrap_or_default()
        );
        self.focus().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to focus element for scroll fallback: {e:?}"
            ))
        })?;

        match direction {
            "up" | "down" => {
                let times = amount.abs().round().max(1.0) as usize;
                let key = if direction == "up" {
                    "{page_up}"
                } else {
                    "{page_down}"
                };
                for _ in 0..times {
                    self.press_key(key)?;
                }
            }
            _ => {
                return Err(AutomationError::UnsupportedOperation(
                    "Only 'up' and 'down' scroll directions are supported".to_string(),
                ));
            }
        }
        Ok(())
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

// Get apps information using Get-StartApps
pub fn get_app_info_from_startapps(app_name: &str) -> Result<(String, String), AutomationError> {
    let command = r#"Get-StartApps | Select-Object Name, AppID | ConvertTo-Json"#.to_string();

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "hidden", "-Command", &command])
        .output()
        .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AutomationError::PlatformError(format!(
            "Failed to get UWP apps list: {error_msg}"
        )));
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let apps: Vec<Value> = serde_json::from_str(&output_str)
        .map_err(|e| AutomationError::PlatformError(format!("Failed to parse apps list: {e}")))?;

    // two parts
    let search_terms: Vec<String> = app_name
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    // Search for matching app by name or AppID
    let matching_app = apps.iter().find(|app| {
        let name = app
            .get("Name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_lowercase();
        let app_id = app
            .get("AppID")
            .and_then(|id| id.as_str())
            .unwrap_or("")
            .to_lowercase();

        // make sure both parts exists
        search_terms
            .iter()
            .all(|term| name.contains(term) || app_id.contains(term))
    });

    match matching_app {
        Some(app) => {
            let display_name = app.get("Name").and_then(|n| n.as_str()).ok_or_else(|| {
                AutomationError::PlatformError("Failed to get app name".to_string())
            })?;
            let app_id = app.get("AppID").and_then(|id| id.as_str()).ok_or_else(|| {
                AutomationError::PlatformError("Failed to get app ID".to_string())
            })?;
            Ok((app_id.to_string(), display_name.to_string()))
        }
        None => Err(AutomationError::PlatformError(format!(
            "No app found matching '{app_name}' in Get-StartApps list"
        ))),
    }
}

// Helper function to get application by PID with fallback to child process and name
fn get_application_pid(
    engine: &WindowsEngine,
    pid: i32,
    app_name: &str,
) -> Result<UIElement, AutomationError> {
    unsafe {
        // Check if the process with this PID exists
        let mut pid_exists = false;
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(handle) => handle,
            Err(_) => {
                debug!(
                    "Failed to create process snapshot for PID existence check, falling back to name"
                );
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
        debug!(
            "Failed to get application by PID and child PID, trying by name: {}",
            app_name
        );
        let app = engine.get_application_by_name(app_name)?;
        app.activate_window()?;
        Ok(app)
    }
}

// launches any windows application returns its UIElement
fn launch_app(
    engine: &WindowsEngine,
    app_id: &str,
    display_name: &str,
) -> Result<UIElement, AutomationError> {
    let pid = unsafe {
        // Initialize COM with proper error handling
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr.is_err() && hr != HRESULT(0x80010106u32 as i32) {
            // Only return error if it's not the "already initialized" case
            return Err(AutomationError::PlatformError(format!(
                "Failed to initialize COM: {hr}"
            )));
        }
        // If we get here, either initialization succeeded or it was already initialized
        if hr == HRESULT(0x80010106u32 as i32) {
            debug!("COM already initialized in this thread");
        }

        // Create the ApplicationActivationManager COM object
        let manager: IApplicationActivationManager =
            CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_ALL).map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to create ApplicationActivationManager: {e}"
                ))
            })?;

        // Set options (e.g., NoSplashScreen)
        let options = ACTIVATEOPTIONS(ActivateOptions::None as i32);

        match manager.ActivateApplication(
            &HSTRING::from(app_id),
            &HSTRING::from(""), // no arguments
            options,
        ) {
            Ok(pid) => pid,
            Err(_) => {
                let shell_app_id: Vec<u16> = format!("shell:AppsFolder\\{app_id}")
                    .encode_utf16()
                    .chain(Some(0))
                    .collect();
                let operation_wide: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
                let mut sei = SHELLEXECUTEINFOW {
                    cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                    fMask: SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS,
                    hwnd: HWND(std::ptr::null_mut()),
                    lpVerb: PCWSTR(operation_wide.as_ptr()),
                    lpFile: PCWSTR::from_raw(shell_app_id.as_ptr()),
                    lpParameters: PCWSTR::null(),
                    lpDirectory: PCWSTR::null(),
                    nShow: SW_SHOWNORMAL.0,
                    hInstApp: HINSTANCE(std::ptr::null_mut()),
                    lpIDList: std::ptr::null_mut(),
                    lpClass: PCWSTR::null(),
                    hkeyClass: HKEY(std::ptr::null_mut()),
                    dwHotKey: 0,
                    Anonymous: Default::default(),
                    hProcess: HANDLE(std::ptr::null_mut()),
                };

                ShellExecuteExW(&mut sei).map_err(|e| {
                    AutomationError::PlatformError(format!(
                        "ShellExecuteExW failed: 
                        '{e}' to launch app '{display_name}':"
                    ))
                })?;

                let process_handle = sei.hProcess;

                if process_handle.is_invalid() {
                    let _ = CloseHandle(process_handle);
                    debug!(
                        "Failed to get pid of launched app: '{:?}' using `ShellExecuteExW`, will get the ui element of by its name ",
                        display_name
                    );
                    return engine.get_application_by_name(display_name);
                }

                let pid = GetProcessId(process_handle);
                let _ = CloseHandle(process_handle); // we can use HandleGuard too

                pid
            }
        }
    };

    if pid > 0 {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        get_application_pid(engine, pid as i32, display_name)
    } else {
        Err(Error::new(
            HRESULT(0x80004005u32 as i32),
            "Failed to launch the application",
        )
        .into())
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
    // OPTIMIZATION: Use a static cache to avoid repeated process enumeration
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    struct ProcessCache {
        processes: HashMap<String, i32>,
        last_updated: Instant,
    }

    static PROCESS_CACHE: Mutex<Option<ProcessCache>> = Mutex::new(None);
    const CACHE_DURATION: Duration = Duration::from_secs(2); // Cache for 2 seconds

    let search_name_lower = name.to_lowercase();

    // Check cache first
    {
        let cache_guard = PROCESS_CACHE.lock().unwrap();
        if let Some(ref cache) = *cache_guard {
            if cache.last_updated.elapsed() < CACHE_DURATION {
                // Cache is still valid, check if we have the process
                for (cached_name, &pid) in &cache.processes {
                    if cached_name.contains(&search_name_lower) {
                        debug!("Found PID {} for '{}' in cache", pid, name);
                        return Some(pid);
                    }
                }
                // If we reach here, process not found in valid cache
                return None;
            }
        }
    }

    // Cache is stale or doesn't exist, refresh it
    debug!("Refreshing process cache for PID lookup");
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

        let mut new_processes = HashMap::new();
        let mut found_pid: Option<i32> = None;

        // Iterate through processes to build cache and find our target
        loop {
            // Convert the process name from wide string to String
            let name_slice = &process_entry.szExeFile;
            let name_len = name_slice
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(name_slice.len());
            let process_name = String::from_utf16_lossy(&name_slice[..name_len]);

            // Remove .exe extension if present for comparison
            let clean_name = process_name
                .strip_suffix(".exe")
                .or_else(|| process_name.strip_suffix(".EXE"))
                .unwrap_or(&process_name);

            let clean_name_lower = clean_name.to_lowercase();
            let pid = process_entry.th32ProcessID as i32;

            // Add to cache
            new_processes.insert(clean_name_lower.clone(), pid);

            // Check if this is our target process
            if found_pid.is_none() && clean_name_lower.contains(&search_name_lower) {
                found_pid = Some(pid);
            }

            // Get the next process
            if Process32NextW(snapshot, &mut process_entry).is_err() {
                break;
            }
        }

        // Update cache
        {
            let mut cache_guard = PROCESS_CACHE.lock().unwrap();
            *cache_guard = Some(ProcessCache {
                processes: new_processes,
                last_updated: Instant::now(),
            });
        }

        if let Some(pid) = found_pid {
            debug!("Found PID {} for '{}' via process enumeration", pid, name);
        }

        found_pid
    }
}

// Add this function before the WindowsUIElement implementation
pub(crate) fn generate_element_id(
    element: &uiautomation::UIElement,
) -> Result<usize, AutomationError> {
    // Attempt to get stable properties first
    let automation_id = element
        .get_automation_id()
        .map(|s| if s.is_empty() { None } else { Some(s) })
        .unwrap_or(None);
    let role = element
        .get_control_type()
        .map(|s| {
            if s == ControlType::Custom {
                None
            } else {
                Some(s)
            }
        })
        .unwrap_or(None);
    let name = element
        .get_name()
        .map(|s| if s.is_empty() { None } else { Some(s) })
        .unwrap_or(None);
    let class_name = element
        .get_classname()
        .map(|s| if s.is_empty() { None } else { Some(s) })
        .unwrap_or(None);

    let mut to_hash = String::new();
    if let Some(id) = automation_id {
        to_hash.push_str(&id);
    }
    if let Some(role) = role {
        to_hash.push_str(&role.to_string());
    }
    if let Some(n) = name {
        to_hash.push_str(&n);
    }
    if let Some(cn) = class_name {
        to_hash.push_str(&cn);
    }

    // If still no stable properties, use bounds as a fallback for more stability
    if to_hash.is_empty() {
        if let Ok(rect) = element.get_bounding_rectangle() {
            to_hash.push_str(&format!(
                "{}:{}:{}:{}",
                rect.get_left(),
                rect.get_top(),
                rect.get_width(),
                rect.get_height()
            ));
        }
    }

    // As a last resort for elements with no stable identifiers, use the object's memory address.
    // This is NOT stable across sessions, but provides a unique ID within a single session.
    if to_hash.is_empty() {
        let element_arc = Arc::new(element.clone());
        let ptr = Arc::as_ptr(&element_arc);
        return Ok(ptr as usize);
    }

    let hash = blake3::hash(to_hash.as_bytes());
    Ok(hash.as_bytes()[0..8]
        .try_into()
        .map(u64::from_le_bytes)
        .unwrap() as usize)
}

// Add this function after the generate_element_id function and before the tests module
/// Converts a raw uiautomation::UIElement to a terminator UIElement
pub fn convert_uiautomation_element_to_terminator(element: uiautomation::UIElement) -> UIElement {
    let arc_ele = ThreadSafeWinUIElement(Arc::new(element));
    UIElement::new(Box::new(WindowsUIElement { element: arc_ele }))
}

// Helper function to create UIAutomation instance with proper COM initialization
fn create_ui_automation_with_com_init() -> Result<UIAutomation, AutomationError> {
    unsafe {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr.is_err() && hr != HRESULT(0x80010106u32 as i32) {
            // Only return error if it's not the "already initialized" case
            return Err(AutomationError::PlatformError(format!(
                "Failed to initialize COM: {hr}"
            )));
        }
    }

    UIAutomation::new_direct().map_err(|e| AutomationError::PlatformError(e.to_string()))
}

fn build_ui_node_tree_configurable(
    element: &UIElement,
    current_depth: usize,
    context: &mut TreeBuildingContext,
) -> Result<crate::UINode, AutomationError> {
    context.increment_element_count();
    context.update_max_depth(current_depth);

    // Yield CPU periodically to prevent freezing while processing everything
    if context.should_yield() {
        thread::sleep(Duration::from_millis(1));
    }

    // Get element attributes with configurable property loading
    let attributes = get_configurable_attributes(element, &context.property_mode);

    let mut children_nodes = Vec::new();

    // Get children with safe strategy
    match get_element_children_safe(element, context) {
        Ok(children_elements) => {
            // Process children in efficient batches
            for batch in children_elements.chunks(context.config.batch_size) {
                for child_element in batch {
                    match build_ui_node_tree_configurable(child_element, current_depth + 1, context)
                    {
                        Ok(child_node) => children_nodes.push(child_node),
                        Err(e) => {
                            debug!(
                                "Failed to process child element: {}. Continuing with next child.",
                                e
                            );
                            context.increment_errors();
                            // Continue processing - we want the full tree
                        }
                    }
                }

                // Small yield between large batches to maintain responsiveness
                if batch.len() == context.config.batch_size
                    && children_elements.len() > context.config.batch_size
                {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }
        Err(e) => {
            debug!(
                "Failed to get children for element: {}. Proceeding with no children.",
                e
            );
            context.increment_errors();
        }
    }

    Ok(crate::UINode {
        id: element.id(),
        attributes,
        children: children_nodes,
    })
}

/// Get element attributes based on the configured property loading mode
fn get_configurable_attributes(
    element: &UIElement,
    property_mode: &crate::platforms::PropertyLoadingMode,
) -> UIElementAttributes {
    let mut attrs = match property_mode {
        crate::platforms::PropertyLoadingMode::Fast => {
            // Only essential properties - current optimized version
            element.attributes()
        }
        crate::platforms::PropertyLoadingMode::Complete => {
            // Get full attributes by temporarily bypassing optimization
            get_complete_attributes(element)
        }
        crate::platforms::PropertyLoadingMode::Smart => {
            // Load properties based on element type
            get_smart_attributes(element)
        }
    };

    // Check if element is keyboard focusable and add bounds if it is
    if let Ok(is_focusable) = element.is_keyboard_focusable() {
        if is_focusable {
            attrs.is_keyboard_focusable = Some(true);
            // Only add bounds for keyboard-focusable elements
            if let Ok(bounds) = element.bounds() {
                attrs.bounds = Some(bounds);
            }
        }
    }

    if let Ok(is_focused) = element.is_focused() {
        if is_focused {
            attrs.is_focused = Some(true);
        }
    }

    if let Ok(text) = element.text(0) {
        if !text.is_empty() {
            attrs.text = Some(text);
        }
    }

    if let Ok(is_enabled) = element.is_enabled() {
        attrs.enabled = Some(is_enabled);
    }

    attrs
}

/// Get complete attributes for an element (all properties)
fn get_complete_attributes(element: &UIElement) -> UIElementAttributes {
    // This would be the original attributes() implementation
    // For now, just use the current optimized one
    // TODO: Implement full property loading when needed
    element.attributes()
}

/// Get smart attributes based on element type
fn get_smart_attributes(element: &UIElement) -> UIElementAttributes {
    let role = element.role();

    // Load different properties based on element type
    match role.as_str() {
        "Button" | "MenuItem" => {
            // For interactive elements, load name and enabled state
            element.attributes()
        }
        "Edit" | "Text" => {
            // For text elements, load value and text content
            element.attributes()
        }
        "Window" | "Dialog" => {
            // For containers, load name and description
            element.attributes()
        }
        _ => {
            // Default to fast loading for unknown types
            element.attributes()
        }
    }
}

fn launch_legacy_app(engine: &WindowsEngine, app_name: &str) -> Result<UIElement, AutomationError> {
    info!("Launching legacy app: {}", app_name);
    unsafe {
        // Convert app_name to wide string
        let mut app_name_wide: Vec<u16> =
            app_name.encode_utf16().chain(std::iter::once(0)).collect();

        // Prepare process startup info
        let startup_info = windows::Win32::System::Threading::STARTUPINFOW {
            cb: std::mem::size_of::<windows::Win32::System::Threading::STARTUPINFOW>() as u32,
            ..Default::default()
        };

        // Prepare process info
        let mut process_info = windows::Win32::System::Threading::PROCESS_INFORMATION::default();

        // Create the process
        let result = windows::Win32::System::Threading::CreateProcessW(
            None, // Application name (null means use command line)
            Some(windows::core::PWSTR::from_raw(app_name_wide.as_mut_ptr())), // Command line
            None, // Process security attributes
            None, // Thread security attributes
            false, // Inherit handles
            windows::Win32::System::Threading::CREATE_NEW_CONSOLE, // Creation flags
            None, // Environment
            None, // Current directory
            &startup_info,
            &mut process_info,
        );

        if result.is_err() {
            return Err(AutomationError::PlatformError(format!(
                "Failed to launch application '{app_name}'"
            )));
        }

        // Close thread handle as we don't need it
        let _ = windows::Win32::Foundation::CloseHandle(process_info.hThread);

        // Store process handle in a guard to ensure it's closed
        let _process_handle = HandleGuard(process_info.hProcess);

        // Get the PID
        let pid = process_info.dwProcessId as i32;

        // Extract process name from process_info (unused variable)
        let process_name = get_process_name_by_pid(pid).unwrap_or_else(|_| app_name.to_string());

        match get_application_pid(engine, pid, app_name) {
            Ok(app) => Ok(app),
            Err(_) => {
                let new_pid = get_pid_by_name(&process_name);
                if new_pid.is_none() {
                    return Err(AutomationError::PlatformError(format!(
                        "Failed to get PID for launched process: {process_name}"
                    )));
                }
                // Try again with the extracted PID
                get_application_pid(engine, new_pid.unwrap(), app_name)
            }
        }
    }
}
