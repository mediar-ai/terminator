//! Windows virtual display management helpers.
//!
//! This crate provides a minimal surface for:
//! - Enumerating current display paths and targets (active monitors)
//! - Stubs for installing/uninstalling a virtual display driver (IddCx-based)
//!
//! Limitations:
//! - This crate does not create a virtual monitor by itself. A signed IddCx
//!   kernel-mode driver is required to expose a new display surface to Windows.
//! - The provided installation helpers shell out to `pnputil` and require
//!   Administrator privileges when adding/removing drivers.
//!
//! It is intended as an integration seam for agents that require a persistent
//! desktop surface inside a VM without relying on active RDP sessions.
//!
//! Notes:
//! - Creating a true virtual display requires a kernel-mode Indirect Display
//!   Driver (IddCx). This crate does not implement the driver; it only provides
//!   user-mode helpers and a future-friendly API surface.
//! - For CI/VM scenarios, you can preinstall an IddCx driver and use this crate
//!   to verify presence and enumerate surfaces.

#![cfg_attr(not(target_os = "windows"), allow(unused))]

use anyhow::{anyhow, Result};
use tracing::{debug, instrument};

#[cfg(target_os = "windows")]
mod windows_impl;

/// Describes a display target (monitor) discovered via Windows DisplayConfig.
#[derive(Debug, Clone)]
pub struct DisplayTargetInfo {
    pub adapter_id_low: u32,
    pub adapter_id_high: u32,
    pub target_id: u32,
    pub is_active: bool,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
    pub friendly_name: Option<String>,
}

/// Enumerate active display targets.
#[instrument]
pub fn enumerate_displays() -> Result<Vec<DisplayTargetInfo>> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::enumerate_displays()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err(anyhow!("virtual-display-win only supports Windows"))
    }
}

/// Check whether a known virtual display driver is installed.
///
/// This is a best-effort heuristic placeholder; production logic should
/// validate the specific driver package or device class.
#[instrument]
pub fn is_virtual_driver_present() -> Result<bool> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::is_virtual_driver_present()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

/// Attempt to install a virtual display driver from an `.inf` path using pnputil.
///
/// This requires Administrator privileges. The function returns Ok(true) if
/// the command succeeded, Ok(false) if the command ran but did not report success,
/// and Err on execution failures.
#[instrument]
pub fn install_virtual_driver_via_pnputil(inf_path: &str) -> Result<bool> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::install_virtual_driver_via_pnputil(inf_path)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err(anyhow!("Not supported on this platform"))
    }
}

/// Attempt to uninstall a virtual display driver by published name using pnputil.
#[instrument]
pub fn uninstall_virtual_driver_via_pnputil(published_name: &str) -> Result<bool> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::uninstall_virtual_driver_via_pnputil(published_name)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err(anyhow!("Not supported on this platform"))
    }
}
