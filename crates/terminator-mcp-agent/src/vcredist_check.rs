use std::path::PathBuf;
use tracing::{info, warn};

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
