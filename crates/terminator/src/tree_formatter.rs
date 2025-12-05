//! Tree formatting utilities for UI trees
//!
//! Provides compact YAML-like formatting for UI trees with indexed elements
//! for click targeting.

use crate::element::SerializableUIElement;
use crate::OcrElement;
use crate::UINode;
use std::collections::HashMap;

/// Result of UI tree formatting - includes both the formatted string and bounds mapping
#[derive(Debug, Clone)]
pub struct TreeFormattingResult {
    /// The formatted YAML-like string
    pub formatted: String,
    /// Mapping of index to (role, name, bounds, selector) for click targeting
    /// Key is 1-based index, value is (role, name, (x, y, width, height), selector)
    pub index_to_bounds: HashMap<u32, (String, String, (f64, f64, f64, f64), Option<String>)>,
    /// Total count of indexed elements (elements with bounds)
    pub element_count: u32,
}

/// Result of OCR tree formatting - includes both the formatted string and bounds mapping
#[derive(Debug, Clone)]
pub struct OcrFormattingResult {
    /// The formatted YAML-like string
    pub formatted: String,
    /// Mapping of index to (text, bounds) for click targeting
    /// Key is 1-based index, value is (text, (x, y, width, height))
    pub index_to_bounds: HashMap<u32, (String, (f64, f64, f64, f64))>,
}

/// Convert UINode to SerializableUIElement for unified formatting
fn ui_node_to_serializable(node: &UINode) -> SerializableUIElement {
    SerializableUIElement {
        id: node.id.clone(),
        role: node.attributes.role.clone(),
        name: node.attributes.name.clone(),
        bounds: node.attributes.bounds,
        value: node.attributes.value.clone(),
        description: node.attributes.description.clone(),
        window_and_application_name: node.attributes.application_name.clone(),
        window_title: None,
        url: None,
        process_id: None,
        process_name: None,
        children: if node.children.is_empty() {
            None
        } else {
            Some(node.children.iter().map(ui_node_to_serializable).collect())
        },
        label: node.attributes.label.clone(),
        text: None, // UINode doesn't have text field
        is_keyboard_focusable: node.attributes.is_keyboard_focusable,
        is_focused: node.attributes.is_focused,
        is_toggled: node.attributes.is_toggled,
        enabled: node.attributes.enabled,
        is_selected: node.attributes.is_selected,
        child_count: node.attributes.child_count,
        index_in_parent: node.attributes.index_in_parent,
        selector: node.selector.clone(),
    }
}

/// Format a UI tree as compact YAML with #index [ROLE] name format
///
/// Output format:
/// #1 [ROLE] name (bounds: [x,y,w,h], additional context)
///   #2 [ROLE] name (bounds: [x,y,w,h])
///     - ...
///
/// Elements with bounds get a clickable index first. Elements without bounds use dash prefix.
/// Returns both the formatted string and a mapping of index → (role, name, bounds, selector) for click targeting.
pub fn format_tree_as_compact_yaml(
    tree: &SerializableUIElement,
    indent: usize,
) -> TreeFormattingResult {
    let mut output = String::new();
    let mut index_to_bounds = HashMap::new();
    let mut next_index = 1u32;
    format_node(
        tree,
        indent,
        &mut output,
        &mut index_to_bounds,
        &mut next_index,
    );
    TreeFormattingResult {
        formatted: output,
        index_to_bounds,
        element_count: next_index - 1,
    }
}

/// Format a UINode tree as compact YAML by converting to SerializableUIElement first
pub fn format_ui_node_as_compact_yaml(tree: &UINode, indent: usize) -> TreeFormattingResult {
    let serializable = ui_node_to_serializable(tree);
    format_tree_as_compact_yaml(&serializable, indent)
}

