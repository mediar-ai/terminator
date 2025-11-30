//! Inspect overlay functionality for Windows
//! Renders a visual overlay showing all UI elements with indices and roles

use crate::AutomationError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracing::{debug, error, info};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, FillRect, GetDC, LineTo,
    MoveToEx, Rectangle, ReleaseDC, SelectObject, SetBkMode, SetTextColor, DT_SINGLELINE,
    DT_VCENTER, HBRUSH, HGDIOBJ, PS_SOLID, TRANSPARENT,
};

/// Display mode for overlay labels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayDisplayMode {
    /// Just rectangles, no labels
    Rectangles,
    /// [index] only
    #[default]
    Index,
    /// [role] only
    Role,
    /// [index:role]
    IndexRole,
    /// [name] only
    Name,
    /// [index:name]
    IndexName,
    /// [index:role:name]
    Full,
}
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, LoadCursorW, RegisterClassExW,
    SetLayeredWindowAttributes, ShowWindow, HICON, IDC_ARROW, LWA_COLORKEY, SW_SHOWNOACTIVATE,
    WNDCLASSEXW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const OVERLAY_CLASS_NAME: PCWSTR = w!("TerminatorInspectOverlay");

/// Element data for inspect overlay rendering
#[derive(Debug, Clone)]
pub struct InspectElement {
    pub index: u32,
    pub role: String,
    pub name: Option<String>,
    pub bounds: (f64, f64, f64, f64), // x, y, width, height
}

/// Handle for managing the inspect overlay
pub struct InspectOverlayHandle {
    should_close: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl InspectOverlayHandle {
    /// Close the overlay
    pub fn close(mut self) {
        self.should_close.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if the overlay is still active
    pub fn is_active(&self) -> bool {
        !self.should_close.load(Ordering::Relaxed)
    }
}

impl Drop for InspectOverlayHandle {
    fn drop(&mut self) {
        // Signal close but don't wait
        self.should_close.store(true, Ordering::Relaxed);
    }
}

// Thread-local storage for overlay window handle
thread_local! {
    static INSPECT_OVERLAY_HWND: std::cell::RefCell<Option<HWND>> = const { std::cell::RefCell::new(None) };
}

// Global storage for elements to render (shared between threads)
static INSPECT_ELEMENTS: std::sync::OnceLock<std::sync::Mutex<Vec<InspectElement>>> =
    std::sync::OnceLock::new();
static WINDOW_OFFSET: std::sync::OnceLock<std::sync::Mutex<(i32, i32)>> =
    std::sync::OnceLock::new();
static DISPLAY_MODE: std::sync::OnceLock<std::sync::Mutex<OverlayDisplayMode>> =
    std::sync::OnceLock::new();

fn get_elements_storage() -> &'static std::sync::Mutex<Vec<InspectElement>> {
    INSPECT_ELEMENTS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn get_offset_storage() -> &'static std::sync::Mutex<(i32, i32)> {
    WINDOW_OFFSET.get_or_init(|| std::sync::Mutex::new((0, 0)))
}

fn get_display_mode_storage() -> &'static std::sync::Mutex<OverlayDisplayMode> {
    DISPLAY_MODE.get_or_init(|| std::sync::Mutex::new(OverlayDisplayMode::default()))
}

/// Show inspect overlay for the given elements within window bounds
pub fn show_inspect_overlay(
    elements: Vec<InspectElement>,
    window_bounds: (i32, i32, i32, i32), // x, y, width, height
    display_mode: OverlayDisplayMode,
) -> Result<InspectOverlayHandle, AutomationError> {
    let (win_x, win_y, win_w, win_h) = window_bounds;

    info!(
        "show_inspect_overlay: {} elements, window bounds: ({}, {}, {}, {}), mode: {:?}",
        elements.len(),
        win_x,
        win_y,
        win_w,
        win_h,
        display_mode
    );

    // Store elements, offset, and display mode for the paint callback
    {
        let mut stored = get_elements_storage().lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock elements storage: {e}"))
        })?;
        *stored = elements;
    }
    {
        let mut mode = get_display_mode_storage().lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock display mode storage: {e}"))
        })?;
        *mode = display_mode;
    }
    {
        let mut offset = get_offset_storage().lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock offset storage: {e}"))
        })?;
        *offset = (win_x, win_y);
    }

    let should_close = Arc::new(AtomicBool::new(false));
    let should_close_clone = should_close.clone();

    let handle = thread::spawn(move || {
        if let Err(e) =
            create_inspect_overlay_window(win_x, win_y, win_w, win_h, should_close_clone)
        {
            error!("Failed to create inspect overlay: {}", e);
        }
    });

    Ok(InspectOverlayHandle {
        should_close,
        handle: Some(handle),
    })
}

/// Hide any active inspect overlay (called from same thread)
pub fn hide_inspect_overlay() {
    cleanup_overlay_window();
}

/// Cleans up the overlay window stored in thread-local storage
fn cleanup_overlay_window() {
    INSPECT_OVERLAY_HWND.with(|cell| {
        if let Some(hwnd) = cell.borrow_mut().take() {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            debug!("Destroyed inspect overlay window");
        }
    });
}

