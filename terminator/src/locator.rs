use crate::platforms::AccessibilityEngine;
use crate::element::{ExploreResponse, UIElement, ExploredElementDetail};
use crate::ScreenshotResult;
use crate::errors::AutomationError;
use crate::selector::Selector;
use crate::UIElementAttributes;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::ClickResult;

// Default timeout if none is specified on the locator itself
const DEFAULT_LOCATOR_TIMEOUT: Duration = Duration::from_secs(30);

/// A high-level API for finding and interacting with UI elements
#[derive(Clone)]
pub struct Locator {
    engine: Arc<dyn AccessibilityEngine>,
    selector: Selector,
    timeout: Duration, // Default timeout for this locator instance
    root: Option<UIElement>,
}

impl Locator {
    /// Create a new locator with the given selector
    pub(crate) fn new(engine: Arc<dyn AccessibilityEngine>, selector: Selector) -> Self {
        Self {
            engine,
            selector,
            timeout: DEFAULT_LOCATOR_TIMEOUT, // Use default
            root: None,
        }
    }

    /// Set a default timeout for waiting operations on this locator instance.
    /// This timeout is used if no specific timeout is passed to action/wait methods.
    pub fn set_default_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the root element for this locator
    pub fn within(mut self, element: UIElement) -> Self {
        self.root = Some(element);
        self
    }

