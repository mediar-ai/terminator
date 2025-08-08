use std::collections::BTreeMap;

/// Represents ways to locate a UI element
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Selector {
    /// Select by role and optional name
    Role { role: String, name: Option<String> },
    /// Select by accessibility ID
    Id(String),
    /// Select by name/label
    Name(String),
    /// Select by text content
    Text(String),
    /// Select using XPath-like query
    Path(String),
    /// Select by using Native Automation id, (eg: `AutomationID` for windows) and for linux it is Id value in Attributes
    NativeId(String),
    /// Select by multiple attributes (key-value pairs)
    Attributes(BTreeMap<String, String>),
    /// Filter current elements by a predicate
    Filter(usize), // Uses an ID to reference a filter predicate stored separately
    /// Chain multiple selectors
    Chain(Vec<Selector>),
    /// Select by class name
    ClassName(String),
    /// Filter by visibility on screen
    Visible(bool),
    /// Select by localized role
    LocalizedRole(String),
    /// Select elements to the right of an anchor element
    RightOf(Box<Selector>),
    /// Select elements to the left of an anchor element
    LeftOf(Box<Selector>),
    /// Select elements above an anchor element
    Above(Box<Selector>),
    /// Select elements below an anchor element
    Below(Box<Selector>),
    /// Select elements near an anchor element
    Near(Box<Selector>),
    /// Select the n-th element from the matches
    Nth(i32),
    /// Select elements that have at least one descendant matching the inner selector (Playwright-style :has())
    Has(Box<Selector>),
    /// Navigate to parent element (Playwright-style ..)
    Parent,
    /// Logical AND over a set of selectors (all must match the same element)
    And(Vec<Selector>),
    /// Logical OR over a set of selectors (any may match)
    Or(Vec<Selector>),
    /// Represents an invalid selector string, with a reason.
    Invalid(String),
}

