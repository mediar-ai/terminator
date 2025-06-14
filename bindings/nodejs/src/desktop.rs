use napi_derive::napi;
use std::sync::Once;
use terminator::Desktop as TerminatorDesktop;

use crate::types::{Monitor, MonitorScreenshotPair};
use crate::{
    map_error, CommandOutput, Element, Locator, ScreenshotResult, TreeBuildConfig, UINode,
};

/// Main entry point for desktop automation.
#[napi(js_name = "Desktop")]
pub struct Desktop {
    inner: TerminatorDesktop,
}

#[allow(clippy::needless_pass_by_value)]
#[napi]
impl Desktop {
    /// Create a new Desktop automation instance with configurable options.
    ///
    /// @param {boolean} [useBackgroundApps=false] - Enable background apps support.
    /// @param {boolean} [activateApp=false] - Enable app activation support.
    /// @param {string} [logLevel] - Logging level (e.g., 'info', 'debug', 'warn', 'error').
    /// @returns {Desktop} A new Desktop automation instance.
    #[napi(constructor)]
    pub fn new(
        use_background_apps: Option<bool>,
        activate_app: Option<bool>,
        log_level: Option<String>,
    ) -> Self {
        let use_background_apps = use_background_apps.unwrap_or(false);
        let activate_app = activate_app.unwrap_or(false);
        let log_level = log_level.unwrap_or_else(|| "info".to_string());
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(log_level)
                .try_init();
        });
        let desktop = TerminatorDesktop::new(use_background_apps, activate_app)
            .expect("Failed to create Desktop instance");
        Desktop { inner: desktop }
    }

    /// Get the root UI element of the desktop.
    ///
    /// @returns {Element} The root UI element.
    #[napi]
    pub fn root(&self) -> Element {
        let root = self.inner.root();
        Element::from(root)
    }

    /// Get a list of all running applications.
    ///
    /// @returns {Array<Element>} List of application UI elements.
    #[napi]
    pub fn applications(&self) -> napi::Result<Vec<Element>> {
        self.inner
            .applications()
            .map(|apps| apps.into_iter().map(Element::from).collect())
            .map_err(map_error)
    }

    /// Get a running application by name.
    ///
    /// @param {string} name - The name of the application to find.
    /// @returns {Element} The application UI element.
    #[napi]
    pub fn application(&self, name: String) -> napi::Result<Element> {
        self.inner
            .application(&name)
            .map(Element::from)
            .map_err(map_error)
    }

    /// Open an application by name.
    ///
    /// @param {string} name - The name of the application to open.
    #[napi]
    pub fn open_application(&self, name: String) -> napi::Result<Element> {
        self.inner
            .open_application(&name)
            .map(Element::from)
            .map_err(map_error)
    }

    /// Activate an application by name.
    ///
    /// @param {string} name - The name of the application to activate.
    #[napi]
    pub fn activate_application(&self, name: String) -> napi::Result<()> {
        self.inner.activate_application(&name).map_err(map_error)
    }

    /// (async) Capture a screenshot of the primary monitor.
    ///
    /// @returns {Promise<ScreenshotResult>} The screenshot data.
    #[napi]
    pub async fn capture_screen(&self) -> napi::Result<ScreenshotResult> {
        self.inner
            .capture_screen()
            .await
            .map(|r| ScreenshotResult {
                width: r.width,
                height: r.height,
                image_data: r.image_data,
                monitor: r.monitor.map(crate::types::Monitor::from),
            })
            .map_err(map_error)
    }

    /// (async) Run a shell command.
    ///
    /// @param {string} [windowsCommand] - Command to run on Windows.
    /// @param {string} [unixCommand] - Command to run on Unix.
    /// @returns {Promise<CommandOutput>} The command output.
    #[napi]
    pub async fn run_command(
        &self,
        windows_command: Option<String>,
        unix_command: Option<String>,
    ) -> napi::Result<CommandOutput> {
        self.inner
            .run_command(windows_command.as_deref(), unix_command.as_deref())
            .await
            .map(|r| CommandOutput {
                exit_status: r.exit_status,
                stdout: r.stdout,
                stderr: r.stderr,
            })
            .map_err(map_error)
    }

    /// (async) Get the name of the currently active monitor.
    ///
    /// @returns {Promise<string>} The name of the active monitor.
    #[napi]
    pub async fn get_active_monitor_name(&self) -> napi::Result<String> {
        self.inner
            .get_active_monitor_name()
            .await
            .map_err(map_error)
    }

    /// (async) Capture a screenshot of a specific monitor.
    ///
    /// @param {string} name - The name of the monitor to capture.
    /// @returns {Promise<ScreenshotResult>} The screenshot data.
    #[napi]
    pub async fn capture_monitor_by_name(&self, name: String) -> napi::Result<ScreenshotResult> {
        self.inner
            .capture_monitor_by_name(&name)
            .await
            .map(|r| ScreenshotResult {
                width: r.width,
                height: r.height,
                image_data: r.image_data,
                monitor: r.monitor.map(crate::types::Monitor::from),
            })
            .map_err(map_error)
    }

    /// (async) Perform OCR on an image file.
    ///
    /// @param {string} imagePath - Path to the image file.
    /// @returns {Promise<string>} The extracted text.
    #[napi]
    pub async fn ocr_image_path(&self, image_path: String) -> napi::Result<String> {
        self.inner
            .ocr_image_path(&image_path)
            .await
            .map_err(map_error)
    }

    /// (async) Perform OCR on a screenshot.
    ///
    /// @param {ScreenshotResult} screenshot - The screenshot to process.
    /// @returns {Promise<string>} The extracted text.
    #[napi]
    pub async fn ocr_screenshot(&self, screenshot: ScreenshotResult) -> napi::Result<String> {
        let rust_screenshot = terminator::ScreenshotResult {
            image_data: screenshot.image_data,
            width: screenshot.width,
            height: screenshot.height,
            monitor: screenshot.monitor.map(|m| terminator::Monitor {
                id: m.id,
                name: m.name,
                is_primary: m.is_primary,
                width: m.width,
                height: m.height,
                x: m.x,
                y: m.y,
                scale_factor: m.scale_factor,
            }),
        };
        self.inner
            .ocr_screenshot(&rust_screenshot)
            .await
            .map_err(map_error)
    }

    /// (async) Get the currently focused browser window.
    ///
    /// @returns {Promise<Element>} The current browser window element.
    #[napi]
    pub async fn get_current_browser_window(&self) -> napi::Result<Element> {
        self.inner
            .get_current_browser_window()
            .await
            .map(Element::from)
            .map_err(map_error)
    }

    /// Create a locator for finding UI elements.
    ///
    /// @param {string} selector - The selector string to find elements.
    /// @returns {Locator} A locator for finding elements.
    #[napi]
    pub fn locator(&self, selector: String) -> napi::Result<Locator> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = self.inner.locator(sel);
        Ok(Locator::from(loc))
    }

    /// (async) Get the currently focused window.
    ///
    /// @returns {Promise<Element>} The current window element.
    #[napi]
    pub async fn get_current_window(&self) -> napi::Result<Element> {
        self.inner
            .get_current_window()
            .await
            .map(Element::from)
            .map_err(map_error)
    }

    /// (async) Get the currently focused application.
    ///
    /// @returns {Promise<Element>} The current application element.
    #[napi]
    pub async fn get_current_application(&self) -> napi::Result<Element> {
        self.inner
            .get_current_application()
            .await
            .map(Element::from)
            .map_err(map_error)
    }

    /// Get the currently focused element.
    ///
    /// @returns {Element} The focused element.
    #[napi]
    pub fn focused_element(&self) -> napi::Result<Element> {
        self.inner
            .focused_element()
            .map(Element::from)
            .map_err(map_error)
    }

    /// Open a URL in a browser.
    ///
    /// @param {string} url - The URL to open.
    /// @param {string} [browser] - The browser to use.
    #[napi]
    pub fn open_url(&self, url: String, browser: Option<String>) -> napi::Result<Element> {
        self.inner
            .open_url(&url, browser.as_deref())
            .map(Element::from)
            .map_err(map_error)
    }

    /// Open a file with its default application.
    ///
    /// @param {string} filePath - Path to the file to open.
    #[napi]
    pub fn open_file(&self, file_path: String) -> napi::Result<()> {
        self.inner.open_file(&file_path).map_err(map_error)
    }

    /// Activate a browser window by title.
    ///
    /// @param {string} title - The window title to match.
    #[napi]
    pub fn activate_browser_window_by_title(&self, title: String) -> napi::Result<()> {
        self.inner
            .activate_browser_window_by_title(&title)
            .map_err(map_error)
    }

    /// Get the UI tree for a window identified by process ID and optional title.
    ///
    /// @param {number} pid - Process ID of the target application.
    /// @param {string} [title] - Optional window title filter.
    /// @param {TreeBuildConfig} [config] - Optional configuration for tree building.
    /// @returns {UINode} Complete UI tree starting from the identified window.
    #[napi]
    pub fn get_window_tree(
        &self,
        pid: u32,
        title: Option<String>,
        config: Option<TreeBuildConfig>,
    ) -> napi::Result<UINode> {
        let rust_config = config.map(|c| c.into());
        self.inner
            .get_window_tree(pid, title.as_deref(), rust_config)
            .map(UINode::from)
            .map_err(map_error)
    }

    // ============== NEW MONITOR METHODS ==============

    /// (async) List all available monitors/displays.
    ///
    /// @returns {Promise<Array<Monitor>>} List of monitor information.
    #[napi]
    pub async fn list_monitors(&self) -> napi::Result<Vec<Monitor>> {
        self.inner
            .list_monitors()
            .await
            .map(|monitors| monitors.into_iter().map(Monitor::from).collect())
            .map_err(map_error)
    }

    /// (async) Get the primary monitor.
    ///
    /// @returns {Promise<Monitor>} Primary monitor information.
    #[napi]
    pub async fn get_primary_monitor(&self) -> napi::Result<Monitor> {
        self.inner
            .get_primary_monitor()
            .await
            .map(Monitor::from)
            .map_err(map_error)
    }

    /// (async) Get the monitor containing the currently focused window.
    ///
    /// @returns {Promise<Monitor>} Active monitor information.
    #[napi]
    pub async fn get_active_monitor(&self) -> napi::Result<Monitor> {
        self.inner
            .get_active_monitor()
            .await
            .map(Monitor::from)
            .map_err(map_error)
    }

    /// (async) Get a monitor by its ID.
    ///
    /// @param {string} id - The monitor ID to find.
    /// @returns {Promise<Monitor>} Monitor information.
    #[napi]
    pub async fn get_monitor_by_id(&self, id: String) -> napi::Result<Monitor> {
        self.inner
            .get_monitor_by_id(&id)
            .await
            .map(Monitor::from)
            .map_err(map_error)
    }

    /// (async) Get a monitor by its name.
    ///
    /// @param {string} name - The monitor name to find.
    /// @returns {Promise<Monitor>} Monitor information.
    #[napi]
    pub async fn get_monitor_by_name(&self, name: String) -> napi::Result<Monitor> {
        self.inner
            .get_monitor_by_name(&name)
            .await
            .map(Monitor::from)
            .map_err(map_error)
    }

    /// (async) Capture a screenshot of a specific monitor.
    ///
    /// @param {Monitor} monitor - The monitor to capture.
    /// @returns {Promise<ScreenshotResult>} The screenshot data.
    #[napi]
    pub async fn capture_monitor(&self, monitor: Monitor) -> napi::Result<ScreenshotResult> {
        let rust_monitor = terminator::Monitor {
            id: monitor.id,
            name: monitor.name,
            is_primary: monitor.is_primary,
            width: monitor.width,
            height: monitor.height,
            x: monitor.x,
            y: monitor.y,
            scale_factor: monitor.scale_factor,
        };
        self.inner
            .capture_monitor(&rust_monitor)
            .await
            .map(|r| ScreenshotResult {
                width: r.width,
                height: r.height,
                image_data: r.image_data,
                monitor: r.monitor.map(Monitor::from),
            })
            .map_err(map_error)
    }

    /// (async) Capture screenshots of all monitors.
    ///
    /// @returns {Promise<Array<{monitor: Monitor, screenshot: ScreenshotResult}>>} Array of monitor and screenshot pairs.
    #[napi]
    pub async fn capture_all_monitors(&self) -> napi::Result<Vec<MonitorScreenshotPair>> {
        self.inner
            .capture_all_monitors()
            .await
            .map(|results| {
                results
                    .into_iter()
                    .map(|(monitor, screenshot)| MonitorScreenshotPair {
                        monitor: Monitor::from(monitor),
                        screenshot: ScreenshotResult {
                            width: screenshot.width,
                            height: screenshot.height,
                            image_data: screenshot.image_data,
                            monitor: screenshot.monitor.map(Monitor::from),
                        },
                    })
                    .collect()
            })
            .map_err(map_error)
    }
}
