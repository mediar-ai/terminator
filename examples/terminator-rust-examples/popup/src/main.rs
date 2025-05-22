#[cfg(not(target_os = "windows"))]
fn main() {
    println!("This example only works on Windows.");
}

// #![windows_subsystem = "windows"] // Hides the console window

#[cfg(target_os = "windows")]
mod main {
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use std::time::Instant; // Added for popup timing

    use global_hotkey::{
        hotkey::{Code, HotKey, Modifiers},
        GlobalHotKeyEvent, GlobalHotKeyManager,
    }; // Added for global hotkeys
    use once_cell::sync::Lazy;
    use terminator::{Desktop, Selector};
    use tokio::runtime::Runtime;
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::HINSTANCE;
    use windows::Win32::Foundation::{
        GetLastError, COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
    };
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateCompatibleDC, CreateDIBSection, CreatePen, CreateSolidBrush, DeleteDC,
        DeleteObject, DrawTextW, EndPaint, FillRect, GetStockObject, Rectangle, SelectObject,
        SetBkMode, SetTextColor, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        BLENDFUNCTION, DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE, DT_VCENTER, NULL_BRUSH,
        PAINTSTRUCT, PS_SOLID, TRANSPARENT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetSystemMetrics,
        KillTimer, LoadCursorW, PostQuitMessage, RegisterClassExW, SetTimer, TranslateMessage,
        UpdateLayeredWindow, CS_HREDRAW, CS_VREDRAW, HICON, IDC_ARROW, MSG, SM_CXSCREEN,
        SM_CYSCREEN, ULW_ALPHA, WM_APP, WM_DESTROY, WM_PAINT, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
    };

    const OVERLAY_WINDOW_CLASS: &str = "TerminatorHighlightOverlayClass";
    const OVERLAY_WINDOW_TITLE: &str = "Terminator Highlight Overlay";
    const REPAINT_MSG: u32 = WM_APP + 1;

    // Shared state for highlight bounding boxes (x, y, width, height)
    static HIGHLIGHT_BOUNDS: Lazy<Arc<Mutex<Option<Vec<(i32, i32, i32, i32)>>>>> =
        Lazy::new(|| Arc::new(Mutex::new(None)));

    // --- New State Management ---
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum AppState {
        Idle,
        Highlighting,
    }

    static APP_STATE: Lazy<Arc<Mutex<AppState>>> =
        Lazy::new(|| Arc::new(Mutex::new(AppState::Idle)));

    // --- New: For hotkey debounce ---
    static LAST_HOTKEY_TOGGLE_TIME: Lazy<Arc<Mutex<Option<Instant>>>> =
        Lazy::new(|| Arc::new(Mutex::new(None)));
    const HOTKEY_COOLDOWN_DURATION: Duration = Duration::from_millis(300);
    // --- End New ---

    #[derive(Clone)]
    struct PopupInfo {
        message: String,
        display_until: Instant,
        background_color: COLORREF,
        text_color: COLORREF,
    }

    static CURRENT_POPUP_INFO: Lazy<Arc<Mutex<Option<PopupInfo>>>> =
        Lazy::new(|| Arc::new(Mutex::new(None)));

    const POPUP_TIMER_ID: usize = 1;
    const HOTKEY_ID_MEM_UPDATE: u32 = 1;
    const HOTKEY_ID_TOGGLE_HIGHLIGHT: u32 = 2;
    // --- End New State Management ---

    static TOKIO_RT: Lazy<Runtime> = Lazy::new(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime")
    });

    fn main() -> anyhow::Result<()> {
        // Spawn a thread for the overlay window
        let window_thread_handle = thread::spawn(|| {
            if let Err(e) = create_and_run_overlay_window() {
                eprintln!("Overlay window thread error: {:?}", e);
            }
        });

        // Run Terminator logic in a Tokio task
        TOKIO_RT.spawn(async move {
            if let Err(e) = terminator_logic().await {
                eprintln!("Terminator logic error: {:?}", e);
            }
        });

        // Keep the main thread alive (or do other work if needed)
        // For this example, we'll wait for the window thread to join,
        // though in a real app, main might exit or do other things
        // if the overlay is meant to run indefinitely until process termination.
        if let Err(e) = window_thread_handle.join() {
            eprintln!("Window thread panicked: {:?}", e);
        }
        Ok(())
    }

    async fn terminator_logic() -> anyhow::Result<()> {
        println!("Terminator logic started.");
        let desktop = Desktop::new(false, false).await.map_err(|e| {
            eprintln!("Mapping terminator error: {}", e);
            anyhow::anyhow!("Failed to create desktop: {}", e)
        })?;

        // Initial attempt to activate Firefox, but not critical if it fails
        if desktop.activate_application("firefox").is_err() {
            println!("Failed to activate Firefox. Ensure it's running or active.");
        }

        loop {
            let current_mode = *APP_STATE.lock().unwrap();

            if current_mode == AppState::Highlighting {
                match desktop
                    .locator(Selector::Role {
                        role: "edit".to_string(), // Example: looking for editable fields
                        name: None,
                    })
                    .all(None, None)
                    .await
                {
                    Ok(inputs) => {
                        let mut new_bounds = Vec::new();
                        if !inputs.is_empty() {
                            for input_element in inputs.iter() {
                                if let Ok(bounds_f64) = input_element.bounds() {
                                    new_bounds.push((
                                        bounds_f64.0 as i32,
                                        bounds_f64.1 as i32,
                                        bounds_f64.2 as i32,
                                        bounds_f64.3 as i32,
                                    ));
                                }
                            }
                        }

                        let mut popup_info_lock = CURRENT_POPUP_INFO.lock().unwrap();

                        if !new_bounds.is_empty() {
                            println!("Detected bounds: {:?}", new_bounds);
                            let mut current_bounds_lock = HIGHLIGHT_BOUNDS.lock().unwrap();
                            *current_bounds_lock = Some(new_bounds);
                            drop(current_bounds_lock);

                            // Show "Identified form elements." - this is the main success popup
                            *popup_info_lock = Some(PopupInfo {
                                message: "Identified form elements.".to_string(),
                                display_until: Instant::now() + Duration::from_secs(3600), // Persistent
                                background_color: COLORREF(0x00224422), // Dark green
                                text_color: COLORREF(0x00CCFFCC),       // Light green text
                            });
                        } else {
                            // No new elements found on this scan.
                            // Highlights and the "Identified form elements." popup (if active) should persist.
                            // Only show a "scanning" or "nothing found YET" message if no highlights
                            // have been established in this highlighting session and the main success popup isn't active.
                            let highlights_lock = HIGHLIGHT_BOUNDS.lock().unwrap();
                            let no_highlights_established_yet = highlights_lock.is_none();
                            drop(highlights_lock);

                            let is_identified_popup_currently_active = match &*popup_info_lock {
                                Some(info) => info.message == "Identified form elements.",
                                None => false,
                            };

                            if no_highlights_established_yet
                                && !is_identified_popup_currently_active
                            {
                                // No highlights found *yet* in this session, and success popup isn't showing.
                                *popup_info_lock = Some(PopupInfo {
                                    message: "Scanning... No elements detected yet.".to_string(),
                                    display_until: Instant::now() + Duration::from_secs(3),
                                    background_color: COLORREF(0x00333333), // Neutral dark gray
                                    text_color: COLORREF(0x00DDDDDD),       // Light gray text
                                });
                            }
                            // If highlights *were* established (no_highlights_established_yet is false)
                            // or if the "Identified..." popup is active,
                            // and new_bounds is now empty, we do nothing to the popup here.
                            // The "Identified form elements." popup and the highlights persist.
                        }
                    }
                    Err(e) => {
                        eprintln!("Error finding inputs: {}", e);
                        // Error finding inputs. IMPORTANT: DO NOT clear current_bounds_lock here.
                        // Let existing highlights persist.
                        let mut popup_info_lock = CURRENT_POPUP_INFO.lock().unwrap();
                        *popup_info_lock = Some(PopupInfo {
                            message: "Error detecting elements.".to_string(),
                            display_until: Instant::now() + Duration::from_secs(5),
                            background_color: COLORREF(0x00222255), // Dark blue/purple for error
                            text_color: COLORREF(0x00DDDDFF),       // Light blue/purple text
                        });
                    }
                }
            }
            // The 'else' branch that might have cleared HIGHLIGHT_BOUNDS when not in Highlighting mode
            // has been removed. Clearing HIGHLIGHT_BOUNDS is now solely the responsibility of the
            // hotkey handlers (Shift+H to toggle OFF, Shift+M).
            // The drawing logic in window_proc already checks APP_STATE before drawing.

            tokio::time::sleep(Duration::from_millis(250)).await; // Poll frequency
        }
    }

    fn create_and_run_overlay_window() -> anyhow::Result<()> {
        unsafe {
            let instance = GetModuleHandleW(None)
                .map_err(|e| anyhow::anyhow!("Failed to get module handle: {}", e))?;

            // Ensure HSTRINGs live long enough
            let class_name_hstring = HSTRING::from(OVERLAY_WINDOW_CLASS);
            let window_title_hstring = HSTRING::from(OVERLAY_WINDOW_TITLE);

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                lpszClassName: PCWSTR(class_name_hstring.as_ptr()), // Use persisted HSTRING
                hIcon: HICON::default(),
                hIconSm: HICON::default(),
                hbrBackground: CreateSolidBrush(COLORREF(0x00000000)),
                ..Default::default()
            };

            if RegisterClassExW(&wc) == 0 {
                return Err(anyhow::anyhow!(
                    "Failed to register window class: {:?}",
                    unsafe { GetLastError() }
                ));
            }

            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);

            let ex_style = WS_EX_LAYERED
                | WS_EX_TRANSPARENT
                | WS_EX_TOPMOST
                | WS_EX_NOACTIVATE
                | WS_EX_TOOLWINDOW;

            let hwnd = CreateWindowExW(
                ex_style,
                PCWSTR(class_name_hstring.as_ptr()), // Use persisted HSTRING
                PCWSTR(window_title_hstring.as_ptr()), // Use persisted HSTRING
                WS_POPUP | WS_VISIBLE,
                0,
                0,
                screen_width,
                screen_height,
                None,
                None,
                Some(HINSTANCE(instance.0)),
                None,
            )
            .map_err(|e| anyhow::anyhow!("Failed to create window: {:?}", e))?;

            if hwnd.is_invalid() {
                return Err(anyhow::anyhow!("Failed to create window: {:?}", unsafe {
                    GetLastError()
                }));
            }

            SetTimer(Some(hwnd), POPUP_TIMER_ID, 100, None);

            let hotkey_manager = GlobalHotKeyManager::new()
                .map_err(|e| anyhow::anyhow!("Failed to create hotkey manager: {:?}", e))?;

            let hotkey_mem_update_def = HotKey::new(Some(Modifiers::SHIFT), Code::KeyM);
            let id_mem_update = hotkey_mem_update_def.id(); // Get ID before registration
            hotkey_manager
                .register(hotkey_mem_update_def)
                .map_err(|e| anyhow::anyhow!("Failed to register Shift+M: {:?}", e))?;

            let hotkey_toggle_highlight_def = HotKey::new(Some(Modifiers::SHIFT), Code::KeyH);
            let id_toggle_highlight = hotkey_toggle_highlight_def.id(); // Get ID before registration
            hotkey_manager
                .register(hotkey_toggle_highlight_def)
                .map_err(|e| anyhow::anyhow!("Failed to register Shift+H: {:?}", e))?;

            println!(
                "Hotkeys registered: Shift+M (ID: {}), Shift+H (ID: {})",
                id_mem_update, id_toggle_highlight
            );

            let mut msg = MSG::default();
            loop {
                if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
                    println!("Hotkey event: {:?} ID: {}", event, event.id); // event might not be Debug
                    if event.id == id_mem_update {
                        let mut popup_info_lock = CURRENT_POPUP_INFO.lock().unwrap();
                        *popup_info_lock = Some(PopupInfo {
                            message: "Memory updated".to_string(),
                            display_until: Instant::now() + Duration::from_secs(5),
                            background_color: COLORREF(0x00222222),
                            text_color: COLORREF(0x00FFFFFF),
                        });
                        drop(popup_info_lock);
                        let mut app_state_lock = APP_STATE.lock().unwrap();
                        *app_state_lock = AppState::Idle;
                        drop(app_state_lock);
                        let mut highlights_lock = HIGHLIGHT_BOUNDS.lock().unwrap();
                        *highlights_lock = None;
                        drop(highlights_lock);
                        windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, false);
                    } else if event.id == id_toggle_highlight {
                        let mut last_toggle_time_lock = LAST_HOTKEY_TOGGLE_TIME.lock().unwrap();
                        let now = Instant::now();

                        if let Some(last_time) = *last_toggle_time_lock {
                            if now.duration_since(last_time) < HOTKEY_COOLDOWN_DURATION {
                                println!("Shift+H pressed too quickly, ignoring.");
                                // Still need to release locks if we continue in the loop
                                drop(last_toggle_time_lock); // Release this lock
                                                             // The other locks (app_state_lock, popup_info_lock) are not yet acquired
                                continue; // Skip to the next iteration of the message loop
                            }
                        }

                        // Update the last toggle time since we are processing this event
                        *last_toggle_time_lock = Some(now);
                        drop(last_toggle_time_lock); // Release lock early

                        let mut app_state_lock = APP_STATE.lock().unwrap();
                        let mut popup_info_lock = CURRENT_POPUP_INFO.lock().unwrap(); // Lock earlier

                        if *app_state_lock == AppState::Highlighting {
                            // ---- TURNING OFF ----
                            *app_state_lock = AppState::Idle;
                            let mut highlights_lock = HIGHLIGHT_BOUNDS.lock().unwrap();
                            *highlights_lock = None; // Clear highlights
                            drop(highlights_lock);

                            // Set "Highlighting OFF" popup
                            *popup_info_lock = Some(PopupInfo {
                                message: "Highlighting OFF".to_string(),
                                display_until: Instant::now() + Duration::from_secs(2),
                                background_color: COLORREF(0x00222222),
                                text_color: COLORREF(0x00DDDDDD),
                            });
                        } else {
                            // ---- TURNING ON ----
                            *app_state_lock = AppState::Highlighting;
                            // Clear any existing popup (e.g., "Highlighting OFF" if toggled quickly)
                            // Let terminator_logic handle the "Identified..." or initial "Scanning..." popups.
                            *popup_info_lock = None;
                        }
                        drop(app_state_lock);
                        drop(popup_info_lock);
                        windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, false);
                    }
                }

                if GetMessageW(&mut msg, None, 0, 0).into() {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                } else {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            KillTimer(Some(hwnd), POPUP_TIMER_ID);
            Ok(())
        }
    }

    extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            match msg {
                WM_PAINT | REPAINT_MSG => {
                    let mut ps = PAINTSTRUCT::default();
                    let hdc_screen = BeginPaint(hwnd, &mut ps);

                    let screen_width = GetSystemMetrics(SM_CXSCREEN);
                    let screen_height = GetSystemMetrics(SM_CYSCREEN);

                    let mem_dc = CreateCompatibleDC(Some(hdc_screen));
                    if mem_dc.is_invalid() {
                        EndPaint(hwnd, &ps);
                        return DefWindowProcW(hwnd, msg, wparam, lparam);
                    }

                    let mut bmi = BITMAPINFO {
                        bmiHeader: BITMAPINFOHEADER {
                            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                            biWidth: screen_width,
                            biHeight: -screen_height,
                            biPlanes: 1,
                            biBitCount: 32,
                            biCompression: BI_RGB.0, // Use .0 to get the u32 value from the BI_RGB constant
                            biSizeImage: 0,
                            biXPelsPerMeter: 0,
                            biYPelsPerMeter: 0,
                            biClrUsed: 0,
                            biClrImportant: 0,
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    let mut pv_bits: *mut std::ffi::c_void = std::ptr::null_mut();
                    let mem_bitmap_result = CreateDIBSection(
                        None,
                        &bmi as *const BITMAPINFO,
                        DIB_RGB_COLORS,
                        &mut pv_bits,
                        None,
                        0,
                    );

                    let mem_bitmap = match mem_bitmap_result {
                        Ok(bitmap) => bitmap,
                        Err(_) => {
                            DeleteDC(mem_dc);
                            EndPaint(hwnd, &ps);
                            return DefWindowProcW(hwnd, msg, wparam, lparam);
                        }
                    };

                    if mem_bitmap.is_invalid() || pv_bits.is_null() {
                        if !mem_bitmap.is_invalid() {
                            DeleteObject(mem_bitmap.into());
                        }
                        DeleteDC(mem_dc);
                        EndPaint(hwnd, &ps);
                        return DefWindowProcW(hwnd, msg, wparam, lparam);
                    }

                    let bitmap_data_size = (screen_width * screen_height * 4) as usize;
                    std::ptr::write_bytes(pv_bits as *mut u8, 0, bitmap_data_size);

                    let old_bitmap = SelectObject(mem_dc, mem_bitmap.into());

                    SetBkMode(mem_dc, TRANSPARENT); // Make background transparent for text

                    // 1. Draw highlights (if any)
                    let bounds_option_lock = HIGHLIGHT_BOUNDS.lock().unwrap();
                    if let Some(bounds_vec) = &*bounds_option_lock {
                        if *APP_STATE.lock().unwrap() == AppState::Highlighting {
                            // Only draw if in highlighting state
                            let red_color = COLORREF(0x000000FF);
                            let h_pen = CreatePen(PS_SOLID, 3, red_color); // Thinner pen
                            let h_null_brush = GetStockObject(NULL_BRUSH);

                            if !h_pen.is_invalid() && !h_null_brush.is_invalid() {
                                let h_old_pen = SelectObject(mem_dc, h_pen.into());
                                let h_old_brush = SelectObject(mem_dc, h_null_brush);
                                for &(x, y, w, h) in bounds_vec.iter() {
                                    if w > 0 && h > 0 {
                                        Rectangle(mem_dc, x, y, x + w, y + h);
                                    }
                                }
                                SelectObject(mem_dc, h_old_pen);
                                SelectObject(mem_dc, h_old_brush);
                                DeleteObject(h_pen.into());
                            } else {
                                if !h_pen.is_invalid() {
                                    DeleteObject(h_pen.into());
                                }
                                // Log error or handle, for now, just skip drawing highlights
                                // If we return early, ensure proper cleanup
                                SelectObject(mem_dc, old_bitmap);
                                DeleteObject(mem_bitmap.into());
                                DeleteDC(mem_dc);
                                EndPaint(hwnd, &ps);
                                return DefWindowProcW(hwnd, msg, wparam, lparam);
                            }
                        }
                    }
                    drop(bounds_option_lock);

                    // 2. Draw Popup (if any)
                    let mut popup_info_opt = CURRENT_POPUP_INFO.lock().unwrap();
                    let mut clear_popup_after_drawing = false;
                    if let Some(popup) = &*popup_info_opt {
                        if Instant::now() < popup.display_until {
                            let popup_width = 300;
                            let popup_height = 70;
                            let margin = 20;
                            let popup_rect = RECT {
                                left: screen_width - popup_width - margin,
                                top: screen_height - popup_height - margin - 50, // Move up by an additional 50 pixels
                                right: screen_width - margin,
                                bottom: screen_height - margin - 50, // Move up by an additional 50 pixels
                            };

                            let bg_brush = CreateSolidBrush(popup.background_color);
                            if !bg_brush.is_invalid() {
                                FillRect(mem_dc, &popup_rect, bg_brush);
                                DeleteObject(bg_brush.into());
                            }

                            SetTextColor(mem_dc, popup.text_color);
                            // Font handling can be improved (e.g. CreateFontIndirect), using default for now
                            let mut text_rect_for_draw_text = RECT {
                                left: popup_rect.left + 10,
                                top: popup_rect.top + 10,
                                right: popup_rect.right - 10,
                                bottom: popup_rect.bottom - 10,
                            }; // Text rect with padding
                            let h_message_string = popup.message.clone();
                            let mut wide_buffer: Vec<u16> =
                                h_message_string.encode_utf16().collect();
                            wide_buffer.push(0); // Null-terminate for safety, though DrawText may not strictly need it if rect is well-defined

                            DrawTextW(
                                mem_dc,
                                &mut wide_buffer, // Pass as mutable slice
                                &mut text_rect_for_draw_text,
                                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                            );
                        } else {
                            // Popup expired
                            clear_popup_after_drawing = true;
                        }
                    }
                    if clear_popup_after_drawing {
                        *popup_info_opt = None;
                    }
                    drop(popup_info_opt);

                    // 3. Update Layered Window
                    let mut blend_function = BLENDFUNCTION {
                        BlendOp: AC_SRC_OVER as u8,
                        BlendFlags: 0,
                        SourceConstantAlpha: 255,
                        AlphaFormat: AC_SRC_ALPHA as u8,
                    };
                    let pt_src = POINT { x: 0, y: 0 };
                    let size = SIZE {
                        cx: screen_width,
                        cy: screen_height,
                    };
                    let pt_dst = POINT { x: 0, y: 0 };

                    let _ = UpdateLayeredWindow(
                        hwnd,
                        None,
                        Some(&pt_dst),
                        Some(&size),
                        Some(mem_dc),
                        Some(&pt_src),
                        COLORREF(0),
                        Some(&mut blend_function),
                        ULW_ALPHA,
                    );

                    SelectObject(mem_dc, old_bitmap);
                    DeleteObject(mem_bitmap.into());
                    DeleteDC(mem_dc);
                    EndPaint(hwnd, &ps);
                    LRESULT(0)
                }
                WM_DESTROY => {
                    KillTimer(Some(hwnd), POPUP_TIMER_ID);
                    PostQuitMessage(0);
                    LRESULT(0)
                }
                WM_TIMER => {
                    if wparam.0 == POPUP_TIMER_ID {
                        // Check if popup has expired and needs a repaint to be cleared
                        let mut needs_repaint = false;
                        if let Some(popup_info) = &*CURRENT_POPUP_INFO.lock().unwrap() {
                            if Instant::now() >= popup_info.display_until {
                                needs_repaint = true;
                            }
                        }
                        if needs_repaint {
                            // Setting it to None here and then invalidating ensures it's cleared
                            *CURRENT_POPUP_INFO.lock().unwrap() = None;
                            windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, false);
                        }
                        // Also, repaint if app state is highlighting to refresh bounds (optional, could be more targeted)
                        if *APP_STATE.lock().unwrap() == AppState::Highlighting {
                            windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, false);
                        }
                    }
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
    }
}