fn format_node(
    node: &SerializableUIElement,
    indent: usize,
    output: &mut String,
    index_to_bounds: &mut HashMap<u32, (String, String, (f64, f64, f64, f64), Option<String>)>,
    next_index: &mut u32,
) {
    // Build the indent string
    let indent_str = if indent > 0 {
        "  ".repeat(indent)
    } else {
        String::new()
    };

    // Add indent
    output.push_str(&indent_str);

    // Add index first if element has bounds (clickable), otherwise dash prefix
    if let Some((x, y, w, h)) = node.bounds {
        let idx = *next_index;
        *next_index += 1;
        output.push_str(&format!("#{idx} "));

        // Format: [ROLE]
        output.push_str(&format!("[{}]", node.role));

        // Store in cache: index → (role, name, bounds, selector)
        let name = node.name.clone().unwrap_or_default();
        index_to_bounds.insert(
            idx,
            (node.role.clone(), name, (x, y, w, h), node.selector.clone()),
        );
    } else {
        // No bounds - use dash prefix and [ROLE]
        output.push_str(&format!("- [{}]", node.role));
    }

    // Add name if present
    if let Some(ref name) = node.name {
        if !name.is_empty() {
            output.push_str(&format!(" {name}"));
        }
    }

    // Add additional context in parentheses
    let mut context_parts = Vec::new();

    // Add text if present (for hyperlinks)
    if let Some(ref text) = node.text {
        if !text.is_empty() {
            context_parts.push(format!("text: {text}"));
        }
    }

    // Add bounds if present
    if let Some((x, y, w, h)) = node.bounds {
        context_parts.push(format!("bounds: [{x:.0},{y:.0},{w:.0},{h:.0}]"));
    }

    // Add state flags
    if let Some(enabled) = node.enabled {
        if !enabled {
            context_parts.push("disabled".to_string());
        }
    }

    if node.is_focused == Some(true) {
        context_parts.push("focused".to_string());
    }

    if node.is_keyboard_focusable == Some(true) {
        context_parts.push("focusable".to_string());
    }

    if node.is_selected == Some(true) {
        context_parts.push("selected".to_string());
    }

    if node.is_toggled == Some(true) {
        context_parts.push("toggled".to_string());
    }

    // Add value if present
    if let Some(ref value) = node.value {
        if !value.is_empty() {
            context_parts.push(format!("value: {value}"));
        }
    }

    // Add child count if no children array but count is known
    if node.children.is_none() {
        if let Some(count) = node.child_count {
            if count > 0 {
                context_parts.push(format!("{count} children"));
            }
        }
    }

    // Add context if any parts exist
    if !context_parts.is_empty() {
        output.push_str(&format!(" ({})", context_parts.join(", ")));
    }

    output.push('\n');

    // Recursively format children
    if let Some(ref children) = node.children {
        for child in children {
            format_node(child, indent + 1, output, index_to_bounds, next_index);
        }
    }
}

/// Format an OCR tree as compact YAML with indexed words for click targeting
///
/// Output format:
/// - [OcrResult] (text_angle: 0.0)
///   - [OcrLine] "Line of text"
///     #1 [OcrWord] "word" (bounds: [x,y,w,h])
///     #2 [OcrWord] "another" (bounds: [x,y,w,h])
///
/// Returns both the formatted string and a mapping of index → bounds for click targeting
pub fn format_ocr_tree_as_compact_yaml(tree: &OcrElement, indent: usize) -> OcrFormattingResult {
    let mut output = String::new();
    let mut index_to_bounds = HashMap::new();
    let mut next_index = 1u32;
    format_ocr_node(
        tree,
        indent,
        &mut output,
        &mut index_to_bounds,
        &mut next_index,
    );
    OcrFormattingResult {
        formatted: output,
        index_to_bounds,
    }
}

