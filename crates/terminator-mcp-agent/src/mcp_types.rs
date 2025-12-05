use rmcp::{schemars, schemars::JsonSchema};
use serde::{Deserialize, Serialize};

fn default_font_size() -> u32 {
    12
}

/// Position of text overlay relative to the highlighted element
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum TextPosition {
    #[schemars(description = "Above the element")]
    Top,
    #[schemars(description = "Top-right corner")]
    TopRight,
    #[schemars(description = "Right side of the element")]
    Right,
    #[schemars(description = "Bottom-right corner")]
    BottomRight,
    #[schemars(description = "Below the element")]
    Bottom,
    #[schemars(description = "Bottom-left corner")]
    BottomLeft,
    #[schemars(description = "Left side of the element")]
    Left,
    #[schemars(description = "Top-left corner")]
    TopLeft,
    #[schemars(description = "Inside the element")]
    Inside,
}

#[cfg(target_os = "windows")]
impl From<TextPosition> for terminator::platforms::windows::TextPosition {
    fn from(pos: TextPosition) -> Self {
        match pos {
            TextPosition::Top => terminator::platforms::windows::TextPosition::Top,
            TextPosition::TopRight => terminator::platforms::windows::TextPosition::TopRight,
            TextPosition::Right => terminator::platforms::windows::TextPosition::Right,
            TextPosition::BottomRight => terminator::platforms::windows::TextPosition::BottomRight,
            TextPosition::Bottom => terminator::platforms::windows::TextPosition::Bottom,
            TextPosition::BottomLeft => terminator::platforms::windows::TextPosition::BottomLeft,
            TextPosition::Left => terminator::platforms::windows::TextPosition::Left,
            TextPosition::TopLeft => terminator::platforms::windows::TextPosition::TopLeft,
            TextPosition::Inside => terminator::platforms::windows::TextPosition::Inside,
        }
    }
}

/// Output format for UI tree
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum TreeOutputFormat {
    #[schemars(description = "Full JSON format with all fields (current behavior)")]
    VerboseJson,
    #[schemars(description = "Compact YAML format: [ROLE] name #id (default)")]
    #[default]
    CompactYaml,
    #[schemars(
        description = "Clustered YAML format: groups elements from all sources (UIA, DOM, OCR, Omniparser, Gemini) by spatial proximity with prefixed indices (#u1, #d2, #o3, #p4, #g5)"
    )]
    ClusteredYaml,
}

/// Font styling options for text overlay
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FontStyle {
    #[schemars(description = "Font size in pixels")]
    #[serde(default = "default_font_size")]
    pub size: u32,
    #[schemars(description = "Whether the font should be bold")]
    #[serde(default)]
    pub bold: bool,
    #[schemars(description = "Text color in BGR format")]
    #[serde(default)]
    pub color: u32,
}

#[cfg(target_os = "windows")]
impl From<FontStyle> for terminator::platforms::windows::FontStyle {
    fn from(style: FontStyle) -> Self {
        terminator::platforms::windows::FontStyle {
            size: style.size,
            bold: style.bold,
            color: style.color,
        }
    }
}