impl std::fmt::Display for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<&str> for Selector {
    fn from(s: &str) -> Self {
        // Helper: parse a single, non-chain segment with AND/OR support
        fn parse_segment(input: &str) -> Selector {
            let s = input.trim();

            // OR: comma-separated (CSS/Playwright-style) or explicit ||
            // Note: we split by comma first to avoid interfering with role|name syntax.
            if s.contains(',') || s.contains("||") {
                // Split by comma and ||, trim pieces
                let mut parts: Vec<Selector> = Vec::new();
                for piece in s.split(',') {
                    for sub in piece.split("||") {
                        let sub = sub.trim();
                        if sub.is_empty() { continue; }
                        parts.push(parse_segment(sub));
                    }
                }
                // Flatten single OR
                return if parts.len() == 1 { parts.into_iter().next().unwrap() } else { Selector::Or(parts) };
            }

            // AND: explicit && only (avoid ambiguous natural language 'and')
            if s.contains("&&") {
                let parts: Vec<Selector> = s
                    .split("&&")
                    .map(|p| parse_segment(p.trim()))
                    .collect();
                return if parts.len() == 1 { parts.into_iter().next().unwrap() } else { Selector::And(parts) };
            }

            // Single '|' is not supported; guide to use && instead of role|name
            if s.contains('|') && !s.contains("||") {
                return Selector::Invalid(
                    "Use '&&' to combine conditions, e.g., 'role:button && name:Submit'"
                        .to_string(),
                );
            }

            // Make common UI roles like "window", "button", etc. default to Role selectors
            // instead of Name selectors
            match s {
                // if role:button
                _ if s.starts_with("role:") => Selector::Role {
                    role: s[5..].to_string(),
                    name: None,
                },
                "app" | "application" | "window" | "button" | "checkbox" | "menu" | "menuitem"
                | "menubar" | "textfield" | "input" => {
                    let parts: Vec<&str> = s.splitn(2, ':').collect();
                    Selector::Role {
                        role: parts.first().unwrap_or(&"").to_string(),
                        name: parts.get(1).map(|name| name.to_string()), // optional
                    }
                }
                // starts with AX
                _ if s.starts_with("AX") => Selector::Role {
                    role: s.to_string(),
                    name: None,
                },
                _ if s.starts_with("Name:") || s.starts_with("name:") => {
                    let parts: Vec<&str> = s.splitn(2, ':').collect();
                    Selector::Name(parts[1].to_string())
                }
                _ if s.to_lowercase().starts_with("classname:") => {
                    let parts: Vec<&str> = s.splitn(2, ':').collect();
                    Selector::ClassName(parts[1].to_string())
                }
                _ if s.to_lowercase().starts_with("nativeid:") => {
                    let parts: Vec<&str> = s.splitn(2, ':').collect();
                    Selector::NativeId(parts[1].trim().to_string())
                }
                _ if s.to_lowercase().starts_with("visible:") => {
                    let value = s[8..].trim().to_lowercase();
                    Selector::Visible(value == "true")
                }
                _ if s.to_lowercase().starts_with("attr:") => {
                    let attr_part = &s["attr:".len()..];
                    let mut attributes = BTreeMap::new();

                    if attr_part.contains('=') {
                        // Format: attr:key=value (like Playwright)
                        let parts: Vec<&str> = attr_part.splitn(2, '=').collect();
                        if parts.len() == 2 {
                            attributes.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
                        }
                    } else {
                        // Format: attr:key (check for existence, assume true)
                        attributes.insert(attr_part.trim().to_string(), "true".to_string());
                    }

                    Selector::Attributes(attributes)
                }

                _ if s.to_lowercase().starts_with("rightof:") => {
                    let inner_selector_str = &s["rightof:".len()..];
                    Selector::RightOf(Box::new(Selector::from(inner_selector_str)))
                }
                _ if s.to_lowercase().starts_with("leftof:") => {
                    let inner_selector_str = &s["leftof:".len()..];
                    Selector::LeftOf(Box::new(Selector::from(inner_selector_str)))
                }
                _ if s.to_lowercase().starts_with("above:") => {
                    let inner_selector_str = &s["above:".len()..];
                    Selector::Above(Box::new(Selector::from(inner_selector_str)))
                }
                _ if s.to_lowercase().starts_with("below:") => {
                    let inner_selector_str = &s["below:".len()..];
                    Selector::Below(Box::new(Selector::from(inner_selector_str)))
                }
                _ if s.to_lowercase().starts_with("near:") => {
                    let inner_selector_str = &s["near:".len()..];
                    Selector::Near(Box::new(Selector::from(inner_selector_str)))
                }
                _ if s.to_lowercase().starts_with("has:") => {
                    let inner_selector_str = &s["has:".len()..];
                    Selector::Has(Box::new(Selector::from(inner_selector_str)))
                }
                _ if s.to_lowercase().starts_with("nth=") || s.to_lowercase().starts_with("nth:") => {
                    let index_str = if s.to_lowercase().starts_with("nth:") {
                        &s["nth:".len()..]
                    } else {
                        &s["nth=".len()..]
                    };

                    if let Ok(index) = index_str.parse::<i32>() {
                        Selector::Nth(index)
                    } else {
                        Selector::Invalid(format!("Invalid index for nth selector: '{index_str}'"))
                    }
                }
                _ if s.starts_with("id:") => Selector::Id(s[3..].to_string()),
                _ if s.starts_with("text:") => Selector::Text(s[5..].to_string()),
                _ if s.contains(':') => {
                    let parts: Vec<&str> = s.splitn(2, ':').collect();
                    Selector::Role {
                        role: parts[0].to_string(),
                        name: Some(parts[1].to_string()),
                    }
                }
                _ if s.starts_with('#') => Selector::Id(s[1..].to_string()),
                _ if s.starts_with('/') => Selector::Path(s.to_string()),
                _ if s.to_lowercase().starts_with("text:") => Selector::Text(s[5..].to_string()),
                ".." => Selector::Parent,
                _ => Selector::Invalid(format!(
                    "Unknown selector format: \"{s}\". Use prefixes like 'role:', 'name:', 'id:', 'text:', 'nativeid:', 'classname:', 'attr:', 'visible:', or 'has:' to specify the selector type."
                )),
            }
        }

        // Handle chained selectors first
        let parts: Vec<&str> = s.split(">>").map(|p| p.trim()).collect();
        if parts.len() > 1 {
            return Selector::Chain(parts.into_iter().map(parse_segment).collect());
        }

        // Single segment with logicals
        parse_segment(s)
    }
}