fn format_ocr_node(
    node: &OcrElement,
    indent: usize,
    output: &mut String,
    index_to_bounds: &mut HashMap<u32, (String, (f64, f64, f64, f64))>,
    next_index: &mut u32,
) {
    // Build the indent string
    let indent_str = if indent > 0 {
        "  ".repeat(indent)
    } else {
        String::new()
    };

    // Add indent
    output.push_str(&indent_str);

    // For OcrWord, add index first, otherwise dash prefix
    let current_index = if node.role == "OcrWord" {
        let idx = *next_index;
        *next_index += 1;
        output.push_str(&format!("#{idx} [{role}]", role = node.role));
        Some(idx)
    } else {
        output.push_str(&format!("- [{}]", node.role));
        None
    };

    // Add text if present
    if let Some(ref text) = node.text {
        if !text.is_empty() {
            // For words and lines, show the text in quotes
            if node.role == "OcrWord" || node.role == "OcrLine" {
                output.push_str(&format!(" \"{text}\""));
            }
        }
    }

    // Add additional context in parentheses
    let mut context_parts = Vec::new();

    // Add bounds if present (absolute screen coordinates for clicking)
    if let Some((x, y, w, h)) = node.bounds {
        context_parts.push(format!("bounds: [{x:.0},{y:.0},{w:.0},{h:.0}]"));

        // Store bounds for OcrWord elements
        if let Some(idx) = current_index {
            let text = node.text.clone().unwrap_or_default();
            index_to_bounds.insert(idx, (text, (x, y, w, h)));
        }
    }

    // Add text angle for OcrResult
    if let Some(angle) = node.text_angle {
        context_parts.push(format!("text_angle: {angle:.1}"));
    }

    // Add confidence if present
    if let Some(confidence) = node.confidence {
        context_parts.push(format!("confidence: {confidence:.2}"));
    }

    // Add context if any parts exist
    if !context_parts.is_empty() {
        output.push_str(&format!(" ({})", context_parts.join(", ")));
    }

    output.push('\n');

    // Recursively format children
    if let Some(ref children) = node.children {
        for child in children {
            format_ocr_node(child, indent + 1, output, index_to_bounds, next_index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_formatting() {
        let node = SerializableUIElement {
            id: Some("123".to_string()),
            role: "Button".to_string(),
            name: Some("Submit".to_string()),
            bounds: Some((10.0, 20.0, 100.0, 50.0)),
            value: None,
            description: None,
            window_and_application_name: None,
            window_title: None,
            url: None,
            process_id: None,
            process_name: None,
            children: None,
            label: None,
            text: None,
            is_keyboard_focusable: Some(true),
            is_focused: None,
            is_toggled: None,
            enabled: Some(true),
            is_selected: None,
            child_count: None,
            index_in_parent: None,
            selector: None,
        };

        let result = format_tree_as_compact_yaml(&node, 0);
        assert!(result.formatted.contains("#1 [Button] Submit"));
        assert!(result.formatted.contains("bounds: [10,20,100,50]"));
        assert!(result.formatted.contains("focusable"));
        assert_eq!(result.element_count, 1);
        assert!(result.index_to_bounds.contains_key(&1));
    }

    #[test]
    fn test_nested_formatting() {
        let child = SerializableUIElement {
            id: Some("456".to_string()),
            role: "Text".to_string(),
            name: Some("Label".to_string()),
            bounds: None,
            value: None,
            description: None,
            window_and_application_name: None,
            window_title: None,
            url: None,
            process_id: None,
            process_name: None,
            children: None,
            label: None,
            text: None,
            is_keyboard_focusable: None,
            is_focused: None,
            is_toggled: None,
            enabled: None,
            is_selected: None,
            child_count: None,
            index_in_parent: None,
            selector: None,
        };

        let parent = SerializableUIElement {
            id: Some("123".to_string()),
            role: "Window".to_string(),
            name: Some("Main".to_string()),
            bounds: None,
            value: None,
            description: None,
            window_and_application_name: None,
            window_title: None,
            url: None,
            process_id: None,
            process_name: None,
            children: Some(vec![child]),
            label: None,
            text: None,
            is_keyboard_focusable: None,
            is_focused: None,
            is_toggled: None,
            enabled: None,
            is_selected: None,
            child_count: None,
            index_in_parent: None,
            selector: None,
        };

        let result = format_tree_as_compact_yaml(&parent, 0);
        assert!(result.formatted.contains("- [Window] Main"));
        assert!(result.formatted.contains("  - [Text] Label"));
        assert_eq!(result.element_count, 0); // No elements with bounds
    }

    #[test]
    fn test_mixed_bounds_formatting() {
        let child_with_bounds = SerializableUIElement {
            id: None,
            role: "Button".to_string(),
            name: Some("Click Me".to_string()),
            bounds: Some((100.0, 200.0, 80.0, 30.0)),
            value: None,
            description: None,
            window_and_application_name: None,
            window_title: None,
            url: None,
            process_id: None,
            process_name: None,
            children: None,
            label: None,
            text: None,
            is_keyboard_focusable: None,
            is_focused: None,
            is_toggled: None,
            enabled: None,
            is_selected: None,
            child_count: None,
            index_in_parent: None,
            selector: Some("role:Button && name:Click Me".to_string()),
        };

        let child_no_bounds = SerializableUIElement {
            id: None,
            role: "Text".to_string(),
            name: Some("Label".to_string()),
            bounds: None,
            value: None,
            description: None,
            window_and_application_name: None,
            window_title: None,
            url: None,
            process_id: None,
            process_name: None,
            children: None,
            label: None,
            text: None,
            is_keyboard_focusable: None,
            is_focused: None,
            is_toggled: None,
            enabled: None,
            is_selected: None,
            child_count: None,
            index_in_parent: None,
            selector: None,
        };

        let parent = SerializableUIElement {
            id: None,
            role: "Window".to_string(),
            name: Some("Test".to_string()),
            bounds: Some((0.0, 0.0, 800.0, 600.0)),
            value: None,
            description: None,
            window_and_application_name: None,
            window_title: None,
            url: None,
            process_id: None,
            process_name: None,
            children: Some(vec![child_with_bounds, child_no_bounds]),
            label: None,
            text: None,
            is_keyboard_focusable: None,
            is_focused: None,
            is_toggled: None,
            enabled: None,
            is_selected: None,
            child_count: None,
            index_in_parent: None,
            selector: None,
        };

        let result = format_tree_as_compact_yaml(&parent, 0);
        assert!(result.formatted.contains("#1 [Window] Test"));
        assert!(result.formatted.contains("#2 [Button] Click Me"));
        assert!(result.formatted.contains("- [Text] Label")); // No index for no-bounds
        assert_eq!(result.element_count, 2);

        // Check index_to_bounds has selector
        let button_entry = result.index_to_bounds.get(&2).unwrap();
        assert_eq!(button_entry.3, Some("role:Button && name:Click Me".to_string()));
    }
}
