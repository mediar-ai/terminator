use crate::omniparser::OmniparserItem;
use std::collections::HashMap;
use terminator::element::SerializableUIElement;
use terminator::OcrElement;
use terminator::UINode;

/// Convert UINode to SerializableUIElement for unified formatting
fn ui_node_to_serializable(node: &UINode) -> SerializableUIElement {
    SerializableUIElement {
        id: node.id.clone(),
        role: node.attributes.role.clone(),
        name: node.attributes.name.clone(),
        bounds: node.attributes.bounds,
        value: node.attributes.value.clone(),
        description: node.attributes.description.clone(),
        window_and_application_name: None,
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
    }
}

/// Format a UI tree as compact YAML with - [ROLE] #index name format
///
/// Output format:
/// - [ROLE] #1 name (bounds: [x,y,w,h], additional context)
///   - [ROLE] #2 name (bounds: [x,y,w,h])
///     - ...
///
/// Elements with bounds get a clickable index. Elements without bounds are shown without index.
/// Returns both the formatted string and a mapping of index → (role, name, bounds) for click_index.
pub fn format_tree_as_compact_yaml(tree: &SerializableUIElement, indent: usize) -> UiaFormattingResult {
    let mut output = String::new();
    let mut index_to_bounds = HashMap::new();
    let mut next_index = 1u32;
    format_node(tree, indent, &mut output, &mut index_to_bounds, &mut next_index);
    UiaFormattingResult {
        formatted: output,
        index_to_bounds,
    }
}

/// Format a UINode tree as compact YAML by converting to SerializableUIElement first
pub fn format_ui_node_as_compact_yaml(tree: &UINode, indent: usize) -> UiaFormattingResult {
    let serializable = ui_node_to_serializable(tree);
    format_tree_as_compact_yaml(&serializable, indent)
}

