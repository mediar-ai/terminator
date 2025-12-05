use crate::types::{ComputerUseResult, Monitor, MonitorScreenshotPair, WindowTreeResult};
use crate::Selector;
use crate::{
    map_error, CommandOutput, Element, Locator, ScreenshotResult, TreeBuildConfig, UINode,
};
use napi::bindgen_prelude::Either;
use napi_derive::napi;
use std::sync::Once;
use terminator::Desktop as TerminatorDesktop;

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
    ///                              Falls back to RUST_LOG or TERMINATOR_LOG_LEVEL env vars, defaults to 'info'.
    /// @returns {Desktop} A new Desktop automation instance.
    #[napi(constructor)]
    pub fn new(
        use_background_apps: Option<bool>,
        activate_app: Option<bool>,
        log_level: Option<String>,
    ) -> Self {
        let use_background_apps = use_background_apps.unwrap_or(false);
        let activate_app = activate_app.unwrap_or(false);

        // Priority: explicit param > RUST_LOG env > TERMINATOR_LOG_LEVEL env > "info" default
        let log_level = log_level
            .or_else(|| std::env::var("RUST_LOG").ok())
            .or_else(|| std::env::var("TERMINATOR_LOG_LEVEL").ok())
            .unwrap_or_else(|| "info".to_string());

        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(log_level)
                .with_ansi(false) // Disable ANSI color codes for cleaner output
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

    /// (async) Execute a shell command using GitHub Actions-style syntax.
    ///
    /// @param {string} command - The command to run (can be single or multi-line).
    /// @param {string} [shell] - Optional shell to use (defaults to PowerShell on Windows, bash on Unix).
    /// @param {string} [workingDirectory] - Optional working directory for the command.
    /// @returns {Promise<CommandOutput>} The command output.
    #[napi]
    pub async fn run(
        &self,
        command: String,
        shell: Option<String>,
        working_directory: Option<String>,
    ) -> napi::Result<CommandOutput> {
        self.inner
            .run(
                command.as_str(),
                shell.as_deref(),
                working_directory.as_deref(),
            )
            .await
            .map(|r| CommandOutput {
                exit_status: r.exit_status,
                stdout: r.stdout,
                stderr: r.stderr,
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
                work_area: None,
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
    /// @param {string | Selector} selector - The selector.
    /// @returns {Locator} A locator for finding elements.
    #[napi]
    pub fn locator(
        &self,
        #[napi(ts_arg_type = "string | Selector")] selector: Either<String, &Selector>,
    ) -> napi::Result<Locator> {
        use napi::bindgen_prelude::Either::*;
        let sel_rust: terminator::selector::Selector = match selector {
            A(sel_str) => sel_str.as_str().into(),
            B(sel_obj) => sel_obj.inner.clone(),
        };
        let loc = self.inner.locator(sel_rust);
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
    /// @param {string} [browser] - The browser to use. Can be "Default", "Chrome", "Firefox", "Edge", "Brave", "Opera", "Vivaldi", or a custom browser path.
    #[napi]
    pub fn open_url(&self, url: String, browser: Option<String>) -> napi::Result<Element> {
        let browser_enum = browser.map(|b| match b.to_lowercase().as_str() {
            "default" => terminator::Browser::Default,
            "chrome" => terminator::Browser::Chrome,
            "firefox" => terminator::Browser::Firefox,
            "edge" => terminator::Browser::Edge,
            "brave" => terminator::Browser::Brave,
            "opera" => terminator::Browser::Opera,
            "vivaldi" => terminator::Browser::Vivaldi,
            custom => terminator::Browser::Custom(custom.to_string()),
        });
        self.inner
            .open_url(&url, browser_enum)
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

    /// Get the UI tree with full result including formatting and bounds mapping.
    ///
    /// This is the recommended method for getting window trees when you need:
    /// - Formatted YAML output for LLM consumption
    /// - Index-to-bounds mapping for click targeting
    /// - Browser detection
    ///
    /// @param {number} pid - Process ID of the target application.
    /// @param {string} [title] - Optional window title filter.
    /// @param {TreeBuildConfig} [config] - Configuration (set formatOutput: true for formatted output).
    /// @returns {WindowTreeResult} Complete result with tree, formatted output, and bounds mapping.
    #[napi]
    pub fn get_window_tree_result(
        &self,
        pid: u32,
        title: Option<String>,
        config: Option<TreeBuildConfig>,
    ) -> napi::Result<WindowTreeResult> {
        let rust_config = config.map(|c| c.into());
        self.inner
            .get_window_tree_result(pid, title.as_deref(), rust_config)
            .map(WindowTreeResult::from)
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
            work_area: None,
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

    /// (async) Get all window elements for a given application name.
    ///
    /// @param {string} name - The name of the application whose windows will be retrieved.
    /// @returns {Promise<Array<Element>>} A list of window elements belonging to the application.
    #[napi]
    pub async fn windows_for_application(&self, name: String) -> napi::Result<Vec<Element>> {
        self.inner
            .windows_for_application(&name)
            .await
            .map(|windows| windows.into_iter().map(Element::from).collect())
            .map_err(map_error)
    }

    // ============== ADDITIONAL MISSING METHODS ==============

    /// (async) Get the UI tree for all open applications in parallel.
    ///
    /// @returns {Promise<Array<UINode>>} List of UI trees for all applications.
    #[napi]
    pub async fn get_all_applications_tree(&self) -> napi::Result<Vec<UINode>> {
        self.inner
            .get_all_applications_tree()
            .await
            .map(|trees| trees.into_iter().map(UINode::from).collect())
            .map_err(map_error)
    }

    /// (async) Press a key globally.
    ///
    /// @param {string} key - The key to press (e.g., "Enter", "Ctrl+C", "F1").
    #[napi]
    pub async fn press_key(&self, key: String) -> napi::Result<()> {
        self.inner.press_key(&key).await.map_err(map_error)
    }

    /// (async) Execute JavaScript in the currently focused browser tab.
    /// Automatically finds the active browser window and executes the script.
    ///
    /// @param {string} script - The JavaScript code to execute in browser context.
    /// @returns {Promise<string>} The result of script execution.
    #[napi]
    pub async fn execute_browser_script(&self, script: String) -> napi::Result<String> {
        self.inner
            .execute_browser_script(&script)
            .await
            .map_err(map_error)
    }

    /// (async) Delay execution for a specified number of milliseconds.
    /// Useful for waiting between actions to ensure UI stability.
    ///
    /// @param {number} delayMs - Delay in milliseconds.
    /// @returns {Promise<void>}
    #[napi]
    pub async fn delay(&self, delay_ms: u32) -> napi::Result<()> {
        self.inner.delay(delay_ms as u64).await.map_err(map_error)
    }

    /// Navigate to a URL in a browser.
    /// This is the recommended method for browser navigation - more reliable than
    /// manually manipulating the address bar with keyboard/mouse actions.
    ///
    /// @param {string} url - URL to navigate to
    /// @param {string | null} browser - Optional browser name ('Chrome', 'Firefox', 'Edge', 'Brave', 'Opera', 'Vivaldi', or 'Default')
    /// @returns {Promise<Element>} The browser window element
    #[napi]
    pub fn navigate_browser(&self, url: String, browser: Option<String>) -> napi::Result<Element> {
        let browser_enum = browser.map(|b| match b.as_str() {
            "Chrome" => terminator::Browser::Chrome,
            "Firefox" => terminator::Browser::Firefox,
            "Edge" => terminator::Browser::Edge,
            "Brave" => terminator::Browser::Brave,
            "Opera" => terminator::Browser::Opera,
            "Vivaldi" => terminator::Browser::Vivaldi,
            "Default" => terminator::Browser::Default,
            custom => terminator::Browser::Custom(custom.to_string()),
        });

        let element = self.inner.open_url(&url, browser_enum).map_err(map_error)?;
        Ok(Element { inner: element })
    }

    /// (async) Set the zoom level to a specific percentage.
    ///
    /// @param {number} percentage - The zoom percentage (e.g., 100 for 100%, 150 for 150%, 50 for 50%).
    #[napi]
    pub async fn set_zoom(&self, percentage: u32) -> napi::Result<()> {
        self.inner.set_zoom(percentage).await.map_err(map_error)
    }

    /// (async) Run Gemini Computer Use agentic loop.
    ///
    /// Provide a goal and target process, and this will autonomously take actions
    /// (click, type, scroll, etc.) until the goal is achieved or max_steps is reached.
    /// Uses Gemini's vision model to analyze screenshots and decide actions.
    ///
    /// @param {string} process - Process name of the target application (e.g., "chrome", "notepad")
    /// @param {string} goal - What to achieve (e.g., "Open Notepad and type Hello World")
    /// @param {number} [maxSteps=20] - Maximum number of steps before stopping
    /// @returns {Promise<ComputerUseResult>} Result with status, steps executed, and history
    #[napi]
    pub async fn gemini_computer_use(
        &self,
        process: String,
        goal: String,
        max_steps: Option<u32>,
    ) -> napi::Result<ComputerUseResult> {
        self.inner
            .gemini_computer_use(&process, &goal, max_steps, None)
            .await
            .map(ComputerUseResult::from)
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    /// Stop all currently executing operations.
    ///
    /// This cancels the internal cancellation token, which will cause any
    /// operations that check `isCancelled()` to abort. After calling this,
    /// you should create a new Desktop instance to start fresh.
    #[napi]
    pub fn stop_execution(&self) {
        self.inner.stop_execution();
    }

    /// Check if execution has been cancelled.
    ///
    /// Returns `true` if `stopExecution()` has been called.
    /// Long-running operations should periodically check this and abort if true.
    #[napi]
    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }

    /// Stop all active highlight overlays globally.
    ///
    /// This finds and destroys all highlight overlay windows that were created
    /// by `element.highlight()`. Useful for cleaning up highlights without
    /// needing to track individual HighlightHandle objects.
    ///
    /// @returns {number} The number of highlights that were stopped.
    #[napi]
    pub fn stop_highlighting(&self) -> u32 {
        #[cfg(target_os = "windows")]
        {
            terminator::stop_all_highlights() as u32
        }
        #[cfg(not(target_os = "windows"))]
        {
            // Not implemented for other platforms yet
            0
        }
    }

    /// Hide any active inspect overlay.
    ///
    /// This hides the visual overlay that was shown via `get_window_tree` with
    /// `show_overlay` parameter. Can be called from any thread.
    #[napi]
    pub fn hide_inspect_overlay(&self) {
        #[cfg(target_os = "windows")]
        {
            terminator::hide_inspect_overlay();
        }
        #[cfg(not(target_os = "windows"))]
        {
            // Not implemented for other platforms yet
        }
    }
}
