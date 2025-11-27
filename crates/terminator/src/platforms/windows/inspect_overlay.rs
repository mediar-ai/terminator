//! Inspect overlay functionality for Windows
//! Renders a visual overlay showing all UI elements with indices and roles

use crate::AutomationError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing::{debug, error, info};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint,
    FillRect, Rectangle, SelectObject, SetBkMode, SetTextColor, DT_SINGLELINE,
    DT_VCENTER, HBRUSH, HGDIOBJ, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetMessageW, LoadCursorW, PostMessageW,
    RegisterClassExW, SetLayeredWindowAttributes, ShowWindow, TranslateMessage, DispatchMessageW,
    HICON, IDC_ARROW, LWA_COLORKEY, MSG, SW_SHOWNOACTIVATE, WM_CLOSE, WM_DESTROY, WM_PAINT,
    WNDCLASSEXW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

const OVERLAY_CLASS_NAME: PCWSTR = w!("TerminatorInspectOverlay");

/// Element data for inspect overlay rendering
#[derive(Debug, Clone)]
pub struct InspectElement {
    pub index: u32,
    pub role: String,
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
static INSPECT_ELEMENTS: std::sync::OnceLock<std::sync::Mutex<Vec<InspectElement>>> = std::sync::OnceLock::new();
static WINDOW_OFFSET: std::sync::OnceLock<std::sync::Mutex<(i32, i32)>> = std::sync::OnceLock::new();

fn get_elements_storage() -> &'static std::sync::Mutex<Vec<InspectElement>> {
    INSPECT_ELEMENTS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn get_offset_storage() -> &'static std::sync::Mutex<(i32, i32)> {
    WINDOW_OFFSET.get_or_init(|| std::sync::Mutex::new((0, 0)))
}

/// Show inspect overlay for the given elements within window bounds
pub fn show_inspect_overlay(
    elements: Vec<InspectElement>,
    window_bounds: (i32, i32, i32, i32), // x, y, width, height
) -> Result<InspectOverlayHandle, AutomationError> {
    let (win_x, win_y, win_w, win_h) = window_bounds;

    info!(
        "show_inspect_overlay: {} elements, window bounds: ({}, {}, {}, {})",
        elements.len(), win_x, win_y, win_w, win_h
    );

    // Store elements and offset for the paint callback
    {
        let mut stored = get_elements_storage().lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock elements storage: {}", e))
        })?;
        *stored = elements;
    }
    {
        let mut offset = get_offset_storage().lock().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to lock offset storage: {}", e))
        })?;
        *offset = (win_x, win_y);
    }

    let should_close = Arc::new(AtomicBool::new(false));
    let should_close_clone = should_close.clone();

    let handle = thread::spawn(move || {
        if let Err(e) = create_inspect_overlay_window(win_x, win_y, win_w, win_h, should_close_clone) {
            error!("Failed to create inspect overlay: {}", e);
        }
    });

    Ok(InspectOverlayHandle {
        should_close,
        handle: Some(handle),
    })
}