    /// Get all elements matching this locator, waiting up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn all(&self, timeout: Option<Duration>, depth: Option<usize>) -> Result<Vec<UIElement>, AutomationError> {
        let effective_timeout = timeout.unwrap_or(self.timeout);
        // find_elements itself handles the timeout now
        self.engine
            .find_elements(&self.selector, self.root.as_ref(), Some(effective_timeout), depth)
    }

    pub async fn first(&self, timeout: Option<Duration>) -> Result<UIElement, AutomationError> {
        let element = self.wait(timeout).await?;
        Ok(element)
    }

    /// Wait for an element matching the locator to appear, up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn wait(&self, timeout: Option<Duration>) -> Result<UIElement, AutomationError> {
        let effective_timeout = timeout.unwrap_or(self.timeout);
        let start = std::time::Instant::now();

        loop {
            // Directly use find_element with the timeout
            match self.engine.find_element(
                &self.selector,
                self.root.as_ref(),
                Some(effective_timeout - start.elapsed()), // Pass remaining time
            ) {
                Ok(element) => return Ok(element),
                Err(AutomationError::ElementNotFound(_)) => {
                    // Continue looping if not found yet
                    if start.elapsed() >= effective_timeout {
                         // Use the original error message format if possible, or create a new one
                         return Err(AutomationError::Timeout(format!(
                            "Timed out after {:?} waiting for element {:?}",
                            effective_timeout, self.selector
                        )));
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await; // Small delay before retry
                }
                 // Propagate other errors immediately
                 Err(e) => return Err(e),
            }
            // Redundant check, loop condition handles timeout
            // if start.elapsed() >= effective_timeout { ... }
        }
    }

    /// Get a nested locator
    pub fn locator(&self, selector: impl Into<Selector>) -> Locator {
        let next_selector = selector.into();
        let new_chain = match self.selector.clone() {
             // If the current selector is already a chain, append to it
             Selector::Chain(mut existing_chain) => {
                 existing_chain.push(next_selector);
                 existing_chain
             }
             // If the current selector is not a chain, create a new chain
             current_selector => {
                 vec![current_selector, next_selector]
             }
         };

        Locator {
            engine: self.engine.clone(),
            selector: Selector::Chain(new_chain), // Create the chain variant
            timeout: self.timeout, // Inherit timeout
            root: self.root.clone(), // Inherit root
        }
    }
    
    /// Explore the first matching element and its direct children
    pub async fn explore(&self, timeout: Option<Duration>) -> Result<ExploreResponse, AutomationError> {
        let element = self.wait(timeout).await?;
        let mut children = Vec::new();
        for child in element.children()? {
            children.push(ExploredElementDetail::from_element(&child, element.id())?);
        }
        Ok(ExploreResponse {
            parent: element,
            children,
        })
    }

    // --- Convenience methods for common actions ---
    // These now accept an optional timeout

    /// Click on the first matching element, waiting up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn click(&self, timeout: Option<Duration>) -> Result<ClickResult, AutomationError> {
        let element = self.wait(timeout).await?;
        element.click()
    }

    /// Double click on the first matching element, waiting up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn double_click(&self, timeout: Option<Duration>) -> Result<ClickResult, AutomationError> {
        let element = self.wait(timeout).await?;
        element.double_click()
    }

    /// Right click on the first matching element, waiting up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn right_click(&self, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.right_click()
    }

    /// Hover over the first matching element, waiting up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn hover(&self, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.hover()
    }

    /// Type text into the first matching element, waiting up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn type_text(&self, text: &str, use_clipboard: bool, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.type_text(text, use_clipboard)
    }

    /// Press a key on the first matching element, waiting up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn press_key(&self, key: &str, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.press_key(key)
    }

    /// Get text from the first matching element, waiting up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn text(&self, max_depth: usize, timeout: Option<Duration>) -> Result<String, AutomationError> {
        let element = self.wait(timeout).await?;
        element.text(max_depth)
    }

    /// Get attributes from the first matching element, waiting up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn attributes(&self, timeout: Option<Duration>) -> Result<UIElementAttributes, AutomationError> {
        let element = self.wait(timeout).await?;
        Ok(element.attributes()) // attributes() itself doesn't return Result
    }

    /// Get bounds from the first matching element, waiting up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn bounds(&self, timeout: Option<Duration>) -> Result<(f64, f64, f64, f64), AutomationError> {
        let element = self.wait(timeout).await?;
        element.bounds()
    }

    /// Check if the element is visible, waiting up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn is_visible(&self, timeout: Option<Duration>) -> Result<bool, AutomationError> {
        // Wait might return ElementNotFound or Timeout, handle appropriately
        match self.wait(timeout).await {
            Ok(element) => element.is_visible(),
            Err(AutomationError::Timeout(_)) | Err(AutomationError::ElementNotFound(_)) => {
                 // If the element wasn't found within the timeout, it's not visible
                 Ok(false)
            }
            Err(e) => Err(e), // Propagate other errors
        }
    }

    // --- Expectation Methods ---
    // These already handle timeouts internally via loops

    /// Waits for the element matched by the locator to be enabled.
    pub async fn expect_enabled(&self, timeout: Option<Duration>) -> Result<UIElement, AutomationError> {
        let effective_timeout = timeout.unwrap_or(self.timeout);
        let start = Instant::now();

        loop {
            // Use self.wait with a short internal timeout for each check? Or direct find_element?
            // Using find_element directly within the loop is more efficient here.
            match self.engine.find_element(&self.selector, self.root.as_ref(), Some(Duration::from_millis(100))) { // Short timeout for check
                Ok(element) => {
                    match element.is_enabled() {
                        Ok(true) => return Ok(element),
                        Ok(false) => { /* Condition not met, continue loop */ }
                        Err(e) => { /* Error checking enabled state, maybe retry or fail */
                             if !matches!(e, AutomationError::ElementNotFound(_)) { // Ignore not found during check
                                return Err(e); // Return other errors from is_enabled
                             }
                        }
                    }
                }
                Err(AutomationError::ElementNotFound(_)) => { /* Element not found yet, continue loop */ }
                Err(e) => return Err(e), // Error finding the element
            }

            if start.elapsed() >= effective_timeout {
                return Err(AutomationError::Timeout(format!(
                    "Timed out after {:?} waiting for element {:?} to be enabled",
                    effective_timeout, self.selector
                )));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Waits for the element matched by the locator to be visible.
    pub async fn expect_visible(&self, timeout: Option<Duration>) -> Result<UIElement, AutomationError> {
        let effective_timeout = timeout.unwrap_or(self.timeout);
        let start = Instant::now();

        loop {
             // Use find_element directly
             match self.engine.find_element(&self.selector, self.root.as_ref(), Some(Duration::from_millis(100))) {
                Ok(element) => {
                    match element.is_visible() {
                        Ok(true) => return Ok(element),
                        Ok(false) => { /* Condition not met, continue loop */ }
                        Err(e) => {
                             if !matches!(e, AutomationError::ElementNotFound(_)) {
                                return Err(e);
                             }
                        }
                    }
                }
                Err(AutomationError::ElementNotFound(_)) => { /* Element not found yet, continue loop */ }
                Err(e) => return Err(e),
            }

            if start.elapsed() >= effective_timeout {
                return Err(AutomationError::Timeout(format!(
                    "Timed out after {:?} waiting for element {:?} to be visible",
                    effective_timeout, self.selector
                )));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Waits for the element's text content to match the expected text.
    pub async fn expect_text_equals(&self, expected_text: &str, max_depth: usize, timeout: Option<Duration>) -> Result<UIElement, AutomationError> {
        let effective_timeout = timeout.unwrap_or(self.timeout);
        let start = Instant::now();

        loop {
             // Use find_element directly
             match self.engine.find_element(&self.selector, self.root.as_ref(), Some(Duration::from_millis(100))) {
                Ok(element) => {
                    match element.text(max_depth) {
                        // Trim both actual and expected for comparison robustness
                        Ok(actual_text) if actual_text.trim() == expected_text.trim() => return Ok(element),
                        Ok(_) => { /* Text doesn't match, continue loop */ }
                        Err(e) => {
                             if !matches!(e, AutomationError::ElementNotFound(_)) {
                                return Err(e);
                             }
                        }
                    }
                }
                Err(AutomationError::ElementNotFound(_)) => { /* Element not found yet, continue loop */ }
                Err(e) => return Err(e),
            }

            if start.elapsed() >= effective_timeout {
                 // Include actual text in timeout message if possible? Might require getting text one last time.
                return Err(AutomationError::Timeout(format!(
                    "Timed out after {:?} waiting for element {:?} text to equal '{}'",
                    effective_timeout, self.selector, expected_text
                )));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Get the id of the first matching element
    pub async fn id(&self, timeout: Option<Duration>) -> Result<Option<String>, AutomationError> {
        let element = self.wait(timeout).await?;
        Ok(element.id())
    }

    /// Get the role of the first matching element
    pub async fn role(&self, timeout: Option<Duration>) -> Result<String, AutomationError> {
        let element = self.wait(timeout).await?;
        Ok(element.role())
    }

    /// Get the children of the first matching element
    pub async fn children(&self, timeout: Option<Duration>) -> Result<Vec<UIElement>, AutomationError> {
        let element = self.wait(timeout).await?;
        element.children()
    }

    /// Get the parent of the first matching element
    pub async fn parent(&self, timeout: Option<Duration>) -> Result<Option<UIElement>, AutomationError> {
        let element = self.wait(timeout).await?;
        element.parent()
    }

    /// Set value of the first matching element
    pub async fn set_value(&self, value: &str, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.set_value(value)
    }

    /// Check if the first matching element is focused
    pub async fn is_focused(&self, timeout: Option<Duration>) -> Result<bool, AutomationError> {
        let element = self.wait(timeout).await?;
        element.is_focused()
    }

    /// Perform a named action on the first matching element
    pub async fn perform_action(&self, action: &str, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.perform_action(action)
    }

    /// Scroll the first matching element in a given direction
    pub async fn scroll(&self, direction: &str, amount: f64, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.scroll(direction, amount)
    }

    /// Activate the window containing the first matching element
    pub async fn activate_window(&self, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.activate_window()
    }

    /// Get the name of the first matching element
    pub async fn name(&self, timeout: Option<Duration>) -> Result<Option<String>, AutomationError> {
        let element = self.wait(timeout).await?;
        Ok(element.name())
    }

    /// Check if the first matching element is keyboard focusable
    pub async fn is_keyboard_focusable(&self, timeout: Option<Duration>) -> Result<bool, AutomationError> {
        let element = self.wait(timeout).await?;
        element.is_keyboard_focusable()
    }

    /// Drag mouse from start to end coordinates on the first matching element
    pub async fn mouse_drag(&self, start_x: f64, start_y: f64, end_x: f64, end_y: f64, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.mouse_drag(start_x, start_y, end_x, end_y)
    }

    /// Press and hold mouse at (x, y) on the first matching element
    pub async fn mouse_click_and_hold(&self, x: f64, y: f64, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.mouse_click_and_hold(x, y)
    }

    /// Move mouse to (x, y) on the first matching element
    pub async fn mouse_move(&self, x: f64, y: f64, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.mouse_move(x, y)
    }

    /// Release mouse button on the first matching element
    pub async fn mouse_release(&self, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.mouse_release()
    }

    /// Get the containing application element of the first matching element
    pub async fn application(&self, timeout: Option<Duration>) -> Result<Option<UIElement>, AutomationError> {
        let element = self.wait(timeout).await?;
        element.application()
    }

    /// Get the containing window element of the first matching element
    pub async fn window(&self, timeout: Option<Duration>) -> Result<Option<UIElement>, AutomationError> {
        let element = self.wait(timeout).await?;
        element.window()
    }

    /// Highlights the first matching element with a colored border.
    /// 
    /// # Arguments
    /// * `color` - Optional BGR color code (32-bit integer). Default: 0x0000FF (red)
    /// * `duration` - Optional duration for the highlight.
    /// * `timeout` - Optional timeout for finding the element.
    pub async fn highlight(&self, color: Option<u32>, duration: Option<Duration>, timeout: Option<Duration>) -> Result<(), AutomationError> {
        let element = self.wait(timeout).await?;
        element.highlight(color, duration)
    }

    /// Capture a screenshot of the first matching element
    pub async fn capture(&self, timeout: Option<Duration>) -> Result<ScreenshotResult, AutomationError> {
        let element = self.wait(timeout).await?;
        element.capture()
    }

    /// Get the process ID of the application containing the first matching element
    pub async fn process_id(&self, timeout: Option<Duration>) -> Result<u32, AutomationError> {
        let element = self.wait(timeout).await?;
        element.process_id()
    }

}
