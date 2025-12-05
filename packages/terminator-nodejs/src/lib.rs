mod desktop;
mod element;
mod exceptions;
mod locator;
mod selector;
mod types;
mod window_manager;

// Main types first
pub use desktop::Desktop;
pub use element::Element;
pub use locator::Locator;
pub use selector::Selector;
pub use types::{
    Bounds, ClickResult, CommandOutput, Coordinates, FontStyle, HighlightHandle, Monitor,
    MonitorScreenshotPair, PropertyLoadingMode, ScreenshotResult, TextPosition, TreeBuildConfig,
    UIElementAttributes, UINode,
};
pub use window_manager::{WindowInfo, WindowManager};

// Error handling - see exceptions.rs for detailed architecture
pub use exceptions::map_error;
