mod desktop;
mod element;
mod exceptions;
mod locator;
mod types;

// Main types first
pub use desktop::Desktop;
pub use element::Element;
pub use locator::Locator;
pub use types::{
    Bounds, ClickResult, CommandOutput, Coordinates, Monitor, MonitorScreenshotPair,
    PropertyLoadingMode, ScreenshotResult, TreeBuildConfig, UIElementAttributes, UINode,
};

// Error handling - see exceptions.rs for detailed architecture
pub use exceptions::map_error;
