use std::path::PathBuf;
use std::process::Command;
use tracing::{error, info, warn};

/// Check if Visual C++ Redistributables are installed on Windows
/// This is checked once at startup to avoid runtime overhead
pub fn check_vcredist_installed() -> bool {
    // Only relevant on Windows
    if !cfg!(windows) {
        return true;
    }

    // Check for the presence of key VC++ runtime DLLs
    // These are installed by Visual C++ Redistributables 2015-2022
    let system32_path = if let Ok(windir) = std::env::var("WINDIR") {
        PathBuf::from(windir).join("System32")
    } else {
        PathBuf::from("C:\\Windows\\System32")
    };

    // Key DLLs that should exist if VC++ redistributables are installed
    let required_dlls = [
        "vcruntime140.dll",
        "vcruntime140_1.dll", // Additional runtime for x64
        "msvcp140.dll",       // C++ standard library
    ];

    let mut all_found = true;
    let mut missing_dlls = Vec::new();

    for dll in &required_dlls {
        let dll_path = system32_path.join(dll);
        if !dll_path.exists() {
            all_found = false;
            missing_dlls.push(dll.to_string());
        }
    }

    if !all_found {
        warn!(
            "====================================================================\n\
             WARNING: Visual C++ Redistributables are not installed!\n\
             ====================================================================\n\
             Missing DLLs: {}\n\
             \n\
             JavaScript/TypeScript execution with terminator.js will fail.\n\
             \n\
             To fix this issue, install Visual C++ Redistributables 2015-2022:\n\
               winget install Microsoft.VCRedist.2015+.x64\n\
             \n\
             Or download from:\n\
               https://aka.ms/vs/17/release/vc_redist.x64.exe\n\
             ====================================================================",
            missing_dlls.join(", ")
        );
    } else {
        info!("Visual C++ Redistributables check: OK");
    }

    all_found
}

/// Get a user-friendly error message for VC++ redistributables
pub fn get_vcredist_error_message() -> &'static str {
    "JavaScript/TypeScript execution failed because Visual C++ Redistributables are not installed.\n\
     \n\
     To fix this issue, run:\n\
       winget install Microsoft.VCRedist.2015+.x64\n\
     \n\
     Or download from:\n\
       https://aka.ms/vs/17/release/vc_redist.x64.exe"
}

/// Check if an error is related to missing VC++ redistributables
pub fn is_vcredist_error(error_message: &str) -> bool {
    error_message.contains("ERR_DLOPEN_FAILED")
        || error_message.contains("specified module could not be found")
        || error_message.contains("terminator.win32")
}

// Cache the check result to avoid repeated filesystem access
static mut VCREDIST_CHECKED: bool = false;
static mut VCREDIST_INSTALLED: bool = false;

/// Get cached VC++ redistributables status (call after initial check)
pub fn is_vcredist_available() -> bool {
    unsafe {
        if !VCREDIST_CHECKED {
            VCREDIST_INSTALLED = check_vcredist_installed();
            VCREDIST_CHECKED = true;
        }
        VCREDIST_INSTALLED
    }
}

/// Attempt to automatically install VC++ Redistributables
/// Returns true if installation succeeded or was skipped, false if it failed
pub fn try_auto_install_vcredist() -> bool {
    // Only relevant on Windows
    if !cfg!(windows) {
        return true;
    }

    // Check if auto-install is disabled via environment variable
    if std::env::var("VCREDIST_AUTO_INSTALL")
        .unwrap_or_default()
        .eq_ignore_ascii_case("false")
    {
        info!("VC++ auto-install disabled via VCREDIST_AUTO_INSTALL=false");
        return false;
    }

    info!("Attempting to auto-install Visual C++ Redistributables...");

    // Try winget first (cleanest method, built into Windows 11+)
    if try_install_via_winget() {
        info!("Successfully installed VC++ Redistributables via winget");
        return true;
    }

    // Fall back to direct download + silent install
    info!("winget installation failed, trying direct download method...");
    if try_install_via_download() {
        info!("Successfully installed VC++ Redistributables via direct download");
        return true;
    }

    error!("Failed to auto-install VC++ Redistributables. Please install manually.");
    false
}

/// Try to install VC++ Redistributables using winget
fn try_install_via_winget() -> bool {
    info!("Trying winget installation...");

    // Check if winget is available
    let winget_check = Command::new("winget")
        .arg("--version")
        .output();

    if winget_check.is_err() {
        warn!("winget not available on this system");
        return false;
    }

    // Run winget install with --silent and --accept-package-agreements flags
    let result = Command::new("winget")
        .args([
            "install",
            "--id",
            "Microsoft.VCRedist.2015+.x64",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ])
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                info!("winget installation completed successfully");
                true
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("winget installation failed: {}", stderr);
                false
            }
        }
        Err(e) => {
            warn!("Failed to execute winget: {}", e);
            false
        }
    }
}

/// Try to install VC++ Redistributables via direct download
fn try_install_via_download() -> bool {
    info!("Downloading VC++ Redistributables installer...");

    // URL for VC++ Redistributables 2015-2022 x64
    let download_url = "https://aka.ms/vs/17/release/vc_redist.x64.exe";

    // Get temp directory
    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join("vc_redist.x64.exe");

    // Download the installer using PowerShell (available on all Windows systems)
    let download_result = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Invoke-WebRequest -Uri '{}' -OutFile '{}'",
                download_url,
                installer_path.display()
            ),
        ])
        .output();

    match download_result {
        Ok(output) if output.status.success() => {
            info!("Downloaded installer to {}", installer_path.display());
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to download installer: {}", stderr);
            return false;
        }
        Err(e) => {
            warn!("Failed to execute PowerShell download: {}", e);
            return false;
        }
    }

    // Run the installer silently
    info!("Running VC++ Redistributables installer silently...");
    let install_result = Command::new(&installer_path)
        .args(["/install", "/quiet", "/norestart"])
        .output();

    // Clean up installer file
    let _ = std::fs::remove_file(&installer_path);

    match install_result {
        Ok(output) => {
            if output.status.success() {
                info!("Direct download installation completed successfully");
                true
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Installer execution failed: {}", stderr);
                false
            }
        }
        Err(e) => {
            warn!("Failed to execute installer: {}", e);
            false
        }
    }
}