/// Hide any active inspect overlay
pub fn hide_inspect_overlay() {
    INSPECT_OVERLAY_HWND.with(|cell| {
        if let Some(hwnd) = cell.borrow_mut().take() {
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
            info!("Posted WM_CLOSE to inspect overlay");
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
        let instance = GetModuleHandleW(None)
            .map_err(|e| AutomationError::PlatformError(format!("GetModuleHandleW failed: {}", e)))?;

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
        .map_err(|e| AutomationError::PlatformError(format!("CreateWindowExW failed: {}", e)))?;

        if hwnd.is_invalid() {
            return Err(AutomationError::PlatformError(
                "CreateWindowExW returned invalid HWND".to_string(),
            ));
        }

        // Make black transparent (color key)
        SetLayeredWindowAttributes(hwnd, COLORREF(0x000000), 255, LWA_COLORKEY).map_err(|e| {
            AutomationError::PlatformError(format!("SetLayeredWindowAttributes failed: {}", e))
        })?;

        // Store HWND
        INSPECT_OVERLAY_HWND.with(|cell| {
            *cell.borrow_mut() = Some(hwnd);
        });

        // Show without activating
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        info!("Inspect overlay window created");

        // Message loop
        let mut msg = MSG::default();
        while !should_close.load(Ordering::Relaxed) {
            if GetMessageW(&mut msg, None, 0, 0).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                break;
            }
        }

        // Cleanup
        let _ = DestroyWindow(hwnd);
        INSPECT_OVERLAY_HWND.with(|cell| {
            *cell.borrow_mut() = None;
        });

        info!("Inspect overlay window destroyed");
    }

    Ok(())
}

unsafe extern "system" fn inspect_overlay_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DESTROY => {
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            // Get window rect
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);

            // Fill background with black (will be transparent due to color key)
            let black_brush = CreateSolidBrush(COLORREF(0x000000));
            FillRect(hdc, &rect, black_brush);
            let _ = DeleteObject(black_brush.into());

            // Get stored elements and offset
            let elements = get_elements_storage().lock().ok();
            let offset = get_offset_storage().lock().ok();

            if let (Some(elements), Some(offset)) = (elements, offset) {
                let (offset_x, offset_y) = *offset;

                // Create pen for borders (green) - no fill
                let pen = CreatePen(PS_SOLID, 2, COLORREF(0x00FF00)); // Green in BGR
                let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));

                // Select null brush for transparent fill on rectangles
                let null_brush = windows::Win32::Graphics::Gdi::GetStockObject(
                    windows::Win32::Graphics::Gdi::NULL_BRUSH,
                );
                let old_brush = SelectObject(hdc, null_brush);

                // Create font for labels (smaller: 11 instead of 14)
                let font = CreateFontW(
                    11, // Height - 3 increments smaller
                    0, 0, 0,
                    700, // Bold
                    0, 0, 0,
                    windows::Win32::Graphics::Gdi::FONT_CHARSET(1),
                    windows::Win32::Graphics::Gdi::FONT_OUTPUT_PRECISION(0),
                    windows::Win32::Graphics::Gdi::FONT_CLIP_PRECISION(0),
                    windows::Win32::Graphics::Gdi::FONT_QUALITY(0),
                    0,
                    PCWSTR::null(),
                );
                let old_font = SelectObject(hdc, HGDIOBJ(font.0));

                // Set text properties
                SetTextColor(hdc, COLORREF(0xFFFFFF)); // White text
                SetBkMode(hdc, TRANSPARENT);

                let label_height = 13; // Adjusted for smaller font

                for elem in elements.iter() {
                    let (ex, ey, ew, eh) = elem.bounds;

                    // Convert to overlay-relative coordinates
                    let rel_x = (ex as i32) - offset_x;
                    let rel_y = (ey as i32) - offset_y;
                    let rel_w = ew as i32;
                    let rel_h = eh as i32;

                    // Skip if outside visible area or too small
                    if rel_w < 5 || rel_h < 5 {
                        continue;
                    }

                    // Draw border rectangle (transparent fill due to null brush)
                    let _ = Rectangle(hdc, rel_x, rel_y, rel_x + rel_w, rel_y + rel_h);

                    // Draw label ABOVE the green box - just show index number
                    let label = format!("[{}]", elem.index);
                    let label_width = (label.len() * 6) as i32 + 4; // Width for index only

                    // Position label above the box (rel_y - label_height)
                    let label_top = if rel_y > label_height { rel_y - label_height } else { rel_y };

                    let label_rect = RECT {
                        left: rel_x,
                        top: label_top,
                        right: rel_x + label_width,
                        bottom: label_top + label_height,
                    };

                    // Fill label background with dark color (not pure black to avoid transparency)
                    let label_bg = CreateSolidBrush(COLORREF(0x333333)); // Dark gray
                    FillRect(hdc, &label_rect, label_bg);
                    let _ = DeleteObject(label_bg.into());

                    // Draw text
                    let mut wide_text: Vec<u16> = label.encode_utf16().collect();
                    wide_text.push(0);

                    let mut text_rect = RECT {
                        left: rel_x + 1,
                        top: label_top,
                        right: rel_x + label_width,
                        bottom: label_top + label_height,
                    };

                    let _ = DrawTextW(hdc, &mut wide_text, &mut text_rect, DT_SINGLELINE | DT_VCENTER);
                }

                // Restore old brush
                SelectObject(hdc, old_brush);

                // Cleanup
                SelectObject(hdc, old_font);
                SelectObject(hdc, old_pen);
                let _ = DeleteObject(HGDIOBJ(font.0));
                let _ = DeleteObject(pen.into());
            }

            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
