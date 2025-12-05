mod desktop;
mod element;
mod exceptions;
mod locator;
mod selector;
mod types;
mod window_manager;

// Main types first
pub use desktop::Desktop;
pub use element::{ActionOptions, Element, PressKeyOptions, TypeTextOptions};
pub use locator::Locator;
pub use selector::Selector;
pub use types::{
    Bounds, BoundsEntry, ClickResult, CommandOutput, Coordinates, FontStyle, HighlightHandle,
    Monitor, MonitorScreenshotPair, PropertyLoadingMode, ScreenshotResult, TextPosition,
    TreeBuildConfig, UIElementAttributes, UINode, WindowTreeResult,
};
pub use window_manager::{WindowInfo, WindowManager};

// Error handling - see exceptions.rs for detailed architecture
pub use exceptions::map_error;
