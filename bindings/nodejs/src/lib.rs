mod desktop;
mod element;
mod locator;
mod types;
mod exceptions;

// Main types first
pub use desktop::Desktop;
pub use element::Element;
pub use locator::Locator;
pub use types::{
    Bounds,
    Coordinates,
    ClickResult,
    CommandOutput,
    ScreenshotResult,
    UIElementAttributes,
};

// Error handling - see exceptions.rs for detailed architecture
pub use exceptions::map_error;
