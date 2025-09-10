#![cfg(target_os = "windows")]

use anyhow::{anyhow, Context, Result};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::process::Command;
use tracing::{debug, instrument, warn};
use windows::core::PCWSTR;
use windows::Win32::Foundation::BOOL;
use windows::Win32::Graphics::Gdi::{
    DisplayConfigGetDeviceInfo, EnumDisplaySettingsW, GetDisplayConfigBufferSizes,
    QueryDisplayConfig, DEVMODEW, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_ACTIVE, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_TARGET_DEVICE_NAME, ENUM_CURRENT_SETTINGS, QDC_ONLY_ACTIVE_PATHS,
};

use crate::DisplayTargetInfo;

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Retrieve current mode for a display device by device name using EnumDisplaySettingsW.
fn query_current_mode(device_name: &str) -> Option<(u32, u32, u32)> {
    unsafe {
        let mut devmode: DEVMODEW = std::mem::zeroed();
        devmode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
        let name_w = to_wide(device_name);
        let ok: BOOL =
            EnumDisplaySettingsW(PCWSTR(name_w.as_ptr()), ENUM_CURRENT_SETTINGS, &mut devmode);
        if ok.as_bool() {
            let width = devmode.dmPelsWidth;
            let height = devmode.dmPelsHeight;
            // dmDisplayFrequency may be zero for some paths; default to 60
            let refresh = if devmode.dmDisplayFrequency == 0 {
                60
            } else {
                devmode.dmDisplayFrequency
            };
            Some((width, height, refresh))
        } else {
            None
        }
    }
}

#[instrument]
pub fn enumerate_displays() -> Result<Vec<DisplayTargetInfo>> {
    unsafe {
        // First, query the buffer sizes
        let mut path_count: u32 = 0;
        let mut mode_count: u32 = 0;
        let status =
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count);
        if status != 0 {
            return Err(anyhow!("GetDisplayConfigBufferSizes failed: {status}"));
        }

        let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> = vec![std::mem::zeroed(); path_count as usize];
        let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> = vec![std::mem::zeroed(); mode_count as usize];
        let status = QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            std::ptr::null_mut(),
        );
        if status != 0 {
            return Err(anyhow!("QueryDisplayConfig failed: {status}"));
        }

        paths.truncate(path_count as usize);
        modes.truncate(mode_count as usize);

        let mut results: Vec<DisplayTargetInfo> = Vec::new();

        for path in &paths {
            let target_id = path.targetInfo.id;
            let adapter_id_low = path.targetInfo.adapterId.LowPart;
            let adapter_id_high = path.targetInfo.adapterId.HighPart as u32;
            let is_active = path.flags & DISPLAYCONFIG_PATH_ACTIVE != 0;

            // Try to map to a friendly device name via DISPLAYCONFIG_TARGET_DEVICE_NAME
            let mut name = None;
            let mut dev_name_buf: DISPLAYCONFIG_TARGET_DEVICE_NAME = std::mem::zeroed();
            dev_name_buf.header.adapterId = path.targetInfo.adapterId;
            dev_name_buf.header.id = target_id;
            dev_name_buf.header.size =
                std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            dev_name_buf.header._type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;

            let status = DisplayConfigGetDeviceInfo(&mut dev_name_buf.header);
            if status == 0 {
                if let Some(first_null) = dev_name_buf
                    .monitorFriendlyDeviceName
                    .iter()
                    .position(|&c| c == 0)
                {
                    let slice = &dev_name_buf.monitorFriendlyDeviceName[..first_null];
                    name = Some(String::from_utf16_lossy(slice));
                }
            } else {
                warn!("DisplayConfigGetDeviceInfo(GET_TARGET_NAME) failed: {status}");
            }

            // Estimate resolution/refresh via EnumDisplaySettingsW by using the output device path
            // For simplicity we use the friendly device name if present; otherwise default placeholders
            let (width, height, refresh_hz) = if let Some(ref friendly) = name {
                query_current_mode(friendly).unwrap_or((0, 0, 0))
            } else {
                (0, 0, 0)
            };

            results.push(DisplayTargetInfo {
                adapter_id_low,
                adapter_id_high,
                target_id,
                is_active,
                width,
                height,
                refresh_hz,
                friendly_name: name,
            });
        }

        Ok(results)
    }
}

#[instrument]
pub fn is_virtual_driver_present() -> Result<bool> {
    // Heuristic: look for display devices with source of type DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_WIRED
    // or indirect wireless; or enumerate PnP drivers in the Display class via pnputil /enum-drivers
    // and look for an INF that contains "IddCx" in its provider/section. We keep it simple: call pnputil.
    let output = Command::new("pnputil")
        .arg("/enum-drivers")
        .output()
        .context("Failed to execute pnputil /enum-drivers")?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let found = stdout.to_ascii_lowercase().contains("iddcx");
    Ok(found)
}

#[instrument]
pub fn install_virtual_driver_via_pnputil(inf_path: &str) -> Result<bool> {
    let output = Command::new("pnputil")
        .arg("/add-driver")
        .arg(inf_path)
        .arg("/install")
        .output()
        .with_context(|| format!("Failed to execute pnputil to add driver: {inf_path}"))?;
    let success = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout);
    debug!(?success, stdout = %stdout, "pnputil add-driver result");
    Ok(success || stdout.contains("was added") || stdout.contains("successfully"))
}

#[instrument]
pub fn uninstall_virtual_driver_via_pnputil(published_name: &str) -> Result<bool> {
    let output = Command::new("pnputil")
        .arg("/delete-driver")
        .arg(published_name)
        .arg("/uninstall")
        .output()
        .with_context(|| format!("Failed to execute pnputil to delete driver: {published_name}"))?;
    let success = output.status.success();
    let stdout = String::from_utf8_lossy(&output.stdout);
    debug!(?success, stdout = %stdout, "pnputil delete-driver result");
    Ok(success || stdout.contains("was deleted") || stdout.contains("successfully"))
}
