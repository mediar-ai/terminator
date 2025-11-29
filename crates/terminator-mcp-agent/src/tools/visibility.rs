//! Element visibility and scrolling helpers.
//!
//! This module contains functions for ensuring UI elements are visible
//! and properly positioned in the viewport before interaction.

use terminator::UIElement;

/// Helper function to check if rectangles intersect
fn rects_intersect(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    let (ax, ay, aw, ah) = a;
    let (bx, by, bw, bh) = b;
    let a_right = ax + aw;
    let a_bottom = ay + ah;
    let b_right = bx + bw;
    let b_bottom = by + bh;
    ax < b_right && a_right > bx && ay < b_bottom && a_bottom > by
}

/// Helper function to check if element is within work area (Windows only)
#[cfg(target_os = "windows")]
fn check_work_area(ex: f64, ey: f64, ew: f64, eh: f64) -> bool {
    use terminator::platforms::windows::element::WorkArea;
    if let Ok(work_area) = WorkArea::get_primary() {
        work_area.intersects(ex, ey, ew, eh)
    } else {
        true // If we can't get work area, assume visible
    }
}

#[cfg(not(target_os = "windows"))]
fn check_work_area(_ex: f64, _ey: f64, _ew: f64, _eh: f64) -> bool {
    true // Non-Windows platforms don't need taskbar adjustment
}

/// Get viewport bounds based on work area (Windows) or defaults (other platforms)
#[cfg(target_os = "windows")]
fn get_viewport_bounds() -> (f64, f64, f64) {
    use terminator::platforms::windows::element::WorkArea;
    if let Ok(work_area) = WorkArea::get_primary() {
        let work_height = work_area.height as f64;
        (
            100.0,               // Too close to top
            work_height * 0.65,  // Good zone ends at 65% of work area
            work_height - 100.0, // Too close to bottom (accounting for taskbar)
        )
    } else {
        // Fallback to defaults if work area unavailable
        (100.0, 700.0, 900.0)
    }
}

#[cfg(not(target_os = "windows"))]
fn get_viewport_bounds() -> (f64, f64, f64) {
    (100.0, 700.0, 900.0)
}

/// Check if element Y position indicates it needs scrolling
#[cfg(target_os = "windows")]
fn check_element_needs_scroll_by_y(ey: f64) -> bool {
    use terminator::platforms::windows::element::WorkArea;
    if let Ok(work_area) = WorkArea::get_primary() {
        let work_height = work_area.height as f64;
        if ey > work_height - 100.0 {
            tracing::info!(
                "Element Y={ey} near bottom of work area, assuming needs scroll"
            );
            return true;
        }
    } else if ey > 1080.0 {
        // Fallback to heuristic if we can't get work area
        tracing::info!("Element Y={ey} > 1080, assuming needs scroll");
        return true;
    }
    false
}

#[cfg(not(target_os = "windows"))]
fn check_element_needs_scroll_by_y(ey: f64) -> bool {
    if ey > 1080.0 {
        tracing::info!("Element Y={ey} > 1080, assuming needs scroll");
        return true;
    }
    false
}