fn create_inspect_overlay_window(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    should_close: Arc<AtomicBool>,
) -> Result<(), AutomationError> {
    unsafe {
        // Clean up any previous overlay
        cleanup_overlay_window();

        let instance = GetModuleHandleW(None)
            .map_err(|e| AutomationError::PlatformError(format!("GetModuleHandleW failed: {e}")))?;

        // Register window class
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: windows::Win32::UI::WindowsAndMessaging::WNDCLASS_STYLES(0),
            lpfnWndProc: Some(inspect_overlay_window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance.into(),
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: OVERLAY_CLASS_NAME,
            hIconSm: HICON::default(),
        };

        let atom = RegisterClassExW(&wc);
        if atom == 0 {
            debug!("RegisterClassExW returned 0 (class may already exist)");
        }

        // Create overlay window
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            OVERLAY_CLASS_NAME,
            w!("Inspect Overlay"),
            WS_POPUP,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| AutomationError::PlatformError(format!("CreateWindowExW failed: {e}")))?;

        if hwnd.is_invalid() {
            return Err(AutomationError::PlatformError(
                "CreateWindowExW returned invalid HWND".to_string(),
            ));
        }

        // Make black transparent (color key)
        SetLayeredWindowAttributes(hwnd, COLORREF(0x000000), 255, LWA_COLORKEY).map_err(|e| {
            AutomationError::PlatformError(format!("SetLayeredWindowAttributes failed: {e}"))
        })?;

        // Show without activating
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        // Draw overlay content directly (not via WM_PAINT)
        draw_inspect_overlay(hwnd);

        // Store HWND for cleanup
        INSPECT_OVERLAY_HWND.with(|cell| {
            *cell.borrow_mut() = Some(hwnd);
        });

        info!("Inspect overlay window created and drawn");

        // Polling loop - check should_close every 50ms (like highlighting.rs)
        while !should_close.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(50));
        }

        // Cleanup
        cleanup_overlay_window();

        info!("Inspect overlay window destroyed");
    }

    Ok(())
}

/// Helper to format label based on display mode
fn format_label(elem: &InspectElement, mode: OverlayDisplayMode) -> Option<String> {
    match mode {
        OverlayDisplayMode::Rectangles => None,
        OverlayDisplayMode::Index => Some(format!("[{}]", elem.index)),
        OverlayDisplayMode::Role => Some(format!("[{}]", elem.role)),
        OverlayDisplayMode::IndexRole => Some(format!("[{}:{}]", elem.index, elem.role)),
        OverlayDisplayMode::Name => {
            if let Some(ref name) = elem.name {
                let truncated = if name.len() > 15 {
                    format!("{}...", &name[..12])
                } else {
                    name.clone()
                };
                Some(format!("[{truncated}]"))
            } else {
                Some(format!("[{}]", elem.index))
            }
        }
        OverlayDisplayMode::IndexName => {
            if let Some(ref name) = elem.name {
                let truncated = if name.len() > 15 {
                    format!("{}...", &name[..12])
                } else {
                    name.clone()
                };
                Some(format!("[{}:{}]", elem.index, truncated))
            } else {
                Some(format!("[{}]", elem.index))
            }
        }
        OverlayDisplayMode::Full => {
            if let Some(ref name) = elem.name {
                let truncated = if name.len() > 12 {
                    format!("{}...", &name[..9])
                } else {
                    name.clone()
                };
                Some(format!("[{}:{}:{}]", elem.index, elem.role, truncated))
            } else {
                Some(format!("[{}:{}]", elem.index, elem.role))
            }
        }
    }
}

/// Check if two rectangles overlap
fn rects_overlap(r1: &RECT, r2: &RECT) -> bool {
    !(r1.right < r2.left || r1.left > r2.right || r1.bottom < r2.top || r1.top > r2.bottom)
}

