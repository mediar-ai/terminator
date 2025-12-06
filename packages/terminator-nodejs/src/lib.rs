mod desktop;
mod element;
mod exceptions;
mod locator;
mod selector;
mod types;
mod window_manager;

// Main types first
pub use desktop::Desktop;
pub use element::{Element, TypeTextOptions};
pub use locator::Locator;
pub use selector::Selector;
pub use types::{
    Bounds, BoundsEntry, BrowserDomElement, BrowserDomResult, ClickResult, CommandOutput,
    Coordinates, DomBoundsEntry, FontStyle, GeminiVisionResult, HighlightHandle, Monitor,
    MonitorScreenshotPair, OcrBoundsEntry, OcrElement, OcrResult, OmniparserBoundsEntry,
    OmniparserItem, OmniparserResult, PropertyLoadingMode, ScreenshotResult, TextPosition,
    TreeBuildConfig, TreeOutputFormat, UIElementAttributes, UINode, VisionBoundsEntry,
    VisionElement, WindowTreeResult,
};
pub use window_manager::{WindowInfo, WindowManager};

// Error handling - see exceptions.rs for detailed architecture
pub use exceptions::map_error;
