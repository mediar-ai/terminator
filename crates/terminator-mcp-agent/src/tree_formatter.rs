use crate::omniparser::OmniparserItem;
use crate::vision::VisionElement;
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
        selector: node.selector.clone(), // Pass through the chained selector from tree building
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
/// Returns both the formatted string and a mapping of index → (role, name, bounds) for click_index.
pub fn format_tree_as_compact_yaml(
    tree: &SerializableUIElement,
    indent: usize,
) -> UiaFormattingResult {
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

/// Result of UIA tree formatting - includes both the formatted string and bounds mapping
#[derive(Debug, Clone)]
pub struct UiaFormattingResult {
    /// The formatted YAML string
    pub formatted: String,
    /// Mapping of index to (role, name, bounds, selector) for click targeting
    /// Key is 1-based index, value is (role, name, (x, y, width, height), selector)
    pub index_to_bounds: HashMap<u32, (String, String, (f64, f64, f64, f64), Option<String>)>,
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
///     #1 [OcrWord] "word" (bounds: [x,y,w,h])
///     #2 [OcrWord] "another" (bounds: [x,y,w,h])
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

        // Format: #index [label] "content" (bounds: [x,y,w,h])
        output.push_str(&format!("#{} [{}]", index, item.label));

        if let Some(ref content) = item.content {
            if !content.is_empty() {
                output.push_str(&format!(" \"{content}\""));
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

/// Format vision elements as compact YAML with #index [type] "content" - "description" format
///
/// Output format:
/// #1 [button] "Submit" - "Primary form submission button" (bounds: [x,y,w,h])
/// #2 [input] "Email" - "Text input for email address" (bounds: [x,y,w,h])
/// #3 [icon] "" - "Settings gear icon" (bounds: [x,y,w,h])
///
/// Returns tuple of (formatted string, cache for click_element_by_index)
pub fn format_vision_tree_as_compact_yaml(
    items: &[VisionElement],
) -> (String, HashMap<u32, VisionElement>) {
    let mut output = String::new();
    let mut cache = HashMap::new();

    for (i, item) in items.iter().enumerate() {
        let index = (i + 1) as u32;
        cache.insert(index, item.clone());

        // Format: #index [type] "content" - "description" (bounds: [x,y,w,h])
        output.push_str(&format!("#{} [{}]", index, item.element_type));

        // Add content (visible text)
        let content_str = item.content.as_deref().unwrap_or("");
        output.push_str(&format!(" \"{}\"", content_str));

        // Add description if present
        if let Some(ref desc) = item.description {
            if !desc.is_empty() {
                output.push_str(&format!(" - \"{}\"", desc));
            }
        }

        // Add bounds
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
/// #1 [tag] [.class1.class2] name #element_id (bounds: [x,y,w,h])
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

        let has_bounds =
            matches!((x, y, w, h), (Some(_), Some(_), Some(w), Some(h)) if w > 0.0 && h > 0.0);

        // Add index first if element has valid bounds, otherwise dash prefix
        if has_bounds {
            output.push_str(&format!("#{next_index} [{tag}]"));
        } else {
            output.push_str(&format!("- [{tag}]"));
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
                output.push_str(&format!(" [.{classes_joined}]"));
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
            output.push_str(&format!(" {truncated}"));
        }

        // Add element id if present
        let elem_id = elem
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        if let Some(id) = elem_id {
            output.push_str(&format!(" #{id}"));
        }

        // Add bounds and store in cache
        if let (Some(x), Some(y), Some(w), Some(h)) = (x, y, w, h) {
            if w > 0.0 && h > 0.0 {
                output.push_str(&format!(
                    " (bounds: [{},{},{},{}])",
                    x as i64, y as i64, w as i64, h as i64
                ));

                // Build identifier for overlay: prefer text content, then id, then classes, then empty
                let identifier = name
                    .map(|s| {
                        // Truncate for overlay display
                        let clean = s.replace('\n', " ").replace('\r', "");
                        if clean.len() > 30 {
                            format!("{}...", &clean[..27])
                        } else {
                            clean
                        }
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

// ============================================================================
// Clustered Tree Output - Groups elements from all sources by spatial proximity
// ============================================================================

/// Source of an element for clustered output
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementSource {
    Uia,        // #u - Accessibility tree
    Dom,        // #d - Browser DOM
    Ocr,        // #o - OCR text
    Omniparser, // #p - Omniparser vision
    Gemini,     // #g - Gemini vision
}

impl ElementSource {
    /// Get the prefix character for this source
    pub fn prefix(&self) -> char {
        match self {
            ElementSource::Uia => 'u',
            ElementSource::Dom => 'd',
            ElementSource::Ocr => 'o',
            ElementSource::Omniparser => 'p',
            ElementSource::Gemini => 'g',
        }
    }

    /// Parse a prefixed index string like "u1" or "d23"
    pub fn parse_prefixed_index(s: &str) -> Option<(ElementSource, u32)> {
        if s.is_empty() {
            return None;
        }
        let prefix = s.chars().next()?;
        let num_str = &s[1..];
        let num: u32 = num_str.parse().ok()?;
        let source = match prefix {
            'u' => ElementSource::Uia,
            'd' => ElementSource::Dom,
            'o' => ElementSource::Ocr,
            'p' => ElementSource::Omniparser,
            'g' => ElementSource::Gemini,
            _ => return None,
        };
        Some((source, num))
    }
}

/// A unified element representation for clustering across all sources
#[derive(Debug, Clone)]
pub struct UnifiedElement {
    pub source: ElementSource,
    pub index: u32,
    pub display_type: String,         // role/tag/label/element_type
    pub text: Option<String>,         // name/text/content
    pub description: Option<String>,  // Gemini description, DOM identifier
    pub bounds: (f64, f64, f64, f64), // x, y, width, height
}

impl UnifiedElement {
    /// Get the prefixed index string (e.g., "u1", "d2")
    pub fn prefixed_index(&self) -> String {
        format!("{}{}", self.source.prefix(), self.index)
    }

    /// Get the center point of the element
    pub fn center(&self) -> (f64, f64) {
        let (x, y, w, h) = self.bounds;
        (x + w / 2.0, y + h / 2.0)
    }
}

/// Result of clustered tree formatting
#[derive(Debug, Clone)]
pub struct ClusteredFormattingResult {
    /// The formatted YAML string with clusters
    pub formatted: String,
    /// Mapping of prefixed index (e.g., "u1", "d2") to (source, original_index, bounds)
    pub index_to_source_and_bounds: HashMap<String, (ElementSource, u32, (f64, f64, f64, f64))>,
}

/// Calculate minimum edge-to-edge distance between two bounding boxes
/// Returns 0 for overlapping/touching elements
fn min_edge_distance(b1: (f64, f64, f64, f64), b2: (f64, f64, f64, f64)) -> f64 {
    let (x1, y1, w1, h1) = b1;
    let (x2, y2, w2, h2) = b2;

    // Horizontal gap (0 if overlapping horizontally)
    let h_gap = f64::max(0.0, f64::max(x1 - (x2 + w2), x2 - (x1 + w1)));

    // Vertical gap (0 if overlapping vertically)
    let v_gap = f64::max(0.0, f64::max(y1 - (y2 + h2), y2 - (y1 + h1)));

    // Euclidean distance if diagonal, otherwise just the gap
    (h_gap * h_gap + v_gap * v_gap).sqrt()
}

/// Determine if two elements should be clustered together
/// Uses relative threshold based on smaller element dimension
fn should_cluster(b1: (f64, f64, f64, f64), b2: (f64, f64, f64, f64)) -> bool {
    let smaller_dim = f64::min(f64::min(b1.2, b1.3), f64::min(b2.2, b2.3));
    // Threshold: 1.5x the smaller dimension
    let threshold = smaller_dim * 1.5;
    min_edge_distance(b1, b2) < threshold
}

/// Cluster elements by spatial proximity using union-find approach
fn cluster_elements(elements: Vec<UnifiedElement>) -> Vec<Vec<UnifiedElement>> {
    if elements.is_empty() {
        return vec![];
    }

    let n = elements.len();
    // Union-find parent array
    let mut parent: Vec<usize> = (0..n).collect();

    // Find with path compression
    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }

    // Union two sets
    fn union(parent: &mut [usize], i: usize, j: usize) {
        let pi = find(parent, i);
        let pj = find(parent, j);
        if pi != pj {
            parent[pi] = pj;
        }
    }

    // Build clusters by checking all pairs
    for i in 0..n {
        for j in (i + 1)..n {
            if should_cluster(elements[i].bounds, elements[j].bounds) {
                union(&mut parent, i, j);
            }
        }
    }

    // Group elements by their cluster root
    let mut cluster_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        cluster_map.entry(root).or_default().push(i);
    }

    // Convert to Vec<Vec<UnifiedElement>> and sort by reading order within clusters
    let mut clusters: Vec<Vec<UnifiedElement>> = cluster_map
        .into_values()
        .map(|indices| {
            let mut cluster: Vec<UnifiedElement> =
                indices.into_iter().map(|i| elements[i].clone()).collect();
            // Sort within cluster by reading order (Y then X)
            cluster.sort_by(|a, b| {
                let (_, ay, _, _) = a.bounds;
                let (_, by, _, _) = b.bounds;
                let (ax, _, _, _) = a.bounds;
                let (bx, _, _, _) = b.bounds;
                ay.partial_cmp(&by)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal))
            });
            cluster
        })
        .collect();

    // Sort clusters by the position of their first element (reading order)
    clusters.sort_by(|a, b| {
        let a_first = a.first().map(|e| e.bounds).unwrap_or((0.0, 0.0, 0.0, 0.0));
        let b_first = b.first().map(|e| e.bounds).unwrap_or((0.0, 0.0, 0.0, 0.0));
        a_first
            .1
            .partial_cmp(&b_first.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a_first
                    .0
                    .partial_cmp(&b_first.0)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    clusters
}

/// Flatten UIA tree into a list of elements with bounds
fn flatten_uia_tree(
    node: &SerializableUIElement,
    elements: &mut Vec<UnifiedElement>,
    next_index: &mut u32,
) {
    // Only include elements with bounds
    if let Some((x, y, w, h)) = node.bounds {
        if w > 0.0 && h > 0.0 {
            let idx = *next_index;
            *next_index += 1;
            elements.push(UnifiedElement {
                source: ElementSource::Uia,
                index: idx,
                display_type: node.role.clone(),
                text: node.name.clone(),
                description: node.description.clone(),
                bounds: (x, y, w, h),
            });
        }
    }

    // Recurse into children
    if let Some(ref children) = node.children {
        for child in children {
            flatten_uia_tree(child, elements, next_index);
        }
    }
}

/// Format clustered tree output combining all sources
///
/// Groups elements from UIA, DOM, OCR, Omniparser, and Gemini Vision by spatial proximity.
/// Elements within 1.5x their smaller dimension are grouped together.
///
/// Output format:
/// ```text
/// # Cluster @(100,200)
/// - [Button] #u1 "Submit" (bounds: [100,200,80,30])
/// - [button] #d1 "Submit" (bounds: [100,200,80,30])
/// - [OcrWord] #o1 "Submit" (bounds: [102,205,76,25])
///
/// # Cluster @(100,280)
/// - [Text] #u2 "Username"
/// - [input] #d2 (bounds: [100,300,200,30])
/// ```
pub fn format_clustered_tree(
    uia_tree: Option<&SerializableUIElement>,
    dom_elements: &HashMap<u32, (String, String, (f64, f64, f64, f64))>,
    ocr_elements: &HashMap<u32, (String, (f64, f64, f64, f64))>,
    omniparser_items: &HashMap<u32, OmniparserItem>,
    vision_items: &HashMap<u32, VisionElement>,
) -> ClusteredFormattingResult {
    let mut all_elements: Vec<UnifiedElement> = Vec::new();
    let mut index_to_source_and_bounds: HashMap<
        String,
        (ElementSource, u32, (f64, f64, f64, f64)),
    > = HashMap::new();

    // 1. Flatten UIA tree
    if let Some(tree) = uia_tree {
        let mut next_idx = 1u32;
        flatten_uia_tree(tree, &mut all_elements, &mut next_idx);
    }

    // 2. Add DOM elements
    for (idx, (tag, identifier, bounds)) in dom_elements {
        all_elements.push(UnifiedElement {
            source: ElementSource::Dom,
            index: *idx,
            display_type: tag.clone(),
            text: if identifier.is_empty() {
                None
            } else {
                Some(identifier.clone())
            },
            description: None,
            bounds: *bounds,
        });
    }

    // 3. Add OCR elements
    for (idx, (text, bounds)) in ocr_elements {
        all_elements.push(UnifiedElement {
            source: ElementSource::Ocr,
            index: *idx,
            display_type: "OcrWord".to_string(),
            text: Some(text.clone()),
            description: None,
            bounds: *bounds,
        });
    }

    // 4. Add Omniparser elements
    for (idx, item) in omniparser_items {
        if let Some(box_2d) = item.box_2d {
            // Convert [x_min, y_min, x_max, y_max] to (x, y, w, h)
            let bounds = (
                box_2d[0],
                box_2d[1],
                box_2d[2] - box_2d[0],
                box_2d[3] - box_2d[1],
            );
            all_elements.push(UnifiedElement {
                source: ElementSource::Omniparser,
                index: *idx,
                display_type: item.label.clone(),
                text: item.content.clone(),
                description: None,
                bounds,
            });
        }
    }

    // 5. Add Gemini Vision elements
    for (idx, item) in vision_items {
        if let Some(box_2d) = item.box_2d {
            // Convert [x_min, y_min, x_max, y_max] to (x, y, w, h)
            let bounds = (
                box_2d[0],
                box_2d[1],
                box_2d[2] - box_2d[0],
                box_2d[3] - box_2d[1],
            );
            all_elements.push(UnifiedElement {
                source: ElementSource::Gemini,
                index: *idx,
                display_type: item.element_type.clone(),
                text: item.content.clone(),
                description: item.description.clone(),
                bounds,
            });
        }
    }

    // Build the index mapping
    for elem in &all_elements {
        let key = elem.prefixed_index();
        index_to_source_and_bounds.insert(key, (elem.source, elem.index, elem.bounds));
    }

    // Cluster the elements
    let clusters = cluster_elements(all_elements);

    // Format output
    let mut output = String::new();

    for cluster in clusters {
        if cluster.is_empty() {
            continue;
        }

        // Calculate cluster centroid for header
        let (sum_x, sum_y, count) = cluster.iter().fold((0.0, 0.0, 0), |(sx, sy, c), elem| {
            let (cx, cy) = elem.center();
            (sx + cx, sy + cy, c + 1)
        });
        let centroid_x = sum_x / count as f64;
        let centroid_y = sum_y / count as f64;

        // Cluster header
        output.push_str(&format!(
            "# Cluster @({:.0},{:.0})\n",
            centroid_x, centroid_y
        ));

        // Format each element in the cluster
        for elem in &cluster {
            output.push_str(&format!(
                "#{} [{}]",
                elem.prefixed_index(),
                elem.display_type
            ));

            // Add text if present
            if let Some(ref text) = elem.text {
                if !text.is_empty() {
                    // Truncate long text
                    let clean_text = text.replace('\n', " ").replace('\r', "");
                    let truncated = if clean_text.len() > 50 {
                        format!("{}...", &clean_text[..47])
                    } else {
                        clean_text
                    };
                    output.push_str(&format!(" \"{}\"", truncated));
                }
            }

            // Add description if present (Gemini)
            if let Some(ref desc) = elem.description {
                if !desc.is_empty() {
                    let clean_desc = desc.replace('\n', " ").replace('\r', "");
                    let truncated = if clean_desc.len() > 40 {
                        format!("{}...", &clean_desc[..37])
                    } else {
                        clean_desc
                    };
                    output.push_str(&format!(" - \"{}\"", truncated));
                }
            }

            // Add bounds
            let (x, y, w, h) = elem.bounds;
            output.push_str(&format!(" (bounds: [{:.0},{:.0},{:.0},{:.0}])", x, y, w, h));

            output.push('\n');
        }

        output.push('\n'); // Blank line between clusters
    }

    ClusteredFormattingResult {
        formatted: output,
        index_to_source_and_bounds,
    }
}

/// Format clustered tree output from cached bounds data
///
/// This is a convenience function that takes the cached bounds from each source
/// (as stored in DesktopWrapper) and produces a clustered output.
pub fn format_clustered_tree_from_caches(
    uia_bounds: &HashMap<u32, (String, String, (f64, f64, f64, f64), Option<String>)>,
    dom_bounds: &HashMap<u32, (String, String, (f64, f64, f64, f64))>,
    ocr_bounds: &HashMap<u32, (String, (f64, f64, f64, f64))>,
    omniparser_items: &HashMap<u32, OmniparserItem>,
    vision_items: &HashMap<u32, VisionElement>,
) -> ClusteredFormattingResult {
    let mut all_elements: Vec<UnifiedElement> = Vec::new();

    // Add UIA elements from cache
    for (idx, (role, name, bounds, _selector)) in uia_bounds {
        all_elements.push(UnifiedElement {
            source: ElementSource::Uia,
            index: *idx,
            display_type: role.clone(),
            text: if name.is_empty() {
                None
            } else {
                Some(name.clone())
            },
            description: None,
            bounds: *bounds,
        });
    }

    // Add DOM elements
    for (idx, (tag, identifier, bounds)) in dom_bounds {
        all_elements.push(UnifiedElement {
            source: ElementSource::Dom,
            index: *idx,
            display_type: tag.clone(),
            text: if identifier.is_empty() {
                None
            } else {
                Some(identifier.clone())
            },
            description: None,
            bounds: *bounds,
        });
    }

    // Add OCR elements
    for (idx, (text, bounds)) in ocr_bounds {
        all_elements.push(UnifiedElement {
            source: ElementSource::Ocr,
            index: *idx,
            display_type: "OcrWord".to_string(),
            text: Some(text.clone()),
            description: None,
            bounds: *bounds,
        });
    }

    // Add Omniparser elements
    for (idx, item) in omniparser_items {
        if let Some(box_2d) = item.box_2d {
            let bounds = (
                box_2d[0],
                box_2d[1],
                box_2d[2] - box_2d[0],
                box_2d[3] - box_2d[1],
            );
            all_elements.push(UnifiedElement {
                source: ElementSource::Omniparser,
                index: *idx,
                display_type: item.label.clone(),
                text: item.content.clone(),
                description: None,
                bounds,
            });
        }
    }

    // Add Gemini Vision elements
    for (idx, item) in vision_items {
        if let Some(box_2d) = item.box_2d {
            let bounds = (
                box_2d[0],
                box_2d[1],
                box_2d[2] - box_2d[0],
                box_2d[3] - box_2d[1],
            );
            all_elements.push(UnifiedElement {
                source: ElementSource::Gemini,
                index: *idx,
                display_type: item.element_type.clone(),
                text: item.content.clone(),
                description: item.description.clone(),
                bounds,
            });
        }
    }

    // Build the index mapping
    let mut index_to_source_and_bounds: HashMap<
        String,
        (ElementSource, u32, (f64, f64, f64, f64)),
    > = HashMap::new();
    for elem in &all_elements {
        let key = elem.prefixed_index();
        index_to_source_and_bounds.insert(key, (elem.source, elem.index, elem.bounds));
    }

    // Cluster the elements
    let clusters = cluster_elements(all_elements);

    // Format output
    let mut output = String::new();

    for cluster in clusters {
        if cluster.is_empty() {
            continue;
        }

        // Calculate cluster centroid for header
        let (sum_x, sum_y, count) = cluster.iter().fold((0.0, 0.0, 0), |(sx, sy, c), elem| {
            let (cx, cy) = elem.center();
            (sx + cx, sy + cy, c + 1)
        });
        let centroid_x = sum_x / count as f64;
        let centroid_y = sum_y / count as f64;

        // Cluster header
        output.push_str(&format!(
            "# Cluster @({:.0},{:.0})\n",
            centroid_x, centroid_y
        ));

        // Format each element in the cluster
        for elem in &cluster {
            output.push_str(&format!(
                "#{} [{}]",
                elem.prefixed_index(),
                elem.display_type
            ));

            // Add text if present
            if let Some(ref text) = elem.text {
                if !text.is_empty() {
                    let clean_text = text.replace('\n', " ").replace('\r', "");
                    let truncated = if clean_text.len() > 50 {
                        format!("{}...", &clean_text[..47])
                    } else {
                        clean_text
                    };
                    output.push_str(&format!(" \"{}\"", truncated));
                }
            }

            // Add description if present (Gemini)
            if let Some(ref desc) = elem.description {
                if !desc.is_empty() {
                    let clean_desc = desc.replace('\n', " ").replace('\r', "");
                    let truncated = if clean_desc.len() > 40 {
                        format!("{}...", &clean_desc[..37])
                    } else {
                        clean_desc
                    };
                    output.push_str(&format!(" - \"{}\"", truncated));
                }
            }

            // Add bounds
            let (x, y, w, h) = elem.bounds;
            output.push_str(&format!(" (bounds: [{:.0},{:.0},{:.0},{:.0}])", x, y, w, h));

            output.push('\n');
        }

        output.push('\n'); // Blank line between clusters
    }

    ClusteredFormattingResult {
        formatted: output,
        index_to_source_and_bounds,
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
        assert!(result.formatted.contains("[Button] Submit"));
        assert!(!result.formatted.contains("#123")); // IDs should not appear in compact view
        assert!(result.formatted.contains("bounds: [10,20,100,50]"));
        assert!(result.formatted.contains("focusable"));
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
        assert!(result.formatted.contains("- [Window] Main"));
        assert!(result.formatted.contains("  - [Text] Label"));
        assert!(!result.formatted.contains("#123")); // IDs should not appear in compact view
        assert!(!result.formatted.contains("#456")); // IDs should not appear in compact view
    }
}