/// Ensure element is scrolled into view for reliable interaction.
/// Uses sophisticated scrolling logic with focus fallback and viewport positioning.
/// Returns Ok(()) if element is visible or successfully scrolled into view.
pub fn ensure_element_in_view(element: &UIElement) -> Result<(), String> {
    // Check if element needs scrolling
    let mut need_scroll = false;

    if let Ok((ex, ey, ew, eh)) = element.bounds() {
        tracing::debug!("Element bounds: x={ex}, y={ey}, w={ew}, h={eh}");

        // First check if element is outside work area (behind taskbar)
        if !check_work_area(ex, ey, ew, eh) {
            tracing::info!("Element outside work area (possibly behind taskbar), need scroll");
            need_scroll = true;
        } else {
            // Try to get window bounds, but if that fails, use heuristics
            if let Ok(Some(win)) = element.window() {
                if let Ok((wx, wy, ww, wh)) = win.bounds() {
                    tracing::debug!("Window bounds: x={wx}, y={wy}, w={ww}, h={wh}");

                    let e_box = (ex, ey, ew, eh);
                    let w_box = (wx, wy, ww, wh);
                    if !rects_intersect(e_box, w_box) {
                        tracing::info!("Element NOT in viewport, need scroll");
                        need_scroll = true;
                    } else {
                        tracing::debug!(
                            "Element IS in viewport and work area, no scroll needed"
                        );
                    }
                } else {
                    need_scroll = check_element_needs_scroll_by_y(ey);
                }
            } else {
                need_scroll = check_element_needs_scroll_by_y(ey);
            }
        }
    } else if !element.is_visible().unwrap_or(true) {
        tracing::info!("Element not visible, needs scroll");
        need_scroll = true;
    }

    if need_scroll {
        // First try focusing the element to allow the application to auto-scroll it into view
        tracing::info!("Element outside viewport; attempting focus() to auto-scroll into view");
        match element.focus() {
            Ok(()) => {
                // Re-check visibility/intersection after focus
                std::thread::sleep(std::time::Duration::from_millis(50));

                let mut still_offscreen = false;
                if let Ok((_, ey2, _, _)) = element.bounds() {
                    tracing::debug!("After focus(), element Y={ey2}");
                    // Use same heuristic as before
                    if ey2 > 1080.0 {
                        tracing::debug!("After focus(), element Y={ey2} still > 1080");
                        still_offscreen = true;
                    } else {
                        tracing::info!("Focus() brought element into view");
                    }
                } else if !element.is_visible().unwrap_or(true) {
                    still_offscreen = true;
                }

                if !still_offscreen {
                    tracing::info!(
                        "Focus() brought element into view; skipping scroll_into_view"
                    );
                    need_scroll = false;
                } else {
                    tracing::info!("Focus() did not bring element into view; will attempt scroll_into_view()");
                }
            }
            Err(e) => {
                tracing::debug!("Focus() failed: {e}; will attempt scroll_into_view()");
            }
        }

        if need_scroll {
            tracing::info!("Element outside viewport; attempting scroll_into_view()");
            if let Err(e) = element.scroll_into_view() {
                tracing::warn!("scroll_into_view failed: {e}");
                // Don't return error, scrolling is best-effort
            } else {
                tracing::info!("scroll_into_view succeeded");

                // After initial scroll, verify element position and adjust if needed
                std::thread::sleep(std::time::Duration::from_millis(50)); // Let initial scroll settle

                if let Ok((_, ey, _, eh)) = element.bounds() {
                    tracing::debug!("After scroll_into_view, element at y={ey}");

                    let (viewport_top_edge, viewport_optimal_bottom, viewport_bottom_edge) =
                        get_viewport_bounds();

                    // Check if we have window bounds for more accurate positioning
                    let mut needs_adjustment = false;
                    let mut adjustment_direction: Option<&str> = None;
                    let adjustment_amount: f64 = 0.3; // Smaller adjustment

                    if let Ok(Some(window)) = element.window() {
                        if let Ok((_, wy, _, wh)) = window.bounds() {
                            // We have window bounds - use precise positioning
                            let element_relative_y = ey - wy;
                            let element_bottom = element_relative_y + eh;

                            tracing::debug!(
                                "Element relative_y={element_relative_y}, window_height={wh}"
                            );

                            // Check if element is poorly positioned
                            if element_relative_y < 50.0 {
                                // Too close to top - scroll up a bit
                                tracing::debug!(
                                    "Element too close to top ({element_relative_y}px)"
                                );
                                needs_adjustment = true;
                                adjustment_direction = Some("up");
                            } else if element_bottom > wh - 50.0 {
                                // Too close to bottom or cut off - scroll down a bit
                                tracing::debug!("Element too close to bottom or cut off");
                                needs_adjustment = true;
                                adjustment_direction = Some("down");
                            } else if element_relative_y > wh * 0.7 {
                                // Element is in lower 30% of viewport - not ideal
                                tracing::debug!("Element in lower portion of viewport");
                                needs_adjustment = true;
                                adjustment_direction = Some("down");
                            }
                        } else {
                            // No window bounds - use heuristic based on absolute Y position
                            if ey < viewport_top_edge {
                                tracing::debug!(
                                    "Element at y={ey} < {viewport_top_edge}, too high"
                                );
                                needs_adjustment = true;
                                adjustment_direction = Some("up");
                            } else if ey > viewport_bottom_edge {
                                tracing::debug!(
                                    "Element at y={ey} > {viewport_bottom_edge}, too low"
                                );
                                needs_adjustment = true;
                                adjustment_direction = Some("down");
                            } else if ey > viewport_optimal_bottom {
                                // Element is lower than optimal but not at edge
                                tracing::debug!("Element at y={ey} lower than optimal");
                                needs_adjustment = true;
                                adjustment_direction = Some("down");
                            }
                        }
                    } else {
                        // No window available - use simple heuristics
                        if !(viewport_top_edge..=viewport_bottom_edge).contains(&ey) {
                            needs_adjustment = true;
                            adjustment_direction = Some(if ey < 500.0 { "up" } else { "down" });
                            tracing::debug!("Element at y={ey} outside optimal range");
                        }
                    }

                    // Apply fine-tuning adjustment if needed
                    if needs_adjustment {
                        if let Some(dir) = adjustment_direction {
                            tracing::debug!(
                                "Fine-tuning position: scrolling {dir} by {adjustment_amount}"
                            );
                            let _ = element.scroll(dir, adjustment_amount);
                            std::thread::sleep(std::time::Duration::from_millis(30));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Ensures element is visible and applies highlighting before action with hardcoded defaults.
pub fn ensure_visible_and_apply_highlight(element: &UIElement, action_name: &str) {
    // Always ensure element is in view first (for all actions, not just when highlighting)
    if let Err(e) = ensure_element_in_view(element) {
        tracing::warn!("Failed to ensure element is in view for {action_name} action: {e}");
    }

    // Hardcoded highlight configuration
    let duration = Some(std::time::Duration::from_millis(500));
    let color = Some(0x00FF00); // Green in BGR
    let role_text = element.role();
    let text = Some(role_text.as_str());

    #[cfg(target_os = "windows")]
    let text_position = Some(crate::mcp_types::TextPosition::Top.into());
    #[cfg(not(target_os = "windows"))]
    let text_position = None;

    #[cfg(target_os = "windows")]
    let font_style = Some(
        crate::mcp_types::FontStyle {
            size: 12,
            bold: true,
            color: 0xFFFFFF, // White text
        }
        .into(),
    );
    #[cfg(not(target_os = "windows"))]
    let font_style = None;

    tracing::info!(
        "HIGHLIGHT_BEFORE_{} duration={:?} role={}",
        action_name.to_uppercase(),
        duration,
        role_text
    );
    if let Ok(_highlight_handle) =
        element.highlight(color, duration, text, text_position, font_style)
    {
        // Highlight applied successfully - runs concurrently with action
    } else {
        tracing::warn!("Failed to apply highlighting before {action_name} action");
    }
}
