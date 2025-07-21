//! Experimental support for Windows CoreAutomationRemoteOperation
//!
//! NOTE: this is *very* early-stage and only exposes a single helper that
//! demonstrates how to create a `CoreAutomationRemoteOperation` instance,
//! import a UIA element into the operation and attempt to invoke it.  The
//! API surface will evolve once we migrate more of `WindowsEngine` over to
//! remote-execution internally.
//!
//! The implementation is deliberately kept small so it can compile on older
//! Windows SDKs – we avoid fancy builder helpers and call the raw WinRT
//! methods instead.  Error handling is translated into Terminator’s
//! `AutomationError` so callers do not have to depend on WinRT’s `Error`.

#![cfg(target_os = "windows")]

use std::sync::Arc;

use crate::AutomationError;
use crate::platforms::windows::WindowsUIElement;

// WinRT Core UI Automation namespace (requires the `Windows_UI_UIAutomation_Core` feature
// to be enabled for the `windows` crate – see `Cargo.toml`).
use windows::UI::UIAutomation::Core::CoreAutomationRemoteOperation;

/// Attempts to perform an *Invoke* action on the supplied element entirely via
/// `CoreAutomationRemoteOperation` (i.e. no cross-process COM calls once the
/// byte-code is marshalled).
///
/// On platforms where Core Automation is not available (pre-Windows 10 20H2)
/// the function returns `AutomationError::UnsupportedOperation`.
pub fn invoke_element_via_remote(element: &WindowsUIElement) -> Result<(), AutomationError> {
    // The WinRT API is only present on recent builds – fail fast if the DLL
    // cannot be loaded.
    let remote_op = match CoreAutomationRemoteOperation::new() {
        Ok(op) => op,
        Err(_) => {
            return Err(AutomationError::UnsupportedOperation(
                "CoreAutomationRemoteOperation is not available on this version of Windows"
                    .to_string(),
            ))
        }
    };

    // Safety: the underlying `uiautomation::UIElement` already implements the
    // correct COM interface – we just need the raw pointer so WinRT can pin it
    // inside the remote-operation’s object table.  Unfortunately the public
    // `uiautomation` crate does not expose the COM pointer so we cannot (yet)
    // fully wire this up without modifications upstream.
    //
    // For now we abort with an explanatory error so callers understand what is
    // missing instead of silently doing nothing.
    return Err(AutomationError::UnsupportedOperation(
        "invoke_element_via_remote() is not wired-up yet – waiting on low-level \\" \
        "access to the underlying COM pointer in uiautomation::UIElement.\""
            .to_string(),
    ));

    // --- planned implementation (kept as comment for future reference) ---
    // let token = remote_op.ImportElement(&element.element.0 /* raw COM pointer */)?;
    // remote_op.Invoke(token)?;               // enqueue InvokePattern.Invoke()
    // remote_op.Execute()?;                   // transmit byte-code to the target session
    // Ok(())
}