/// Draw overlay content directly using GetDC (not via WM_PAINT)
fn draw_inspect_overlay(hwnd: HWND) {
    unsafe {
        let hdc = GetDC(Some(hwnd));
        if hdc.is_invalid() {
            return;
        }

        // Get window rect
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);

        // Fill background with black (will be transparent due to color key)
        let black_brush = CreateSolidBrush(COLORREF(0x000000));
        FillRect(hdc, &rect, black_brush);
        let _ = DeleteObject(black_brush.into());

        // Get stored elements, offset, and display mode
        let elements = get_elements_storage().lock().ok();
        let offset = get_offset_storage().lock().ok();
        let display_mode = get_display_mode_storage().lock().ok();

        if let (Some(elements), Some(offset), Some(display_mode)) = (elements, offset, display_mode)
        {
            let (offset_x, offset_y) = *offset;
            let mode = *display_mode;

            // Create pen for borders (green, 2px) and connector lines (green, 1px)
            let border_pen = CreatePen(PS_SOLID, 2, COLORREF(0x00FF00));
            let connector_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00FF00));
            let old_pen = SelectObject(hdc, HGDIOBJ(border_pen.0));

            // Select null brush for transparent fill on rectangles
            let null_brush = windows::Win32::Graphics::Gdi::GetStockObject(
                windows::Win32::Graphics::Gdi::NULL_BRUSH,
            );
            let old_brush = SelectObject(hdc, null_brush);

            // Create font for labels (11px, bold)
            let font = CreateFontW(
                11, // Height
                0,
                0,
                0,
                700, // Bold weight
                0,
                0,
                0,
                windows::Win32::Graphics::Gdi::FONT_CHARSET(1),
                windows::Win32::Graphics::Gdi::FONT_OUTPUT_PRECISION(0),
                windows::Win32::Graphics::Gdi::FONT_CLIP_PRECISION(0),
                windows::Win32::Graphics::Gdi::FONT_QUALITY(0),
                0,
                PCWSTR::null(),
            );
            let old_font = SelectObject(hdc, HGDIOBJ(font.0));

            // Set text properties
            SetTextColor(hdc, COLORREF(0x000000)); // Black text
            SetBkMode(hdc, TRANSPARENT);

            let label_height = 12;
            let diagonal_offset = 12; // How far to offset diagonally when collision

            // Track used label positions for collision detection
            let mut used_rects: Vec<RECT> = Vec::new();

            // First pass: draw all element rectangles
            for elem in elements.iter() {
                let (ex, ey, ew, eh) = elem.bounds;
                let rel_x = (ex as i32) - offset_x;
                let rel_y = (ey as i32) - offset_y;
                let rel_w = ew as i32;
                let rel_h = eh as i32;

                if rel_w < 5 || rel_h < 5 {
                    continue;
                }

                // Draw border rectangle
                let _ = Rectangle(hdc, rel_x, rel_y, rel_x + rel_w, rel_y + rel_h);
            }

            // Second pass: draw labels with collision detection (if not rectangles-only mode)
            if mode != OverlayDisplayMode::Rectangles {
                for elem in elements.iter() {
                    let (ex, ey, ew, eh) = elem.bounds;
                    let rel_x = (ex as i32) - offset_x;
                    let rel_y = (ey as i32) - offset_y;
                    let rel_w = ew as i32;
                    let rel_h = eh as i32;

                    if rel_w < 5 || rel_h < 5 {
                        continue;
                    }

                    // Format label based on display mode
                    let label = match format_label(elem, mode) {
                        Some(l) => l,
                        None => continue,
                    };
                    let label_width = (label.len() * 5) as i32 + 4; // 5px per char for smaller font

                    // Initial label position (above element)
                    let mut label_left = rel_x;
                    let mut label_top = if rel_y > label_height + 2 {
                        rel_y - label_height - 2
                    } else {
                        rel_y
                    };

                    let mut label_rect = RECT {
                        left: label_left,
                        top: label_top,
                        right: label_left + label_width,
                        bottom: label_top + label_height,
                    };

                    // Check for collisions and offset diagonally
                    let mut attempts = 0;
                    let max_attempts = 10;
                    while attempts < max_attempts
                        && used_rects.iter().any(|r| rects_overlap(&label_rect, r))
                    {
                        // Offset diagonally (up-right)
                        label_left += diagonal_offset;
                        label_top -= diagonal_offset;
                        label_rect = RECT {
                            left: label_left,
                            top: label_top,
                            right: label_left + label_width,
                            bottom: label_top + label_height,
                        };
                        attempts += 1;
                    }

                    // Check if label was offset (needs connector line)
                    let was_offset = label_left != rel_x
                        || label_top
                            != (if rel_y > label_height + 2 {
                                rel_y - label_height - 2
                            } else {
                                rel_y
                            });

                    // Draw connector line if label was offset
                    if was_offset {
                        SelectObject(hdc, HGDIOBJ(connector_pen.0));
                        let _ = MoveToEx(hdc, label_left, label_top + label_height, None);
                        let _ = LineTo(hdc, rel_x, rel_y);
                        SelectObject(hdc, HGDIOBJ(border_pen.0));
                    }

                    // Draw text (no background fill)
                    let mut wide_text: Vec<u16> = label.encode_utf16().collect();
                    wide_text.push(0);

                    let mut text_rect = RECT {
                        left: label_left + 1,
                        top: label_top,
                        right: label_left + label_width,
                        bottom: label_top + label_height,
                    };

                    let _ = DrawTextW(
                        hdc,
                        &mut wide_text,
                        &mut text_rect,
                        DT_SINGLELINE | DT_VCENTER,
                    );

                    // Track this label's position
                    used_rects.push(label_rect);
                }
            }

            // Restore and cleanup
            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_font);
            SelectObject(hdc, old_pen);
            let _ = DeleteObject(HGDIOBJ(font.0));
            let _ = DeleteObject(border_pen.into());
            let _ = DeleteObject(connector_pen.into());
        }

        let _ = ReleaseDC(Some(hwnd), hdc);
    }
}

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};

unsafe extern "system" fn inspect_overlay_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Minimal window proc - we draw directly, not via WM_PAINT
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