fn format_node(
    node: &SerializableUIElement,
    indent: usize,
    output: &mut String,
    index_to_bounds: &mut HashMap<u32, (String, String, (f64, f64, f64, f64))>,
    next_index: &mut u32,
) {
    // Build the indent string
    let indent_str = if indent > 0 {
        "  ".repeat(indent)
    } else {
        String::new()
    };

    // Add indent and dash prefix
    output.push_str(&indent_str);
    output.push_str("- ");

    // Format: [ROLE]
    output.push_str(&format!("[{}]", node.role));

    // Add index if element has bounds (clickable)
    if let Some((x, y, w, h)) = node.bounds {
        let idx = *next_index;
        *next_index += 1;
        output.push_str(&format!(" #{idx}"));

        // Store in cache: index → (role, name, bounds)
        let name = node.name.clone().unwrap_or_default();
        index_to_bounds.insert(idx, (node.role.clone(), name, (x, y, w, h)));
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

/// Result of UIA tree formatting - includes both the formatted string and bounds mapping
#[derive(Debug, Clone)]
pub struct UiaFormattingResult {
    /// The formatted YAML string
    pub formatted: String,
    /// Mapping of index to (role, name, bounds) for click targeting
    /// Key is 1-based index, value is (role, name, (x, y, width, height))
    pub index_to_bounds: HashMap<u32, (String, String, (f64, f64, f64, f64))>,
}

/// Result of OCR tree formatting - includes both the formatted string and bounds mapping
#[derive(Debug, Clone)]
pub struct OcrFormattingResult {
    /// The formatted YAML string
    pub formatted: String,
    /// Mapping of index to (text, bounds) for click targeting
    /// Key is 1-based index, value is (text, (x, y, width, height))
    pub index_to_bounds: std::collections::HashMap<u32, (String, (f64, f64, f64, f64))>,
}

/// Result of browser DOM formatting - includes both the formatted string and bounds mapping
#[derive(Debug, Clone)]
pub struct DomFormattingResult {
    /// The formatted YAML string
    pub formatted: String,
    /// Mapping of index to (tag, identifier, bounds) for click targeting
    /// Key is 1-based index, value is (tag, id_or_classes, (x, y, width, height))
    /// Bounds are viewport-relative and need window offset added for screen coords
    pub index_to_bounds: HashMap<u32, (String, String, (f64, f64, f64, f64))>,
}

/// Format an OCR tree as compact YAML with indexed words for click targeting
///
/// Output format:
/// - [OcrResult] (text_angle: 0.0)
///   - [OcrLine] "Line of text"
///     - [OcrWord] #1 "word" (bounds: [x,y,w,h])
///     - [OcrWord] #2 "another" (bounds: [x,y,w,h])
///
/// Returns both the formatted string and a mapping of index → bounds for click_cv_index with vision_type='ocr'
pub fn format_ocr_tree_as_compact_yaml(tree: &OcrElement, indent: usize) -> OcrFormattingResult {
    let mut output = String::new();
    let mut index_to_bounds = std::collections::HashMap::new();
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
    index_to_bounds: &mut std::collections::HashMap<u32, (String, (f64, f64, f64, f64))>,
    next_index: &mut u32,
) {
    // Build the indent string
    let indent_str = if indent > 0 {
        "  ".repeat(indent)
    } else {
        String::new()
    };

    // Add indent and dash prefix
    output.push_str(&indent_str);
    output.push_str("- ");

    // Format: [ROLE] "text"
    output.push_str(&format!("[{}]", node.role));

    // For OcrWord, add index number before the text
    let current_index = if node.role == "OcrWord" {
        let idx = *next_index;
        *next_index += 1;
        output.push_str(&format!(" #{idx}"));
        Some(idx)
    } else {
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

/// Format omniparser items as compact YAML with - [label] #index "content" format
///
/// Output format:
/// - [text] #1 "detected text" (bounds: [x,y,w,h])
/// - [icon] #2 "icon description" (bounds: [x,y,w,h])
///
/// Returns tuple of (formatted string, cache for click_cv_index)
pub fn format_omniparser_tree_as_compact_yaml(
    items: &[OmniparserItem],
) -> (String, HashMap<u32, OmniparserItem>) {
    let mut output = String::new();
    let mut cache = HashMap::new();

    for (i, item) in items.iter().enumerate() {
        let index = (i + 1) as u32;
        cache.insert(index, item.clone());

        // Format: - [label] #index "content" (bounds: [x,y,w,h])
        output.push_str(&format!("- [{}] #{}", item.label, index));

        if let Some(ref content) = item.content {
            if !content.is_empty() {
                output.push_str(&format!(" \"{}\"", content));
            }
        }

        if let Some(box_2d) = item.box_2d {
            // Convert [x_min, y_min, x_max, y_max] to [x, y, width, height]
            let w = box_2d[2] - box_2d[0];
            let h = box_2d[3] - box_2d[1];
            output.push_str(&format!(
                " (bounds: [{:.0},{:.0},{:.0},{:.0}])",
                box_2d[0], box_2d[1], w, h
            ));
        }

        output.push('\n');
    }

    (output, cache)
}

/// Format browser DOM elements as compact YAML with indexed elements for click targeting
///
/// Output format:
/// - [tag] #1 [.class1.class2] name #element_id (bounds: [x,y,w,h])
///
/// Name is resolved from: text → aria_label → value → placeholder
/// Null/empty attributes are omitted
/// Returns both the formatted string and a mapping of index → bounds for click_index with vision_type='dom'
pub fn format_browser_dom_as_compact_yaml(elements: &[serde_json::Value]) -> DomFormattingResult {
    let mut output = String::new();
    let mut index_to_bounds = HashMap::new();
    let mut next_index = 1u32;

    for elem in elements {
        // Get tag (required)
        let tag = elem
            .get("tag")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // Get bounds first - only include elements with valid bounds
        let x = elem.get("x").and_then(|v| v.as_f64());
        let y = elem.get("y").and_then(|v| v.as_f64());
        let w = elem.get("width").and_then(|v| v.as_f64());
        let h = elem.get("height").and_then(|v| v.as_f64());

        let has_bounds = matches!((x, y, w, h), (Some(_), Some(_), Some(w), Some(h)) if w > 0.0 && h > 0.0);

        output.push_str(&format!("- [{}]", tag));

        // Add index if element has valid bounds
        if has_bounds {
            output.push_str(&format!(" #{}", next_index));
        }

        // Get classes if any
        let classes_str = if let Some(classes) = elem.get("classes").and_then(|v| v.as_array()) {
            let class_strs: Vec<&str> = classes
                .iter()
                .filter_map(|c| c.as_str())
                .filter(|s| !s.is_empty())
                .collect();
            if !class_strs.is_empty() {
                let classes_joined = class_strs.join(".");
                output.push_str(&format!(" [.{}]", classes_joined));
                Some(classes_joined)
            } else {
                None
            }
        } else {
            None
        };

        // Get name: text → aria_label → value → placeholder
        let name = elem
            .get("text")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                elem.get("aria_label")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
            })
            .or_else(|| {
                elem.get("value")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
            })
            .or_else(|| {
                elem.get("placeholder")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
            });

        if let Some(name) = name {
            // Truncate long names and escape newlines
            let clean_name = name.replace('\n', " ").replace('\r', "");
            let truncated = if clean_name.len() > 60 {
                format!("{}...", &clean_name[..57])
            } else {
                clean_name
            };
            output.push_str(&format!(" {}", truncated));
        }

        // Add element id if present
        let elem_id = elem.get("id").and_then(|v| v.as_str()).filter(|s| !s.is_empty());
        if let Some(id) = elem_id {
            output.push_str(&format!(" #{}", id));
        }

        // Add bounds and store in cache
        if let (Some(x), Some(y), Some(w), Some(h)) = (x, y, w, h) {
            if w > 0.0 && h > 0.0 {
                output.push_str(&format!(" (bounds: [{},{},{},{}])", x as i64, y as i64, w as i64, h as i64));

                // Build identifier for overlay: prefer text content, then id, then classes, then empty
                let identifier = name
                    .map(|s| {
                        // Truncate for overlay display
                        let clean = s.replace('\n', " ").replace('\r', "");
                        if clean.len() > 30 { format!("{}...", &clean[..27]) } else { clean }
                    })
                    .or_else(|| elem_id.map(|s| s.to_string()))
                    .or(classes_str.clone())
                    .unwrap_or_default();

                // Store bounds in cache (viewport-relative)
                index_to_bounds.insert(next_index, (tag.to_string(), identifier, (x, y, w, h)));
                next_index += 1;
            }
        }

        output.push('\n');
    }

    DomFormattingResult {
        formatted: output,
        index_to_bounds,
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
        };

        let result = format_tree_as_compact_yaml(&node, 0);
        assert!(result.contains("[Button] Submit"));
        assert!(!result.contains("#123")); // IDs should not appear in compact view
        assert!(result.contains("bounds: [10,20,100,50]"));
        assert!(result.contains("focusable"));
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
        };

        let result = format_tree_as_compact_yaml(&parent, 0);
        assert!(result.contains("- [Window] Main"));
        assert!(result.contains("  - [Text] Label"));
        assert!(!result.contains("#123")); // IDs should not appear in compact view
        assert!(!result.contains("#456")); // IDs should not appear in compact view
    }
}
