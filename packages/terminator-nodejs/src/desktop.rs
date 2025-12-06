use crate::types::{
    ComputerUseResult, ComputerUseStep, Monitor, MonitorScreenshotPair, TreeOutputFormat,
    WindowTreeResult,
};
use crate::Selector;
use crate::{
    map_error, CommandOutput, Element, Locator, ScreenshotResult, TreeBuildConfig, UINode,
};
use napi::bindgen_prelude::Either;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi_derive::napi;
use std::sync::{Arc, Once};
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

    /// (async) Perform OCR on a window by PID and return structured results with bounding boxes.
    /// Returns an OcrResult containing the OCR tree, formatted output, and index-to-bounds mapping
    /// for click targeting.
    ///
    /// @param {number} pid - Process ID of the target window.
    /// @param {boolean} [formatOutput=true] - Whether to generate formatted compact YAML output.
    /// @returns {Promise<OcrResult>} Complete OCR result with tree, formatted output, and bounds mapping.
    #[napi]
    #[cfg(target_os = "windows")]
    pub async fn perform_ocr_for_process(
        &self,
        pid: u32,
        format_output: Option<bool>,
    ) -> napi::Result<crate::types::OcrResult> {
        let format_output = format_output.unwrap_or(true);

        // Find the application element by PID
        let apps = self.inner.applications().map_err(map_error)?;
        let window_element = apps
            .into_iter()
            .find(|app| app.process_id().ok() == Some(pid))
            .ok_or_else(|| napi::Error::from_reason(format!("No window found for PID {}", pid)))?;

        // Get window bounds (absolute screen coordinates)
        let bounds = window_element.bounds().map_err(map_error)?;
        let (window_x, window_y, win_w, win_h) = bounds;

        // Capture screenshot of the window
        let screenshot = window_element.capture().map_err(map_error)?;

        // Calculate DPI scale factors (physical screenshot pixels / logical window size)
        let dpi_scale_w = screenshot.width as f64 / win_w;
        let dpi_scale_h = screenshot.height as f64 / win_h;

        // Perform OCR with bounding boxes
        let ocr_element = self
            .inner
            .ocr_screenshot_with_bounds(&screenshot, window_x, window_y, dpi_scale_w, dpi_scale_h)
            .map_err(map_error)?;

        // Format the OCR tree if requested
        let (formatted, index_to_bounds) = if format_output {
            let result = terminator::format_ocr_tree_as_compact_yaml(&ocr_element, 0);
            let bounds_map: std::collections::HashMap<String, crate::types::OcrBoundsEntry> = result
                .index_to_bounds
                .into_iter()
                .map(|(idx, (text, (x, y, w, h)))| {
                    (
                        idx.to_string(),
                        crate::types::OcrBoundsEntry {
                            text,
                            bounds: crate::types::Bounds {
                                x,
                                y,
                                width: w,
                                height: h,
                            },
                        },
                    )
                })
                .collect();
            (Some(result.formatted), bounds_map)
        } else {
            (None, std::collections::HashMap::new())
        };

        let element_count = index_to_bounds.len() as u32;

        Ok(crate::types::OcrResult {
            tree: crate::types::OcrElement::from(ocr_element),
            formatted,
            index_to_bounds,
            element_count,
        })
    }

    /// (async) Perform OCR on a window by PID (non-Windows stub).
    #[napi]
    #[cfg(not(target_os = "windows"))]
    pub async fn perform_ocr_for_process(
        &self,
        _pid: u32,
        _format_output: Option<bool>,
    ) -> napi::Result<crate::types::OcrResult> {
        Err(napi::Error::from_reason(
            "OCR with bounding boxes is currently only supported on Windows",
        ))
    }

    /// (async) Capture DOM elements from the current browser tab.
    ///
    /// Extracts visible DOM elements with their properties and screen coordinates.
    /// Uses JavaScript injection via Chrome extension to traverse the DOM tree.
    ///
    /// @param {number} [maxElements=200] - Maximum number of elements to capture.
    /// @param {boolean} [formatOutput=true] - Whether to include formatted compact YAML output.
    /// @returns {Promise<BrowserDomResult>} DOM elements with bounds for click targeting.
    #[napi]
    pub async fn capture_browser_dom(
        &self,
        max_elements: Option<u32>,
        format_output: Option<bool>,
    ) -> napi::Result<crate::types::BrowserDomResult> {
        use std::collections::HashMap;
        use std::time::Duration;

        let max_elements = max_elements.unwrap_or(200);
        let format_output = format_output.unwrap_or(true);

        // Get viewport offset from Document element (more reliable than JS due to DPI scaling)
        let viewport_offset = match self
            .inner
            .locator("role:Document")
            .first(Some(Duration::from_millis(2000)))
            .await
        {
            Ok(doc_element) => match doc_element.bounds() {
                Ok((x, y, _w, _h)) => (x, y),
                Err(_) => (0.0, 0.0),
            },
            Err(_) => (0.0, 0.0),
        };

        // JavaScript to extract visible DOM elements
        let script = format!(
            r#"
(function() {{
    const elements = [];
    const maxElements = {max_elements};

    const walker = document.createTreeWalker(
        document.body,
        NodeFilter.SHOW_ELEMENT,
        {{
            acceptNode: function(node) {{
                const style = window.getComputedStyle(node);
                const rect = node.getBoundingClientRect();

                if (style.display === 'none' ||
                    style.visibility === 'hidden' ||
                    style.opacity === '0' ||
                    rect.width === 0 ||
                    rect.height === 0) {{
                    return NodeFilter.FILTER_SKIP;
                }}

                return NodeFilter.FILTER_ACCEPT;
            }}
        }}
    );

    let node;
    while (node = walker.nextNode()) {{
        if (elements.length >= maxElements) {{
            break;
        }}

        const rect = node.getBoundingClientRect();
        const text = node.innerText ? node.innerText.substring(0, 100).trim() : null;

        elements.push({{
            tag: node.tagName.toLowerCase(),
            id: node.id || null,
            classes: Array.from(node.classList),
            text: text,
            href: node.href || null,
            type: node.type || null,
            name: node.name || null,
            value: node.value || null,
            placeholder: node.placeholder || null,
            aria_label: node.getAttribute('aria-label'),
            role: node.getAttribute('role'),
            x: Math.round(rect.x * window.devicePixelRatio),
            y: Math.round(rect.y * window.devicePixelRatio),
            width: Math.round(rect.width * window.devicePixelRatio),
            height: Math.round(rect.height * window.devicePixelRatio)
        }});
    }}

    return JSON.stringify({{
        elements: elements,
        total_found: elements.length,
        page_url: window.location.href,
        page_title: document.title,
        devicePixelRatio: window.devicePixelRatio
    }});
}})()"#
        );

        let result_str = self
            .inner
            .execute_browser_script(&script)
            .await
            .map_err(map_error)?;

        let parsed: serde_json::Value = serde_json::from_str(&result_str)
            .map_err(|e| napi::Error::from_reason(format!("Failed to parse DOM result: {e}")))?;

        let page_url = parsed
            .get("page_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let page_title = parsed
            .get("page_title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let raw_elements = parsed
            .get("elements")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Convert to BrowserDomElement and build index_to_bounds
        let mut elements = Vec::new();
        let mut index_to_bounds: HashMap<String, crate::types::DomBoundsEntry> = HashMap::new();
        let mut formatted_lines: Vec<String> = Vec::new();

        if format_output {
            formatted_lines.push(format!(
                "Browser DOM: {} elements (url: {}, title: {})",
                raw_elements.len(),
                page_url,
                page_title
            ));
        }

        for (i, elem) in raw_elements.iter().enumerate() {
            let idx = i + 1;
            let tag = elem.get("tag").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let id = elem.get("id").and_then(|v| v.as_str()).map(String::from);
            let classes: Vec<String> = elem
                .get("classes")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|c| c.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let text = elem.get("text").and_then(|v| v.as_str()).map(String::from);
            let href = elem.get("href").and_then(|v| v.as_str()).map(String::from);
            let r#type = elem.get("type").and_then(|v| v.as_str()).map(String::from);
            let name = elem.get("name").and_then(|v| v.as_str()).map(String::from);
            let value = elem.get("value").and_then(|v| v.as_str()).map(String::from);
            let placeholder = elem.get("placeholder").and_then(|v| v.as_str()).map(String::from);
            let aria_label = elem.get("aria_label").and_then(|v| v.as_str()).map(String::from);
            let role = elem.get("role").and_then(|v| v.as_str()).map(String::from);

            // Build bounds with viewport offset added
            let x = elem.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) + viewport_offset.0;
            let y = elem.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) + viewport_offset.1;
            let width = elem.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let height = elem.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);

            let bounds = crate::types::Bounds { x, y, width, height };

            // Display name for index_to_bounds
            let display_name = text
                .as_ref()
                .filter(|t| !t.is_empty())
                .cloned()
                .or_else(|| aria_label.clone())
                .or_else(|| placeholder.clone())
                .or_else(|| name.clone())
                .or_else(|| id.clone())
                .unwrap_or_else(|| format!("<{}>", tag));

            // Format line for compact YAML
            if format_output {
                let mut line_parts = vec![format!("#{} [{}]", idx, tag.to_uppercase())];
                if let Some(ref t) = text {
                    if !t.is_empty() {
                        let truncated = if t.len() > 40 { format!("{}...", &t[..40]) } else { t.clone() };
                        line_parts.push(truncated);
                    }
                }
                if let Some(ref a) = aria_label {
                    line_parts.push(format!("aria:{}", a));
                }
                if let Some(ref r) = role {
                    line_parts.push(format!("role:{}", r));
                }
                formatted_lines.push(format!("  {}", line_parts.join(" ")));
            }

            index_to_bounds.insert(
                idx.to_string(),
                crate::types::DomBoundsEntry {
                    name: display_name,
                    tag: tag.clone(),
                    bounds: bounds.clone(),
                },
            );

            elements.push(crate::types::BrowserDomElement {
                tag,
                id,
                classes,
                text,
                href,
                r#type,
                name,
                value,
                placeholder,
                aria_label,
                role,
                bounds,
            });
        }

        Ok(crate::types::BrowserDomResult {
            elements,
            formatted: if format_output { Some(formatted_lines.join("\n")) } else { None },
            index_to_bounds,
            element_count: raw_elements.len() as u32,
            page_url,
            page_title,
        })
    }

    /// (async) Get a clustered tree combining elements from multiple sources grouped by spatial proximity.
    ///
    /// Combines accessibility tree (UIA) elements with optional DOM and Omniparser elements,
    /// clustering nearby elements together. Each element is prefixed with its source:
    /// - #u1, #u2... for UIA (accessibility tree)
    /// - #d1, #d2... for DOM (browser content)
    /// - #p1, #p2... for Omniparser (vision AI detection)
    ///
    /// @param {number} pid - Process ID of the window to analyze.
    /// @param {number} [maxDomElements=100] - Maximum DOM elements to capture for browsers.
    /// @param {boolean} [includeOmniparser=false] - Whether to include Omniparser vision detection.
    /// @returns {Promise<ClusteredFormattingResult>} Clustered tree with prefixed indices.
    #[napi]
    pub async fn get_clustered_tree(
        &self,
        pid: u32,
        max_dom_elements: Option<u32>,
        include_omniparser: Option<bool>,
    ) -> napi::Result<crate::types::ClusteredFormattingResult> {
        use std::collections::HashMap;

        let max_dom_elements = max_dom_elements.unwrap_or(100);
        let include_omniparser = include_omniparser.unwrap_or(false);

        // Get UIA tree with bounds
        let uia_result = self
            .inner
            .get_window_tree_result(pid, None, None)
            .map_err(map_error)?;

        // Build UIA bounds cache: HashMap<u32, (role, name, bounds, selector)>
        let mut uia_bounds: HashMap<u32, (String, String, (f64, f64, f64, f64), Option<String>)> =
            HashMap::new();

        // Use the formatted result to extract bounds
        let formatted_result = terminator::format_ui_node_as_compact_yaml(&uia_result.tree, 0);
        for (idx, (role, name, bounds, selector)) in formatted_result.index_to_bounds {
            uia_bounds.insert(idx, (role, name, bounds, selector));
        }

        // Check if this is a browser
        let is_browser = terminator::is_browser_process(pid);

        // Build DOM bounds cache: HashMap<u32, (tag, identifier, bounds)>
        let mut dom_bounds: HashMap<u32, (String, String, (f64, f64, f64, f64))> = HashMap::new();

        if is_browser {
            // Try to capture DOM elements
            match self.capture_browser_dom(Some(max_dom_elements), Some(true)).await {
                Ok(dom_result) => {
                    for (idx_str, entry) in dom_result.index_to_bounds {
                        if let Ok(idx) = idx_str.parse::<u32>() {
                            let bounds = (
                                entry.bounds.x,
                                entry.bounds.y,
                                entry.bounds.width,
                                entry.bounds.height,
                            );
                            dom_bounds.insert(idx, (entry.tag, entry.name, bounds));
                        }
                    }
                }
                Err(_) => {
                    // DOM capture failed (e.g., chrome:// page), continue with UIA only
                }
            }
        }

        // Build Omniparser items cache if requested
        let mut omniparser_items: HashMap<u32, terminator::OmniparserItem> = HashMap::new();

        if include_omniparser {
            match self.perform_omniparser_for_process(pid, None, Some(true)).await {
                Ok(omni_result) => {
                    for (idx_str, entry) in omni_result.index_to_bounds {
                        if let Ok(idx) = idx_str.parse::<u32>() {
                            omniparser_items.insert(
                                idx,
                                terminator::OmniparserItem {
                                    label: entry.label.clone(),
                                    content: Some(entry.name.clone()),
                                    box_2d: Some([
                                        entry.bounds.x,
                                        entry.bounds.y,
                                        entry.bounds.x + entry.bounds.width,
                                        entry.bounds.y + entry.bounds.height,
                                    ]),
                                },
                            );
                        }
                    }
                }
                Err(_) => {
                    // Omniparser failed, continue without it
                }
            }
        }

        // Empty caches for sources we don't have yet
        let ocr_bounds: HashMap<u32, (String, (f64, f64, f64, f64))> = HashMap::new();
        let vision_items: HashMap<u32, terminator::VisionElement> = HashMap::new();

        // Call the core clustering function
        let clustered_result = terminator::format_clustered_tree_from_caches(
            &uia_bounds,
            &dom_bounds,
            &ocr_bounds,
            &omniparser_items,
            &vision_items,
        );

        // Convert to SDK types
        let mut index_to_source_and_bounds: HashMap<String, crate::types::ClusteredBoundsEntry> =
            HashMap::new();

        for (key, (source, original_idx, (x, y, w, h))) in clustered_result.index_to_source_and_bounds
        {
            let sdk_source = match source {
                terminator::ElementSource::Uia => crate::types::ElementSource::Uia,
                terminator::ElementSource::Dom => crate::types::ElementSource::Dom,
                terminator::ElementSource::Ocr => crate::types::ElementSource::Ocr,
                terminator::ElementSource::Omniparser => crate::types::ElementSource::Omniparser,
                terminator::ElementSource::Gemini => crate::types::ElementSource::Gemini,
            };
            index_to_source_and_bounds.insert(
                key,
                crate::types::ClusteredBoundsEntry {
                    source: sdk_source,
                    original_index: original_idx,
                    bounds: crate::types::Bounds {
                        x,
                        y,
                        width: w,
                        height: h,
                    },
                },
            );
        }

        Ok(crate::types::ClusteredFormattingResult {
            formatted: clustered_result.formatted,
            index_to_source_and_bounds,
        })
    }

    /// (async) Perform Gemini vision AI detection on a window by PID.
    ///
    /// Captures a screenshot and sends it to the Gemini vision backend for UI element detection.
    /// Requires GEMINI_VISION_BACKEND_URL environment variable (defaults to https://app.mediar.ai/api/vision/parse).
    ///
    /// @param {number} pid - Process ID of the window to capture.
    /// @param {boolean} [formatOutput=true] - Whether to include formatted compact YAML output.
    /// @returns {Promise<GeminiVisionResult>} Detected UI elements with bounds for click targeting.
    #[napi]
    pub async fn perform_gemini_vision_for_process(
        &self,
        pid: u32,
        format_output: Option<bool>,
    ) -> napi::Result<crate::types::GeminiVisionResult> {
        use base64::{engine::general_purpose, Engine};
        use image::{codecs::png::PngEncoder, ExtendedColorType, ImageBuffer, ImageEncoder, Rgba};
        use image::imageops::FilterType;
        use std::collections::HashMap;
        use std::io::Cursor;

        let format_output = format_output.unwrap_or(true);

        // Find the window element for this process
        let apps = self.inner.applications().map_err(map_error)?;
        let window_element = apps
            .into_iter()
            .find(|app| app.process_id().ok() == Some(pid))
            .ok_or_else(|| napi::Error::from_reason(format!("No window found for PID {}", pid)))?;

        // Get window bounds
        let bounds = window_element.bounds().map_err(map_error)?;
        let (window_x, window_y, win_w, win_h) = bounds;

        // Capture screenshot
        let screenshot = window_element.capture().map_err(map_error)?;
        let original_width = screenshot.width;
        let original_height = screenshot.height;

        // Calculate DPI scale
        let dpi_scale_w = original_width as f64 / win_w;
        let dpi_scale_h = original_height as f64 / win_h;

        // Convert BGRA to RGBA
        let rgba_data: Vec<u8> = screenshot
            .image_data
            .chunks_exact(4)
            .flat_map(|bgra| [bgra[2], bgra[1], bgra[0], bgra[3]])
            .collect();

        // Resize if needed (max 1920px)
        const MAX_DIM: u32 = 1920;
        let (final_width, final_height, final_rgba_data, scale_factor) =
            if original_width > MAX_DIM || original_height > MAX_DIM {
                let scale = (MAX_DIM as f32 / original_width.max(original_height) as f32).min(1.0);
                let new_width = (original_width as f32 * scale).round() as u32;
                let new_height = (original_height as f32 * scale).round() as u32;

                let img = ImageBuffer::<Rgba<u8>, _>::from_raw(original_width, original_height, rgba_data)
                    .ok_or_else(|| napi::Error::from_reason("Failed to create image buffer"))?;

                let resized = image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);
                (new_width, new_height, resized.into_raw(), scale as f64)
            } else {
                (original_width, original_height, rgba_data, 1.0)
            };

        // Encode to PNG
        let mut png_data = Vec::new();
        let encoder = PngEncoder::new(Cursor::new(&mut png_data));
        encoder
            .write_image(&final_rgba_data, final_width, final_height, ExtendedColorType::Rgba8)
            .map_err(|e| napi::Error::from_reason(format!("Failed to encode PNG: {e}")))?;

        let base64_image = general_purpose::STANDARD.encode(&png_data);

        // Call Gemini Vision backend
        let backend_url = std::env::var("GEMINI_VISION_BACKEND_URL")
            .unwrap_or_else(|_| "https://app.mediar.ai/api/vision/parse".to_string());

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| napi::Error::from_reason(format!("Failed to create HTTP client: {e}")))?;

        let payload = serde_json::json!({
            "image": base64_image,
            "model": "gemini",
            "prompt": "Detect all UI elements in this screenshot. Return their type, content, description, bounding boxes, and interactivity."
        });

        let resp = client
            .post(&backend_url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| napi::Error::from_reason(format!("Vision backend request failed: {e}")))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(napi::Error::from_reason(format!("Vision backend error: {}", text)));
        }

        let response_text = resp.text().await
            .map_err(|e| napi::Error::from_reason(format!("Failed to read response: {e}")))?;

        let parsed: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| napi::Error::from_reason(format!("Failed to parse response: {e}")))?;

        if let Some(error) = parsed.get("error").and_then(|v| v.as_str()) {
            return Err(napi::Error::from_reason(format!("Vision error: {}", error)));
        }

        let raw_elements = parsed
            .get("elements")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Convert to VisionElement with absolute screen coordinates
        let mut elements = Vec::new();
        let mut index_to_bounds: HashMap<String, crate::types::VisionBoundsEntry> = HashMap::new();
        let mut formatted_lines: Vec<String> = Vec::new();

        if format_output {
            formatted_lines.push(format!("Gemini Vision: {} elements (PID: {})", raw_elements.len(), pid));
        }

        let inv_scale = 1.0 / scale_factor;

        for (i, elem) in raw_elements.iter().enumerate() {
            let idx = i + 1;
            let element_type = elem.get("type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            let content = elem.get("content").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(String::from);
            let description = elem.get("description").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(String::from);
            let interactivity = elem.get("interactivity").and_then(|v| v.as_bool());

            // Get normalized bbox [x1, y1, x2, y2] from 0-1
            let bbox = elem.get("bbox").and_then(|v| v.as_array());
            let bounds = bbox.and_then(|arr| {
                if arr.len() >= 4 {
                    let x1 = arr[0].as_f64()? * final_width as f64;
                    let y1 = arr[1].as_f64()? * final_height as f64;
                    let x2 = arr[2].as_f64()? * final_width as f64;
                    let y2 = arr[3].as_f64()? * final_height as f64;

                    // Scale back to original size and convert to logical screen coords
                    let abs_x = window_x + (x1 * inv_scale / dpi_scale_w);
                    let abs_y = window_y + (y1 * inv_scale / dpi_scale_h);
                    let abs_w = (x2 - x1) * inv_scale / dpi_scale_w;
                    let abs_h = (y2 - y1) * inv_scale / dpi_scale_h;

                    Some(crate::types::Bounds {
                        x: abs_x,
                        y: abs_y,
                        width: abs_w,
                        height: abs_h,
                    })
                } else {
                    None
                }
            });

            // Display name for index_to_bounds
            let display_name = content
                .as_ref()
                .cloned()
                .or_else(|| description.clone())
                .unwrap_or_else(|| format!("<{}>", element_type));

            // Format line for compact YAML
            if format_output {
                let mut line_parts = vec![format!("#{} [{}]", idx, element_type.to_uppercase())];
                if let Some(ref c) = content {
                    let truncated = if c.len() > 40 { format!("{}...", &c[..40]) } else { c.clone() };
                    line_parts.push(truncated);
                }
                if let Some(ref d) = description {
                    let truncated = if d.len() > 30 { format!("{}...", &d[..30]) } else { d.clone() };
                    line_parts.push(format!("desc:{}", truncated));
                }
                if interactivity == Some(true) {
                    line_parts.push("interactive".to_string());
                }
                formatted_lines.push(format!("  {}", line_parts.join(" ")));
            }

            if let Some(ref b) = bounds {
                index_to_bounds.insert(
                    idx.to_string(),
                    crate::types::VisionBoundsEntry {
                        name: display_name.clone(),
                        element_type: element_type.clone(),
                        bounds: b.clone(),
                    },
                );
            }

            elements.push(crate::types::VisionElement {
                element_type,
                content,
                description,
                bounds,
                interactivity,
            });
        }

        Ok(crate::types::GeminiVisionResult {
            elements,
            formatted: if format_output { Some(formatted_lines.join("\n")) } else { None },
            index_to_bounds,
            element_count: raw_elements.len() as u32,
        })
    }

    /// (async) Perform Omniparser V2 detection on a window by PID.
    ///
    /// Captures a screenshot and sends it to the Omniparser backend for icon/field detection.
    /// Requires OMNIPARSER_BACKEND_URL environment variable (defaults to https://app.mediar.ai/api/omniparser/parse).
    ///
    /// @param {number} pid - Process ID of the window to capture.
    /// @param {number} [imgsz=1920] - Icon detection image size (640-1920). Higher = better but slower.
    /// @param {boolean} [formatOutput=true] - Whether to include formatted compact YAML output.
    /// @returns {Promise<OmniparserResult>} Detected items with bounds for click targeting.
    #[napi]
    pub async fn perform_omniparser_for_process(
        &self,
        pid: u32,
        imgsz: Option<u32>,
        format_output: Option<bool>,
    ) -> napi::Result<crate::types::OmniparserResult> {
        use base64::{engine::general_purpose, Engine};
        use image::{codecs::png::PngEncoder, ExtendedColorType, ImageBuffer, ImageEncoder, Rgba};
        use image::imageops::FilterType;
        use std::collections::HashMap;
        use std::io::Cursor;

        let imgsz = imgsz.unwrap_or(1920).clamp(640, 1920);
        let format_output = format_output.unwrap_or(true);

        // Find the window element for this process
        let apps = self.inner.applications().map_err(map_error)?;
        let window_element = apps
            .into_iter()
            .find(|app| app.process_id().ok() == Some(pid))
            .ok_or_else(|| napi::Error::from_reason(format!("No window found for PID {}", pid)))?;

        // Get window bounds
        let bounds = window_element.bounds().map_err(map_error)?;
        let (window_x, window_y, win_w, win_h) = bounds;

        // Capture screenshot
        let screenshot = window_element.capture().map_err(map_error)?;
        let original_width = screenshot.width;
        let original_height = screenshot.height;

        // Calculate DPI scale
        let dpi_scale_w = original_width as f64 / win_w;
        let dpi_scale_h = original_height as f64 / win_h;

        // Convert BGRA to RGBA
        let rgba_data: Vec<u8> = screenshot
            .image_data
            .chunks_exact(4)
            .flat_map(|bgra| [bgra[2], bgra[1], bgra[0], bgra[3]])
            .collect();

        // Resize if needed (max 1920px)
        const MAX_DIM: u32 = 1920;
        let (final_width, final_height, final_rgba_data, scale_factor) =
            if original_width > MAX_DIM || original_height > MAX_DIM {
                let scale = (MAX_DIM as f32 / original_width.max(original_height) as f32).min(1.0);
                let new_width = (original_width as f32 * scale).round() as u32;
                let new_height = (original_height as f32 * scale).round() as u32;

                let img = ImageBuffer::<Rgba<u8>, _>::from_raw(original_width, original_height, rgba_data)
                    .ok_or_else(|| napi::Error::from_reason("Failed to create image buffer"))?;

                let resized = image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);
                (new_width, new_height, resized.into_raw(), scale as f64)
            } else {
                (original_width, original_height, rgba_data, 1.0)
            };

        // Encode to PNG
        let mut png_data = Vec::new();
        let encoder = PngEncoder::new(Cursor::new(&mut png_data));
        encoder
            .write_image(&final_rgba_data, final_width, final_height, ExtendedColorType::Rgba8)
            .map_err(|e| napi::Error::from_reason(format!("Failed to encode PNG: {e}")))?;

        let base64_image = general_purpose::STANDARD.encode(&png_data);

        // Call Omniparser backend
        let backend_url = std::env::var("OMNIPARSER_BACKEND_URL")
            .unwrap_or_else(|_| "https://app.mediar.ai/api/omniparser/parse".to_string());

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| napi::Error::from_reason(format!("Failed to create HTTP client: {e}")))?;

        let payload = serde_json::json!({
            "image": base64_image,
            "imgsz": imgsz
        });

        let resp = client
            .post(&backend_url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| napi::Error::from_reason(format!("Omniparser backend request failed: {e}")))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(napi::Error::from_reason(format!("Omniparser backend error: {}", text)));
        }

        let response_text = resp.text().await
            .map_err(|e| napi::Error::from_reason(format!("Failed to read response: {e}")))?;

        let parsed: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| napi::Error::from_reason(format!("Failed to parse response: {e}")))?;

        if let Some(error) = parsed.get("error").and_then(|v| v.as_str()) {
            return Err(napi::Error::from_reason(format!("Omniparser error: {}", error)));
        }

        let raw_elements = parsed
            .get("elements")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Convert to OmniparserItem with absolute screen coordinates
        let mut items = Vec::new();
        let mut index_to_bounds: HashMap<String, crate::types::OmniparserBoundsEntry> = HashMap::new();
        let mut formatted_lines: Vec<String> = Vec::new();

        if format_output {
            formatted_lines.push(format!("Omniparser: {} items (PID: {})", raw_elements.len(), pid));
        }

        let inv_scale = 1.0 / scale_factor;

        for (i, elem) in raw_elements.iter().enumerate() {
            let idx = i + 1;
            let label = elem.get("type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            let content = elem.get("content").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(String::from);

            // Get normalized bbox [x1, y1, x2, y2] from 0-1
            let bbox = elem.get("bbox").and_then(|v| v.as_array());
            let bounds = bbox.and_then(|arr| {
                if arr.len() >= 4 {
                    let x1 = arr[0].as_f64()? * final_width as f64;
                    let y1 = arr[1].as_f64()? * final_height as f64;
                    let x2 = arr[2].as_f64()? * final_width as f64;
                    let y2 = arr[3].as_f64()? * final_height as f64;

                    // Scale back to original size and convert to logical screen coords
                    let abs_x = window_x + (x1 * inv_scale / dpi_scale_w);
                    let abs_y = window_y + (y1 * inv_scale / dpi_scale_h);
                    let abs_w = (x2 - x1) * inv_scale / dpi_scale_w;
                    let abs_h = (y2 - y1) * inv_scale / dpi_scale_h;

                    Some(crate::types::Bounds {
                        x: abs_x,
                        y: abs_y,
                        width: abs_w,
                        height: abs_h,
                    })
                } else {
                    None
                }
            });

            // Display name for index_to_bounds
            let display_name = content
                .as_ref()
                .cloned()
                .unwrap_or_else(|| format!("<{}>", label));

            // Format line for compact YAML
            if format_output {
                let mut line_parts = vec![format!("#{} [{}]", idx, label.to_uppercase())];
                if let Some(ref c) = content {
                    let truncated = if c.len() > 50 { format!("{}...", &c[..50]) } else { c.clone() };
                    line_parts.push(truncated);
                }
                formatted_lines.push(format!("  {}", line_parts.join(" ")));
            }

            if let Some(ref b) = bounds {
                index_to_bounds.insert(
                    idx.to_string(),
                    crate::types::OmniparserBoundsEntry {
                        name: display_name.clone(),
                        label: label.clone(),
                        bounds: b.clone(),
                    },
                );
            }

            items.push(crate::types::OmniparserItem {
                label,
                content,
                bounds,
            });
        }

        Ok(crate::types::OmniparserResult {
            items,
            formatted: if format_output { Some(formatted_lines.join("\n")) } else { None },
            index_to_bounds,
            item_count: raw_elements.len() as u32,
        })
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
    /// @param {TreeBuildConfig} [config] - Configuration options:
    ///   - formatOutput: Enable formatted output (default: true if treeOutputFormat set)
    ///   - treeOutputFormat: 'CompactYaml' (default) or 'VerboseJson'
    ///   - treeFromSelector: Selector to start tree from (use getWindowTreeResultAsync for this)
    /// @returns {WindowTreeResult} Complete result with tree, formatted output, and bounds mapping.
    #[napi]
    pub fn get_window_tree_result(
        &self,
        pid: u32,
        title: Option<String>,
        config: Option<TreeBuildConfig>,
    ) -> napi::Result<WindowTreeResult> {
        // Extract options before converting config
        let output_format = config
            .as_ref()
            .and_then(|c| c.tree_output_format.clone())
            .unwrap_or(TreeOutputFormat::CompactYaml);

        // If format is VerboseJson, we don't need formatted output from core
        // ClusteredYaml is treated like CompactYaml (needs format_output = true)
        let rust_config = config.map(|mut c| {
            if matches!(output_format, TreeOutputFormat::VerboseJson) {
                c.format_output = Some(false);
            } else if c.format_output.is_none() {
                c.format_output = Some(true);
            }
            c.into()
        });

        let result = self
            .inner
            .get_window_tree_result(pid, title.as_deref(), rust_config)
            .map_err(map_error)?;

        // Convert and handle format
        let mut sdk_result = WindowTreeResult::from(result);

        // For VerboseJson, serialize the tree as the formatted output
        if matches!(output_format, TreeOutputFormat::VerboseJson) {
            sdk_result.formatted =
                Some(serde_json::to_string_pretty(&sdk_result.tree).unwrap_or_default());
        }

        Ok(sdk_result)
    }

    /// (async) Get the UI tree with full result, supporting tree_from_selector.
    ///
    /// Use this method when you need to scope the tree to a specific subtree using a selector.
    ///
    /// @param {number} pid - Process ID of the target application.
    /// @param {string} [title] - Optional window title filter.
    /// @param {TreeBuildConfig} [config] - Configuration options:
    ///   - formatOutput: Enable formatted output (default: true)
    ///   - treeOutputFormat: 'CompactYaml' (default) or 'VerboseJson'
    ///   - treeFromSelector: Selector to start tree from (e.g., "role:Dialog")
    /// @returns {Promise<WindowTreeResult>} Complete result with tree, formatted output, and bounds mapping.
    #[napi]
    pub async fn get_window_tree_result_async(
        &self,
        pid: u32,
        title: Option<String>,
        config: Option<TreeBuildConfig>,
    ) -> napi::Result<WindowTreeResult> {
        // Extract options before converting config
        let output_format = config
            .as_ref()
            .and_then(|c| c.tree_output_format.clone())
            .unwrap_or(TreeOutputFormat::CompactYaml);

        let tree_from_selector = config
            .as_ref()
            .and_then(|c| c.tree_from_selector.clone());

        let max_depth = config
            .as_ref()
            .and_then(|c| c.max_depth)
            .unwrap_or(100) as usize;

        // If tree_from_selector is provided, find the app by PID and search within it
        if let Some(selector_str) = tree_from_selector {
            // First, find the application element by PID
            let apps = self.inner.applications().map_err(map_error)?;
            let app_element = apps
                .into_iter()
                .find(|app| app.process_id().ok() == Some(pid))
                .ok_or_else(|| {
                    napi::Error::from_reason(format!(
                        "No application found with PID {}",
                        pid
                    ))
                })?;

            // Now search within this application element
            let selector = terminator::Selector::from(selector_str.as_str());
            let locator = app_element.locator(selector).map_err(map_error)?;

            let element = locator
                .first(Some(std::time::Duration::from_millis(2000)))
                .await
                .map_err(map_error)?;

            // Get the subtree from this element
            let serializable_tree = element.to_serializable_tree(max_depth);
            let tree = crate::types::serializable_to_ui_node(&serializable_tree);

            // Format based on output format
            let formatted = match output_format {
                TreeOutputFormat::VerboseJson => {
                    Some(serde_json::to_string_pretty(&tree).unwrap_or_default())
                }
                TreeOutputFormat::CompactYaml => {
                    // Use the core formatter
                    let result = terminator::format_tree_as_compact_yaml(&serializable_tree, 0);
                    Some(result.formatted)
                }
                TreeOutputFormat::ClusteredYaml => {
                    // ClusteredYaml requires additional data sources (DOM, OCR, Omniparser, Vision)
                    // For now, fall back to CompactYaml since we only have UIA data here.
                    // Use format_clustered_tree_from_caches when you have all data sources.
                    let result = terminator::format_tree_as_compact_yaml(&serializable_tree, 0);
                    Some(result.formatted)
                }
            };

            // Build index_to_bounds from the formatted result
            let index_to_bounds = if matches!(
                output_format,
                TreeOutputFormat::CompactYaml | TreeOutputFormat::ClusteredYaml
            ) {
                let result = terminator::format_tree_as_compact_yaml(&serializable_tree, 0);
                result
                    .index_to_bounds
                    .into_iter()
                    .map(|(idx, (role, name, (x, y, w, h), selector))| {
                        (
                            idx.to_string(),
                            crate::types::BoundsEntry {
                                role,
                                name,
                                bounds: crate::types::Bounds {
                                    x,
                                    y,
                                    width: w,
                                    height: h,
                                },
                                selector,
                            },
                        )
                    })
                    .collect()
            } else {
                std::collections::HashMap::new()
            };

            let element_count = index_to_bounds.len() as u32;
            let is_browser = terminator::is_browser_process(pid);

            return Ok(WindowTreeResult {
                tree,
                pid,
                is_browser,
                formatted,
                index_to_bounds,
                element_count,
            });
        }

        // No tree_from_selector, use standard method
        let rust_config = config.map(|mut c| {
            if matches!(output_format, TreeOutputFormat::VerboseJson) {
                c.format_output = Some(false);
            } else if c.format_output.is_none() {
                c.format_output = Some(true);
            }
            c.into()
        });

        let result = self
            .inner
            .get_window_tree_result(pid, title.as_deref(), rust_config)
            .map_err(map_error)?;

        let mut sdk_result = WindowTreeResult::from(result);

        if matches!(output_format, TreeOutputFormat::VerboseJson) {
            sdk_result.formatted =
                Some(serde_json::to_string_pretty(&sdk_result.tree).unwrap_or_default());
        }

        Ok(sdk_result)
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
    /// @param {function} [onStep] - Optional callback invoked after each step with step details
    /// @returns {Promise<ComputerUseResult>} Result with status, steps executed, and history
    #[napi]
    pub async fn gemini_computer_use(
        &self,
        process: String,
        goal: String,
        max_steps: Option<u32>,
        #[napi(ts_arg_type = "((err: null | Error, step: ComputerUseStep) => void) | undefined")]
        on_step: Option<ThreadsafeFunction<ComputerUseStep>>,
    ) -> napi::Result<ComputerUseResult> {
        // Create progress callback if onStep is provided
        let progress_callback: Option<Box<dyn Fn(&terminator::ComputerUseStep) + Send + Sync>> =
            on_step.map(|tsfn| {
                let tsfn = Arc::new(tsfn);
                Box::new(move |step: &terminator::ComputerUseStep| {
                    let js_step = ComputerUseStep::from(step.clone());
                    tsfn.call(Ok(js_step), ThreadsafeFunctionCallMode::NonBlocking);
                }) as Box<dyn Fn(&terminator::ComputerUseStep) + Send + Sync>
            });

        self.inner
            .gemini_computer_use(&process, &goal, max_steps, progress_callback)
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

    /// Show inspect overlay with indexed elements for visual debugging.
    ///
    /// Displays a transparent overlay window with colored rectangles around UI elements,
    /// showing their index numbers for click targeting. Use `hideInspectOverlay()` to remove.
    ///
    /// @param {InspectElement[]} elements - Array of elements to highlight with their bounds.
    /// @param {object} windowBounds - The window bounds {x, y, width, height} to constrain the overlay.
    /// @param {OverlayDisplayMode} [displayMode='Index'] - What to show in labels: 'Index', 'Role', 'Name', etc.
    #[napi]
    #[cfg(target_os = "windows")]
    pub fn show_inspect_overlay(
        &self,
        elements: Vec<crate::types::InspectElement>,
        window_bounds: crate::types::Bounds,
        display_mode: Option<crate::types::OverlayDisplayMode>,
    ) -> napi::Result<()> {
        let core_elements: Vec<terminator::InspectElement> =
            elements.into_iter().map(|e| e.into()).collect();
        let core_bounds = (
            window_bounds.x as i32,
            window_bounds.y as i32,
            window_bounds.width as i32,
            window_bounds.height as i32,
        );
        let core_mode = display_mode
            .map(|m| m.into())
            .unwrap_or(terminator::OverlayDisplayMode::Index);

        terminator::show_inspect_overlay(core_elements, core_bounds, core_mode)
            .map(|_handle| ()) // Discard handle - use hideInspectOverlay to close
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    /// Show inspect overlay (non-Windows stub).
    #[napi]
    #[cfg(not(target_os = "windows"))]
    pub fn show_inspect_overlay(
        &self,
        _elements: Vec<crate::types::InspectElement>,
        _window_bounds: crate::types::Bounds,
        _display_mode: Option<crate::types::OverlayDisplayMode>,
    ) -> napi::Result<()> {
        // Not implemented for other platforms yet
        Ok(())
    }

    /// Hide any active inspect overlay.
    ///
    /// This hides the visual overlay that was shown via `showInspectOverlay()`.
    /// Can be called from any thread.
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
