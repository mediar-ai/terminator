use tracing::{debug, instrument};

use crate::platforms::AccessibilityEngine;
use crate::element::UIElement;
use crate::errors::AutomationError;
use crate::selector::Selector;
use std::sync::Arc;
use std::time::Duration;

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
    #[instrument(level = "debug", skip(self, timeout))]
    pub async fn wait(&self, timeout: Option<Duration>) -> Result<UIElement, AutomationError> {
        debug!("Waiting for element matching selector: {:?}", self.selector);
        let effective_timeout = timeout.unwrap_or(self.timeout);
        let start = std::time::Instant::now();

        loop {
            // Calculate remaining time, preventing overflow if already timed out.
            let remaining_time = if start.elapsed() >= effective_timeout {
                Duration::ZERO
            } else {
                effective_timeout - start.elapsed()
            };
            debug!("New wait loop iteration, remaining_time: {:?}", remaining_time);

            // Directly use find_element with the calculated (or zero) remaining timeout
            match self.engine.find_element(
                &self.selector,
                self.root.as_ref(),
                Some(remaining_time), // Pass the safely calculated remaining time
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

}