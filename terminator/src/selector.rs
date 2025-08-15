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
        // Handle chained selectors first
        let parts: Vec<&str> = s.split(">>").map(|p| p.trim()).collect();
        if parts.len() > 1 {
            return Selector::Chain(parts.into_iter().map(Selector::from).collect());
        }

        // if using pipe, use it for the role plus name (preferred precise format)
        if s.contains('|') {
            let parts: Vec<&str> = s.split('|').collect();
            if parts.len() >= 2 {
                let role_part = parts[0].trim();
                let name_part = parts[1].trim();

                // Handle role:abcd|name:abcd format
                let role = role_part
                    .strip_prefix("role:")
                    .unwrap_or(role_part)
                    .to_string();

                // Handle name: and contains: prefixes (including nested name:contains:)
                let mut name = name_part.strip_prefix("name:").unwrap_or(name_part);

                // If after stripping name: we still have contains:, strip that too
                name = name.strip_prefix("contains:").unwrap_or(name);

                let name = name.to_string();

                return Selector::Role {
                    role,
                    name: Some(name),
                };
            }
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
}
