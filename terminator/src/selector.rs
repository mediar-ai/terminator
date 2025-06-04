use std::collections::BTreeMap;

/// Represents ways to locate a UI element
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Selector {
    /// Select by role and optional name
    Role { role: String, name: Option<String> }, // TODO: name unused 
    /// Select by accessibility ID
    Id(String),
    /// Select by name/label
    Name(String),
    /// Select by text content
    Text(String),
    /// Select using XPath-like query
    Path(String),
    /// Select by using Native Automation id, (eg: `AutomationID` for windows)
    NativeId(String),
    /// Select by multiple attributes (key-value pairs)
    Attributes(BTreeMap<String, String>),
    /// Filter current elements by a predicate
    Filter(usize), // Uses an ID to reference a filter predicate stored separately
    /// Chain multiple selectors
    Chain(Vec<Selector>),
    /// Select by class name
    ClassName(String),
}

impl From<&str> for Selector {
    fn from(s: &str) -> Self {
        // Make common UI roles like "window", "button", etc. default to Role selectors
        // instead of Name selectors
        match s {
            // if role:button 
            _ if s.starts_with("role:") => Selector::Role {
                role: s[5..].to_string(),
                name: None,
            },
            "app" | "application" | "window" | "button" | "checkbox" | "menu" | "menuitem" | "menubar" | "textfield"
            | "input" => Selector::Role {
                role: s.to_string(),
                name: None,
            },
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
            _ => Selector::Name(s.to_string()),
        }
    }
}
