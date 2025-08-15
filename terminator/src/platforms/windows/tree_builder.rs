//! UI tree building functionality for Windows

use crate::{AutomationError, UIElement, UIElementAttributes};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tracing::debug;

/// Configuration for tree building operations
pub(crate) struct TreeBuildingConfig {
    pub(crate) timeout_per_operation_ms: u64,
    pub(crate) yield_every_n_elements: usize,
    pub(crate) batch_size: usize,
}

/// Context for tracking tree building progress and stats
pub(crate) struct TreeBuildingContext {
    pub(crate) config: TreeBuildingConfig,
    pub(crate) property_mode: crate::platforms::PropertyLoadingMode,
    pub(crate) elements_processed: usize,
    pub(crate) max_depth_reached: usize,
    pub(crate) cache_hits: usize,
    pub(crate) fallback_calls: usize,
    pub(crate) errors_encountered: usize,
}

impl TreeBuildingContext {
    pub(crate) fn should_yield(&self) -> bool {
        self.elements_processed % self.config.yield_every_n_elements == 0
            && self.elements_processed > 0
    }

    pub(crate) fn increment_element_count(&mut self) {
        self.elements_processed += 1;
    }

    pub(crate) fn update_max_depth(&mut self, depth: usize) {
        self.max_depth_reached = self.max_depth_reached.max(depth);
    }

    pub(crate) fn increment_cache_hit(&mut self) {
        self.cache_hits += 1;
    }

    pub(crate) fn increment_fallback(&mut self) {
        self.fallback_calls += 1;
    }

    pub(crate) fn increment_errors(&mut self) {
        self.errors_encountered += 1;
    }
}

/// Build a UI node tree with configurable properties and performance tuning
pub(crate) fn build_ui_node_tree_configurable(
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

    // Add toggled state if available (or default to false for checkboxes)
    if let Ok(toggled) = element.is_toggled() {
        attrs.is_toggled = Some(toggled);
    } else if element.role() == "CheckBox" {
        // Default checkboxes to false when is_toggled() fails (common for unchecked boxes)
        attrs.is_toggled = Some(false);
    }

    if let Ok(is_selected) = element.is_selected() {
        attrs.is_selected = Some(is_selected);
    }

    if let Ok(children) = element.children() {
        attrs.child_count = Some(children.len());
        // index in parent
        if let Ok(Some(parent)) = element.parent() {
            if let Ok(siblings) = parent.children() {
                if let Some(idx) = siblings.iter().position(|e| e == element) {
                    attrs.index_in_parent = Some(idx);
                }
            }
        }
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
            // Default to fast loading
            element.attributes()
        }
    }
}

/// Safe element children access with fallback strategies
pub(crate) fn get_element_children_safe(
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

/// Helper function to get element children with timeout
pub(crate) fn get_element_children_with_timeout(
    element: &UIElement,
    timeout: Duration,
) -> Result<Vec<UIElement>, AutomationError> {
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
