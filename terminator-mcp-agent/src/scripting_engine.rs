use rmcp::ErrorData as McpError;
use serde_json::json;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

/// Find executable with cross-platform path resolution
pub fn find_executable(name: &str) -> Option<String> {
    use std::env;
    use std::path::Path;

    // On Windows, try multiple extensions, prioritizing executable types
    let candidates = if cfg!(windows) {
        vec![
            format!("{}.exe", name),
            format!("{}.cmd", name),
            format!("{}.bat", name),
            name.to_string(),
        ]
    } else {
        vec![name.to_string()]
    };

    // Check each candidate in PATH
    if let Ok(path_var) = env::var("PATH") {
        let separator = if cfg!(windows) { ";" } else { ":" };

        for path_dir in path_var.split(separator) {
            let path_dir = Path::new(path_dir);

            for candidate in &candidates {
                let full_path = path_dir.join(candidate);
                if full_path.exists() && full_path.is_file() {
                    info!("Found executable: {}", full_path.display());
                    return Some(full_path.to_string_lossy().to_string());
                }
            }
        }
    }

    // Fallback: try the name as-is (might work on some systems)
    info!("Executable '{}' not found in PATH, using name as-is", name);
    Some(name.to_string())
}

/// Log the installed terminator.js version and platform package version (if present)
async fn log_terminator_js_version(script_dir: &std::path::Path, log_prefix: &str) {
    let main_pkg_path = script_dir
        .join("node_modules")
        .join("terminator.js")
        .join("package.json");

    let main_version = match tokio::fs::read_to_string(&main_pkg_path).await {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(pkg) => pkg
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            Err(_) => None,
        },
        Err(_) => None,
    };

    // Determine platform-specific package name (same logic as installer)
    let platform_package_name: Option<&'static str> =
        if cfg!(target_arch = "x86_64") && cfg!(target_os = "windows") {
            Some("terminator.js-win32-x64-msvc")
        } else if cfg!(target_arch = "aarch64") && cfg!(target_os = "windows") {
            Some("terminator.js-win32-arm64-msvc")
        } else if cfg!(target_arch = "x86_64") && cfg!(target_os = "macos") {
            Some("terminator.js-darwin-x64")
        } else if cfg!(target_arch = "aarch64") && cfg!(target_os = "macos") {
            Some("terminator.js-darwin-arm64")
        } else if cfg!(target_arch = "x86_64") && cfg!(target_os = "linux") {
            Some("terminator.js-linux-x64-gnu")
        } else {
            None
        };

    let platform_version = if let Some(pkg_name) = platform_package_name {
        let platform_pkg_path = script_dir
            .join("node_modules")
            .join(pkg_name)
            .join("package.json");
        match tokio::fs::read_to_string(&platform_pkg_path).await {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(pkg) => pkg
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                Err(_) => None,
            },
            Err(_) => None,
        }
    } else {
        None
    };

    match (main_version, platform_package_name, platform_version) {
        (Some(mv), Some(ppn), Some(pv)) => {
            info!(
                "[{}] Using terminator.js version {} (platform package {}@{})",
                log_prefix, mv, ppn, pv
            );
        }
        (Some(mv), _, _) => {
            info!("[{}] Using terminator.js version {}", log_prefix, mv);
        }
        _ => {
            info!(
                "[{}] Could not determine terminator.js version (package.json not found)",
                log_prefix
            );
        }
    }
}

/// Ensure terminator.js is installed in a persistent directory and return the script directory
async fn ensure_terminator_js_installed(runtime: &str) -> Result<std::path::PathBuf, McpError> {
    info!("[{}] Checking if terminator.js is installed...", runtime);

    // Use a persistent directory instead of a new temp directory each time
    let script_dir = std::env::temp_dir().join("terminator_mcp_persistent");

    // Check if we need to install or update terminator.js
    let node_modules_path = script_dir.join("node_modules").join("terminator.js");
    let package_json_path = script_dir.join("package.json");

    // Create the persistent directory if it doesn't exist
    tokio::fs::create_dir_all(&script_dir).await.map_err(|e| {
        McpError::internal_error(
            "Failed to create persistent script directory",
            Some(json!({"error": e.to_string()})),
        )
    })?;

    // Always create/update package.json to ensure we use latest with all platform packages
    let package_json_content = r#"{
  "name": "terminator-mcp-persistent",
  "version": "1.0.0",
  "dependencies": {
    "terminator.js": "latest"
  },
  "optionalDependencies": {
    "terminator.js-darwin-arm64": "latest",
    "terminator.js-darwin-x64": "latest", 
    "terminator.js-linux-x64-gnu": "latest",
    "terminator.js-win32-arm64-msvc": "latest",
    "terminator.js-win32-x64-msvc": "latest"
  }
}"#;

    tokio::fs::write(&package_json_path, package_json_content)
        .await
        .map_err(|e| {
            McpError::internal_error(
                "Failed to write package.json",
                Some(json!({"error": e.to_string()})),
            )
        })?;

    // Check if we need to install/reinstall
    let should_install = !node_modules_path.exists();

    // Also check if we should update (check once per day by default)
    let mut should_check_update = if node_modules_path.exists() {
        // Check the modification time of node_modules/terminator.js
        match tokio::fs::metadata(&node_modules_path).await {
            Ok(metadata) => {
                if let Ok(modified) = metadata.modified() {
                    // Update if older than 1 day
                    let age = std::time::SystemTime::now()
                        .duration_since(modified)
                        .unwrap_or(std::time::Duration::from_secs(0));
                    age.as_secs() > 86400 // 24 hours in seconds
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    } else {
        false
    };

    // Env override to force updates regardless of age
    // Set TERMINATOR_JS_UPDATE=always to force update on every run
    if std::env::var("TERMINATOR_JS_UPDATE")
        .map(|v| v.eq_ignore_ascii_case("always"))
        .unwrap_or(false)
    {
        info!(
            "[{}] Forced update enabled via TERMINATOR_JS_UPDATE=always",
            runtime
        );
        should_check_update = true;
    }

    // Check if platform-specific package exists for current platform
    let platform_package_name = if cfg!(target_arch = "x86_64") && cfg!(target_os = "windows") {
        "terminator.js-win32-x64-msvc"
    } else if cfg!(target_arch = "aarch64") && cfg!(target_os = "windows") {
        "terminator.js-win32-arm64-msvc"
    } else if cfg!(target_arch = "x86_64") && cfg!(target_os = "macos") {
        "terminator.js-darwin-x64"
    } else if cfg!(target_arch = "aarch64") && cfg!(target_os = "macos") {
        "terminator.js-darwin-arm64"
    } else if cfg!(target_arch = "x86_64") && cfg!(target_os = "linux") {
        "terminator.js-linux-x64-gnu"
    } else {
        ""
    };

    let platform_package_exists = if !platform_package_name.is_empty() {
        script_dir
            .join("node_modules")
            .join(platform_package_name)
            .exists()
    } else {
        true // Skip platform check for unsupported platforms
    };

    if should_install {
        info!(
            "[{}] terminator.js not found, installing latest version...",
            runtime
        );
    } else if should_check_update {
        info!(
            "[{}] terminator.js is older than 1 day, checking for updates...",
            runtime
        );
    } else if !platform_package_exists {
        info!(
            "[{}] terminator.js found but platform package {} missing, reinstalling...",
            runtime, platform_package_name
        );
        // Remove existing node_modules to force clean reinstall
        let node_modules_dir = script_dir.join("node_modules");
        if node_modules_dir.exists() {
            info!(
                "[{}] Removing existing node_modules for clean reinstall...",
                runtime
            );
            if let Err(e) = tokio::fs::remove_dir_all(&node_modules_dir).await {
                info!(
                    "[{}] Failed to remove node_modules (continuing): {}",
                    runtime, e
                );
            }
        }
    } else if !should_check_update {
        info!(
            "[{}] terminator.js and platform package found, using existing installation...",
            runtime
        );
        return Ok(script_dir);
    }

    // Find the runtime executable
    let runtime_exe = find_executable(runtime).ok_or_else(|| {
        McpError::internal_error(format!("Could not find {runtime} executable"), None)
    })?;

    info!("[{}] Using executable: {}", runtime, runtime_exe);

    // Find npm/bun for installation
    let installer_exe = match runtime {
        "bun" => find_executable("bun").ok_or_else(|| {
            McpError::internal_error("Could not find bun executable for installation", None)
        })?,
        _ => find_executable("npm").ok_or_else(|| {
            McpError::internal_error("Could not find npm executable for installation", None)
        })?,
    };

    info!("[{}] Using installer: {}", runtime, installer_exe);

    info!(
        "[{}] Using persistent script directory: {}",
        runtime,
        script_dir.display()
    );

    // Install or reinstall terminator.js in the persistent directory
    // If updating, explicitly install latest for both main and platform packages
    let platform_pkg_opt: Option<&str> = if !platform_package_name.is_empty() {
        Some(platform_package_name)
    } else {
        None
    };

    let command_args: Vec<String> = if should_install || !platform_package_exists {
        // Fresh install or reinstall when platform package missing
        vec!["install".to_string()]
    } else if should_check_update {
        // Force upgrade to latest for both packages
        let mut args = vec!["install".to_string(), "terminator.js@latest".to_string()];
        if let Some(pp) = platform_pkg_opt {
            args.push(format!("{pp}@latest"));
        }
        args
    } else {
        // Default to install to reconcile lock if needed
        vec!["install".to_string()]
    };

    info!(
        "[{}] Preparing to run package manager with args: {:?}",
        runtime, command_args
    );

    // Add retry mechanism based on screenpipe patterns
    let max_retries = 3;
    let mut attempt = 0;

    loop {
        attempt += 1;
        info!(
            "[{}] Installation attempt {} of {}",
            runtime, attempt, max_retries
        );

        // Use spawn with real-time output to see what's happening
        let install_result = match runtime {
            "bun" => {
                info!(
                    "[{}] Running bun directly: {} {:?}",
                    runtime, installer_exe, command_args
                );
                info!("[{}] Spawning bun process...", runtime);
                let child = match tokio::process::Command::new(&installer_exe)
                    .current_dir(&script_dir)
                    .args(command_args.iter())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                {
                    Ok(child) => child,
                    Err(e) => {
                        error!("[{}] Failed to spawn bun process: {}", runtime, e);
                        return Err(McpError::internal_error(
                            "Failed to spawn bun process",
                            Some(json!({"error": e.to_string()})),
                        ));
                    }
                };

                info!(
                    "[{}] Bun process spawned, waiting for completion...",
                    runtime
                );
                child.wait_with_output().await
            }
            _ => {
                // On Windows, npm is a batch file, so we need to run it through cmd.exe
                if cfg!(windows) {
                    let mut cmd_args = vec!["/c", "npm"];
                    // Add comprehensive flags based on screenpipe patterns
                    cmd_args.extend(
                        [
                            "--verbose",
                            "--no-audit",
                            "--no-fund",
                            "--yes",
                            "--force",            // Force resolve conflicts
                            "--legacy-peer-deps", // Handle peer dependency issues
                            "--include=optional", // Ensure optional dependencies (platform packages) are installed
                            "--save-exact",       // Lock to exact versions to prevent mismatches
                            "--registry=https://registry.npmjs.org/", // Explicit registry
                        ]
                        .iter(),
                    );
                    cmd_args.extend(command_args.iter().map(|s| s.as_str()));
                    info!(
                        "[{}] Running npm via cmd.exe: cmd {:?} in directory {}",
                        runtime,
                        cmd_args,
                        script_dir.display()
                    );

                    info!("[{}] About to spawn cmd.exe process...", runtime);

                    // First, let's test if npm is working at all with a simple version check
                    // Debug: show what's in the directory and what we're trying to install
                    info!("[{}] Debugging directory contents...", runtime);
                    if let Ok(entries) = std::fs::read_dir(&script_dir) {
                        for entry in entries.flatten() {
                            info!(
                                "[{}] Directory contains: {}",
                                runtime,
                                entry.file_name().to_string_lossy()
                            );
                        }
                    }

                    if let Ok(package_json) =
                        std::fs::read_to_string(script_dir.join("package.json"))
                    {
                        info!("[{}] package.json contents:\n{}", runtime, package_json);
                    } else {
                        error!("[{}] Could not read package.json!", runtime);
                    }

                    info!("[{}] Testing npm connectivity first...", runtime);
                    let test_result = tokio::process::Command::new("cmd")
                        .current_dir(&script_dir)
                        .args(["/c", "npm", "--version"])
                        .output()
                        .await;

                    match test_result {
                        Ok(output) if output.status.success() => {
                            let version = String::from_utf8_lossy(&output.stdout);
                            info!(
                                "[{}] npm version test successful: {}",
                                runtime,
                                version.trim()
                            );
                        }
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            error!("[{}] npm version test failed: {}", runtime, stderr);
                            info!(
                                "[{}] npm stdout: {}",
                                runtime,
                                String::from_utf8_lossy(&output.stdout)
                            );
                        }
                        Err(e) => {
                            error!("[{}] Failed to run npm version test: {}", runtime, e);
                        }
                    }

                    // Now test npm registry connectivity
                    info!("[{}] Testing npm registry connectivity...", runtime);
                    let registry_test = tokio::process::Command::new("cmd")
                        .current_dir(&script_dir)
                        .args(["/c", "npm", "ping"])
                        .output()
                        .await;

                    match registry_test {
                        Ok(output) if output.status.success() => {
                            let ping_result = String::from_utf8_lossy(&output.stdout);
                            info!(
                                "[{}] npm registry ping successful: {}",
                                runtime,
                                ping_result.trim()
                            );
                        }
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            error!("[{}] npm registry ping failed: {}", runtime, stderr);
                            info!(
                                "[{}] npm ping stdout: {}",
                                runtime,
                                String::from_utf8_lossy(&output.stdout)
                            );
                        }
                        Err(e) => {
                            error!("[{}] Failed to run npm ping test: {}", runtime, e);
                        }
                    }
                    let spawn_result = tokio::process::Command::new("cmd")
                        .current_dir(&script_dir)
                        .args(&cmd_args)
                        // Core npm configuration (based on screenpipe patterns)
                        .env("NPM_CONFIG_REGISTRY", "https://registry.npmjs.org/")
                        .env("NPM_CONFIG_LOGLEVEL", "info")
                        .env("NPM_CONFIG_PROGRESS", "true")
                        .env("NPM_CONFIG_YES", "true")
                        .env("NPM_CONFIG_FORCE", "true")
                        .env("NPM_CONFIG_LEGACY_PEER_DEPS", "true")
                        .env("NPM_CONFIG_INCLUDE", "optional")
                        .env("NPM_CONFIG_SAVE_EXACT", "true")
                        .env("NPM_CONFIG_NO_AUDIT", "true")
                        .env("NPM_CONFIG_NO_FUND", "true")
                        // Network and timeout configuration
                        .env("NPM_CONFIG_FETCH_TIMEOUT", "60000") // 60 second timeout
                        .env("NPM_CONFIG_FETCH_RETRY_MINTIMEOUT", "10000") // 10 sec min retry
                        .env("NPM_CONFIG_FETCH_RETRY_MAXTIMEOUT", "60000") // 60 sec max retry
                        .env("NPM_CONFIG_FETCH_RETRIES", "3")
                        // CI and non-interactive mode
                        .env("CI", "true")
                        .env("NODE_ENV", "production")
                        .env("NPM_CONFIG_INTERACTIVE", "false")
                        .env("NPM_CONFIG_STDIN", "false")
                        // Cache configuration to prevent corruption issues
                        .env("NPM_CONFIG_CACHE_VERIFY", "false") // Skip cache verification
                        .env("NPM_CONFIG_PREFER_OFFLINE", "false") // Always try network first
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .spawn();

                    match spawn_result {
                        Ok(mut child) => {
                            info!(
                                "[{}] cmd.exe process spawned successfully, PID: {:?}",
                                runtime,
                                child.id()
                            );
                            info!("[{}] Waiting for npm install to complete...", runtime);

                            // Read stdout and stderr in real-time
                            use tokio::io::{AsyncBufReadExt, BufReader};

                            let stdout = child.stdout.take().unwrap();
                            let stderr = child.stderr.take().unwrap();

                            let mut stdout_reader = BufReader::new(stdout).lines();
                            let mut stderr_reader = BufReader::new(stderr).lines();

                            let timeout_duration = std::time::Duration::from_secs(300); // 5 minutes
                            let start_time = std::time::Instant::now();
                            let mut last_progress_time = start_time;

                            loop {
                                if start_time.elapsed() > timeout_duration {
                                    error!(
                                        "[{}] npm install timed out after {:?}, killing process",
                                        runtime, timeout_duration
                                    );
                                    let _ = child.kill().await;
                                    return Err(McpError::internal_error(
                                        "npm install timed out",
                                        Some(json!({"timeout_secs": timeout_duration.as_secs()})),
                                    ));
                                }

                                tokio::select! {
                                    // Read stdout
                                    stdout_line = stdout_reader.next_line() => {
                                        match stdout_line {
                                            Ok(Some(line)) => {
                                                info!("[{}] npm stdout: {}", runtime, line);
                                                last_progress_time = std::time::Instant::now(); // Reset progress timer on output
                                            }
                                            Ok(None) => {
                                                info!("[{}] npm stdout stream ended", runtime);
                                            }
                                            Err(e) => {
                                                error!("[{}] Error reading npm stdout: {}", runtime, e);
                                            }
                                        }
                                    }

                                    // Read stderr
                                    stderr_line = stderr_reader.next_line() => {
                                        match stderr_line {
                                            Ok(Some(line)) => {
                                                info!("[{}] npm stderr: {}", runtime, line);
                                                last_progress_time = std::time::Instant::now(); // Reset progress timer on output
                                            }
                                            Ok(None) => {
                                                info!("[{}] npm stderr stream ended", runtime);
                                            }
                                            Err(e) => {
                                                error!("[{}] Error reading npm stderr: {}", runtime, e);
                                            }
                                        }
                                    }

                                    // Check if process is done and show progress
                                    _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                                        match child.try_wait() {
                                            Ok(Some(status)) => {
                                                info!("[{}] npm process completed with status: {:?}", runtime, status);

                                                // Read any remaining output
                                                while let Ok(Some(line)) = stdout_reader.next_line().await {
                                                    if !line.is_empty() {
                                                        info!("[{}] npm stdout (final): {}", runtime, line);
                                                    }
                                                }
                                                while let Ok(Some(line)) = stderr_reader.next_line().await {
                                                    if !line.is_empty() {
                                                        info!("[{}] npm stderr (final): {}", runtime, line);
                                                    }
                                                }

                                                break if status.success() {
                                                    Ok(std::process::Output {
                                                        status,
                                                        stdout: Vec::new(), // We already logged the output
                                                        stderr: Vec::new(),
                                                    })
                                                } else {
                                                    Ok(std::process::Output {
                                                        status,
                                                        stdout: Vec::new(),
                                                        stderr: format!("npm exited with code {:?}", status.code()).into_bytes(),
                                                    })
                                                };
                                            }
                                            Ok(None) => {
                                                // Still running - show progress occasionally
                                                if last_progress_time.elapsed().as_secs() >= 10 {
                                                    info!(
                                                        "[{}] npm still running... ({:.1}s elapsed, waiting for output...)",
                                                        runtime,
                                                        start_time.elapsed().as_secs_f32()
                                                    );
                                                    last_progress_time = std::time::Instant::now();
                                                }
                                            }
                                            Err(e) => {
                                                error!("[{}] Error checking npm process status: {}", runtime, e);
                                                break Err(e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("[{}] Failed to spawn cmd.exe process: {}", runtime, e);
                            error!("[{}] Working directory: {}", runtime, script_dir.display());
                            error!("[{}] Command attempted: cmd {:?}", runtime, cmd_args);
                            Err(e)
                        }
                    }
                } else {
                    info!(
                        "[{}] Running npm directly: {} {:?}",
                        runtime, installer_exe, command_args
                    );
                    info!("[{}] Spawning npm process...", runtime);
                    let child = match tokio::process::Command::new(&installer_exe)
                        .current_dir(&script_dir)
                        .args(command_args.iter())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .spawn()
                    {
                        Ok(child) => child,
                        Err(e) => {
                            error!("[{}] Failed to spawn npm process: {}", runtime, e);
                            return Err(McpError::internal_error(
                                "Failed to spawn npm process",
                                Some(json!({"error": e.to_string()})),
                            ));
                        }
                    };

                    info!(
                        "[{}] npm process spawned, waiting for completion...",
                        runtime
                    );
                    child.wait_with_output().await
                }
            }
        };

        info!("[{}] Package manager command execution initiated", runtime);

        match install_result {
            Ok(output) => {
                info!(
                    "[{}] Package manager command completed with exit status: {:?}",
                    runtime, output.status
                );

                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if !stdout.is_empty() {
                    info!("[{}] Package manager stdout:\n{}", runtime, stdout);
                }
                if !stderr.is_empty() {
                    info!("[{}] Package manager stderr:\n{}", runtime, stderr);
                }

                if output.status.success() {
                    let action = if should_install {
                        "installed"
                    } else {
                        "updated"
                    };
                    info!(
                        "[{}] terminator.js {} successfully to latest version in persistent directory",
                        runtime, action
                    );

                    // Check for version mismatches and fix them
                    if !platform_package_name.is_empty() {
                        let main_pkg_json = script_dir
                            .join("node_modules")
                            .join("terminator.js")
                            .join("package.json");
                        let platform_pkg_json = script_dir
                            .join("node_modules")
                            .join(platform_package_name)
                            .join("package.json");

                        if let (Ok(main_content), Ok(platform_content)) = (
                            tokio::fs::read_to_string(&main_pkg_json).await,
                            tokio::fs::read_to_string(&platform_pkg_json).await,
                        ) {
                            if let (Ok(main_pkg), Ok(platform_pkg)) = (
                                serde_json::from_str::<serde_json::Value>(&main_content),
                                serde_json::from_str::<serde_json::Value>(&platform_content),
                            ) {
                                let main_version = main_pkg.get("version").and_then(|v| v.as_str());
                                let platform_version =
                                    platform_pkg.get("version").and_then(|v| v.as_str());

                                if let (Some(mv), Some(pv)) = (main_version, platform_version) {
                                    if mv != pv {
                                        info!(
                                            "[{}] Version mismatch detected: terminator.js@{} vs {}@{}",
                                            runtime, mv, platform_package_name, pv
                                        );
                                        info!(
                                            "[{}] Resolving by upgrading both packages to latest...",
                                            runtime
                                        );

                                        let upgrade_result = if runtime == "bun" {
                                            // bun: add both at latest
                                            let args = [
                                                "add".to_string(),
                                                "terminator.js@latest".to_string(),
                                                format!("{platform_package_name}@latest"),
                                            ];
                                            tokio::process::Command::new(&installer_exe)
                                                .current_dir(&script_dir)
                                                .args(args.iter())
                                                .output()
                                                .await
                                        } else if cfg!(windows) {
                                            // npm on Windows via cmd
                                            tokio::process::Command::new("cmd")
                                                .current_dir(&script_dir)
                                                .args([
                                                    "/c",
                                                    "npm",
                                                    "install",
                                                    "terminator.js@latest",
                                                    &format!("{platform_package_name}@latest"),
                                                ])
                                                .output()
                                                .await
                                        } else {
                                            // npm direct
                                            tokio::process::Command::new("npm")
                                                .current_dir(&script_dir)
                                                .args([
                                                    "install",
                                                    "terminator.js@latest",
                                                    &format!("{platform_package_name}@latest"),
                                                ])
                                                .output()
                                                .await
                                        };

                                        match upgrade_result {
                                            Ok(out) if out.status.success() => {
                                                info!("[{}] Successfully upgraded terminator.js and platform package to latest", runtime);
                                            }
                                            Ok(out) => {
                                                let stderr = String::from_utf8_lossy(&out.stderr);
                                                warn!("[{}] Failed to upgrade both packages (continuing): {}", runtime, stderr);
                                            }
                                            Err(e) => {
                                                warn!("[{}] Error upgrading both packages (continuing): {}", runtime, e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    return Ok(script_dir); // Success - exit retry loop
                } else {
                    let action = if should_install { "install" } else { "update" };
                    let error_msg = format!(
                        "Failed to {} terminator.js with exit code {:?}: {}",
                        action,
                        output.status.code(),
                        stderr
                    );
                    error!("[{}] {}", runtime, error_msg);

                    let error = McpError::internal_error(
                        format!("Failed to {action} terminator.js"),
                        Some(json!({
                            "error": stderr.to_string(),
                            "stdout": stdout.to_string(),
                            "exit_code": output.status.code(),
                            "attempt": attempt,
                            "max_retries": max_retries
                        })),
                    );

                    // Check if we should retry
                    if attempt >= max_retries {
                        error!(
                            "[{}] All {} installation attempts failed",
                            runtime, max_retries
                        );
                        return Err(error);
                    }
                }
            }
            Err(e) => {
                let action = if should_install { "install" } else { "update" };
                let error_msg = format!("Failed to run {action} command: {e}");
                error!("[{}] {}", runtime, error_msg);

                let error = McpError::internal_error(
                    "Failed to run package manager",
                    Some(json!({
                        "error": e.to_string(),
                        "attempt": attempt,
                        "max_retries": max_retries
                    })),
                );

                // Check if we should retry
                if attempt >= max_retries {
                    error!(
                        "[{}] All {} installation attempts failed",
                        runtime, max_retries
                    );
                    return Err(error);
                }
            }
        }

        // Wait before retrying (exponential backoff like screenpipe)
        let delay_secs = 2u64.pow(attempt as u32);
        error!(
            "[{}] Installation attempt {} failed, retrying in {} seconds...",
            runtime, attempt, delay_secs
        );
        tokio::time::sleep(tokio::time::Duration::from_secs(delay_secs)).await;
    }
}

/// Execute JavaScript using Node.js/Bun runtime with terminator.js bindings available
pub async fn execute_javascript_with_nodejs(script: String) -> Result<serde_json::Value, McpError> {
    // Dev override: allow forcing local bindings via env var
    if std::env::var("TERMINATOR_JS_USE_LOCAL")
        .map(|v| {
            matches!(
                v.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "local"
            )
        })
        .unwrap_or(false)
    {
        info!("[Node.js] Using local bindings due to TERMINATOR_JS_USE_LOCAL env var");
        return execute_javascript_with_local_bindings(script).await;
    }

    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    info!("[Node.js] Starting JavaScript execution with terminator.js bindings");
    info!("[Node.js] Script to execute ({} bytes)", script.len());
    debug!(
        preview = %script.chars().take(200).collect::<String>(),
        "[Node.js] Script preview"
    );

    // Check if bun is available, fallback to node
    let runtime = if let Some(bun_exe) = find_executable("bun") {
        match Command::new(&bun_exe).arg("--version").output().await {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                info!(
                    "[Node.js] Found bun at: {} (version: {})",
                    bun_exe,
                    version.trim()
                );
                "bun"
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                info!(
                    "[Node.js] Bun found but version check failed: {}, falling back to node",
                    stderr
                );
                "node"
            }
            Err(e) => {
                info!(
                    "[Node.js] Bun found but not working ({}), falling back to node",
                    e
                );
                "node"
            }
        }
    } else {
        info!("[Node.js] Bun not found, using node");
        "node"
    };

    info!("[Node.js] Using runtime: {}", runtime);

    // Ensure terminator.js is installed and get the script directory
    let script_dir = ensure_terminator_js_installed(runtime).await?;
    info!("[Node.js] Script directory: {}", script_dir.display());

    // Log which terminator.js version is in use
    log_terminator_js_version(&script_dir, "Node.js").await;

    // Create a wrapper script that:
    // 1. Imports terminator.js
    // 2. Executes user script
    // 3. Returns result

    let wrapper_script = format!(
        r#"
const {{ Desktop }} = require('terminator.js');

// Create global objects
global.desktop = new Desktop();
global.log = console.log;
global.sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));

console.log('[Node.js Wrapper] Starting user script execution...');

// Execute user script
(async () => {{
    try {{
        console.log('[Node.js Wrapper] Executing user script...');
        const result = await (async () => {{
            {script}
        }})();
        
        console.log('[Node.js Wrapper] User script completed, result:', typeof result);
        
        // Send result back, handling undefined properly
        const resultToSend = result === undefined ? null : result;
        process.stdout.write('__RESULT__' + JSON.stringify(resultToSend) + '__END__\n');
        console.log('[Node.js Wrapper] Result sent back to parent process');
    }} catch (error) {{
        console.error('[Node.js Wrapper] User script error:', error && error.message);
        console.error('[Node.js Wrapper] Stack trace:', error && error.stack);
        // Emit machine-readable marker on stdout as well for parent capture
        console.log('__ERROR__' + JSON.stringify({{ message: String((error && error.message) || error), stack: String((error && error.stack) || '') }}) + '__END__');
        process.stdout.write('__ERROR__' + JSON.stringify({{
            message: String((error && error.message) || error),
            stack: String((error && error.stack) || '')
        }}) + '__END__\n');
    }}
}})();
"#
    );

    // Write script to the same directory where terminator.js is installed
    // Use unique filename to avoid race conditions between concurrent tests
    let unique_filename = format!(
        "main_{}.js",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let script_path = script_dir.join(&unique_filename);

    info!(
        "[Node.js] Writing wrapper script to: {}",
        script_path.display()
    );

    tokio::fs::write(&script_path, wrapper_script)
        .await
        .map_err(|e| {
            error!("[Node.js] Failed to write script file: {}", e);
            McpError::internal_error(
                "Failed to write script file",
                Some(json!({"error": e.to_string(), "path": script_path.to_string_lossy()})),
            )
        })?;

    info!("[Node.js] Script written successfully");

    // Find the runtime executable for spawning
    let runtime_exe = find_executable(runtime).ok_or_else(|| {
        error!(
            "[Node.js] Could not find {} executable for spawning",
            runtime
        );
        McpError::internal_error(
            format!("Could not find {runtime} executable for spawning"),
            None,
        )
    })?;

    info!("[Node.js] Spawning {} at: {}", runtime, runtime_exe);

    // Check if executable is a batch file on Windows
    let is_batch_file =
        cfg!(windows) && (runtime_exe.ends_with(".cmd") || runtime_exe.ends_with(".bat"));

    // Spawn Node.js/Bun process from the script directory
    let mut child = if runtime == "bun" && !is_batch_file {
        info!("[Node.js] Using direct bun execution");
        // Bun can be executed directly if it's not a batch file
        Command::new(&runtime_exe)
            .current_dir(&script_dir)
            .arg(&unique_filename)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    } else if cfg!(windows) && is_batch_file {
        info!("[Node.js] Using cmd.exe for batch file execution on Windows");
        // Use cmd.exe for batch files on Windows
        Command::new("cmd")
            .current_dir(&script_dir)
            .args(["/c", &runtime_exe, &unique_filename])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    } else if cfg!(windows) && runtime_exe.ends_with(".exe") {
        info!("[Node.js] Using direct .exe execution on Windows");
        // Direct execution should work for .exe files
        Command::new(&runtime_exe)
            .current_dir(&script_dir)
            .arg(&unique_filename)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    } else {
        info!("[Node.js] Using direct execution");
        Command::new(&runtime_exe)
            .current_dir(&script_dir)
            .arg(&unique_filename)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }
    .map_err(|e| {
        error!("[Node.js] Failed to spawn {} process: {}", runtime, e);
        McpError::internal_error(
            format!("Failed to spawn {runtime} process"),
            Some(json!({"error": e.to_string(), "runtime_exe": runtime_exe})),
        )
    })?;

    info!("[Node.js] Process spawned successfully, reading output...");

    let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut stderr = BufReader::new(child.stderr.take().unwrap()).lines();
    let mut result = None;
    let mut stderr_output = Vec::new();

    // Handle communication with Node.js process
    loop {
        tokio::select! {
            stdout_line = stdout.next_line() => {
                match stdout_line {
                    Ok(Some(line)) => {
                        info!("[Node.js stdout] {}", line);
                        if line.starts_with("__RESULT__") && line.ends_with("__END__") {
                            // Parse final result
                            let result_json = line.replace("__RESULT__", "").replace("__END__", "");
                            info!("[Node.js] Received result, parsing JSON ({} bytes)...", result_json.len());
                            info!("[Node.js] Result JSON: {}", result_json);

                            match serde_json::from_str(&result_json) {
                                Ok(parsed_result) => {
                                    info!("[Node.js] Successfully parsed result");
                                    result = Some(parsed_result);
                                    break;
                                }
                                Err(e) => {
                                    error!("[Node.js] Failed to parse result JSON: {}", e);
                                    info!("[Node.js] Invalid JSON was: {}", result_json);
                                }
                            }
                        } else if line.starts_with("__ERROR__") && line.ends_with("__END__") {
                            // Parse error
                            let error_json = line.replace("__ERROR__", "").replace("__END__", "");
                            error!("[Node.js] Received error from script: {}", error_json);

                            if let Ok(error_data) = serde_json::from_str::<serde_json::Value>(&error_json) {
                                return Err(McpError::internal_error(
                                    "JavaScript execution error",
                                    Some(error_data),
                                ));
                            }
                            break;
                        }
                    }
                    Ok(None) => {
                        info!("[Node.js] stdout stream ended");
                        break;
                    }
                    Err(e) => {
                        error!("[Node.js] Error reading stdout: {}", e);
                        break;
                    }
                }
            }
            stderr_line = stderr.next_line() => {
                match stderr_line {
                    Ok(Some(line)) => {
                        error!("[Node.js stderr] {}", line);
                        stderr_output.push(line);
                    }
                    Ok(None) => {
                        info!("[Node.js] stderr stream ended");
                    }
                    Err(e) => {
                        error!("[Node.js] Error reading stderr: {}", e);
                    }
                }
            }
        }
    }

    // Wait for process to complete
    info!("[Node.js] Waiting for process to complete...");
    let status = child.wait().await.map_err(|e| {
        error!("[Node.js] Process wait failed: {}", e);
        McpError::internal_error(
            "Node.js process failed",
            Some(json!({"error": e.to_string()})),
        )
    })?;

    info!("[Node.js] Process completed with status: {:?}", status);

    // Clean up the temporary script file (but keep the directory for reuse)
    if let Err(e) = tokio::fs::remove_file(&script_path).await {
        warn!(
            "[Node.js] Failed to clean up script file {}: {}",
            script_path.display(),
            e
        );
    }

    // Don't clean up script directory - keep it persistent for reuse
    info!(
        "[Node.js] Keeping persistent script directory for reuse: {}",
        script_dir.display()
    );

    if !status.success() {
        let exit_code = status.code();
        let stderr_combined = stderr_output.join("\n");

        error!("[Node.js] Process exited with error code: {:?}", exit_code);
        error!("[Node.js] Combined stderr output:\n{}", stderr_combined);

        return Err(McpError::internal_error(
            "Node.js process exited with error",
            Some(json!({
                "exit_code": exit_code,
                "stderr": stderr_combined,
                "script_path": script_path.to_string_lossy(),
                "working_directory": script_dir.to_string_lossy(),
                "runtime_exe": runtime_exe
            })),
        ));
    }

    match result {
        Some(r) => {
            info!("[Node.js] Execution completed successfully");
            Ok(r)
        }
        None => {
            error!("[Node.js] No result received from process");
            let stderr_combined = stderr_output.join("\n");
            Err(McpError::internal_error(
                "No result received from Node.js process",
                Some(json!({
                    "stderr": stderr_combined,
                    "script_path": script_path.to_string_lossy(),
                    "working_directory": script_dir.to_string_lossy()
                })),
            ))
        }
    }
}

/// Execute JavaScript using Node.js runtime with LOCAL terminator.js bindings (for development/testing)
pub async fn execute_javascript_with_local_bindings(
    script: String,
) -> Result<serde_json::Value, McpError> {
    use std::process::Stdio;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    info!("[Node.js Local] Starting JavaScript execution with local terminator.js bindings");

    // Check if bun is available, fallback to node
    let runtime = if let Some(bun_exe) = find_executable("bun") {
        match Command::new(&bun_exe).arg("--version").output().await {
            Ok(output) if output.status.success() => {
                info!("[Node.js Local] Found bun at: {}", bun_exe);
                "bun"
            }
            _ => {
                info!("[Node.js Local] Bun found but not working, falling back to node");
                "node"
            }
        }
    } else {
        info!("[Node.js Local] Bun not found, using node");
        "node"
    };

    info!("[Node.js Local] Using runtime: {}", runtime);

    // Resolve local bindings path robustly
    // 1) Explicit override via env var TERMINATOR_JS_LOCAL_BINDINGS (points directly to bindings/nodejs)
    // 2) Derive from compile-time crate dir (../bindings/nodejs)
    // 3) Try CWD/bindings/nodejs
    // 4) Try parent_of_CWD/bindings/nodejs
    // 5) Walk up a few ancestors looking for bindings/nodejs
    let local_bindings_path: PathBuf = {
        if let Ok(override_path) = std::env::var("TERMINATOR_JS_LOCAL_BINDINGS") {
            let p = PathBuf::from(override_path);
            info!(
                "[Node.js Local] Using TERMINATOR_JS_LOCAL_BINDINGS override: {}",
                p.display()
            );
            p
        } else {
            // Candidates to probe
            let mut candidates: Vec<PathBuf> = Vec::new();

            // From compile-time crate dir: <workspace>/terminator-mcp-agent => <workspace>/bindings/nodejs
            let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            if let Some(ws) = crate_dir.parent() {
                candidates.push(ws.join("bindings").join("nodejs"));
            }

            // From current dir
            if let Ok(cwd) = std::env::current_dir() {
                candidates.push(cwd.join("bindings").join("nodejs"));
                if let Some(parent) = cwd.parent() {
                    candidates.push(parent.join("bindings").join("nodejs"));
                }

                // Walk up to 5 ancestors looking for bindings/nodejs
                let mut anc = Some(cwd.as_path());
                for _ in 0..5 {
                    if let Some(a) = anc {
                        candidates.push(a.join("bindings").join("nodejs"));
                        anc = a.parent();
                    }
                }
            }

            // Pick the first existing candidate
            match candidates
                .into_iter()
                .find(|p| std::fs::metadata(p).is_ok())
            {
                Some(found) => found,
                None => {
                    return Err(McpError::internal_error(
                        "Local bindings directory not found",
                        Some(json!({
                            "hint": "Set TERMINATOR_JS_LOCAL_BINDINGS to the path of bindings/nodejs or run from the repo",
                            "cwd": std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()),
                            "crate_dir": crate_dir.to_string_lossy().to_string(),
                        })),
                    ));
                }
            }
        }
    };

    // Verify the local bindings directory exists
    if tokio::fs::metadata(&local_bindings_path).await.is_err() {
        return Err(McpError::internal_error(
            "Local bindings directory not found",
            Some(json!({"expected_path": local_bindings_path.to_string_lossy()})),
        ));
    }

    info!(
        "[Node.js Local] Using local bindings at: {}",
        local_bindings_path.display()
    );

    // Skip building local bindings; expect the user to build manually
    info!("[Node.js Local] Skipping local bindings build (user-managed)");

    // Create isolated execution directory for this run
    let script_dir = std::env::temp_dir().join(format!(
        "terminator_mcp_local_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    tokio::fs::create_dir_all(&script_dir).await.map_err(|e| {
        McpError::internal_error(
            "Failed to create script directory",
            Some(json!({"error": e.to_string()})),
        )
    })?;

    info!(
        "[Node.js Local] Created script directory: {}",
        script_dir.display()
    );

    // No npm install; require local bindings directly by absolute path
    info!("[Node.js Local] Using require() with absolute local bindings path (no install)");

    // Create a wrapper script that:
    // 1. Imports local terminator.js
    // 2. Executes user script
    // 3. Returns result
    // Require the bindings index.js explicitly for maximum compatibility (Node/Bun)
    let bindings_entry_path = local_bindings_path.join("index.js");
    let bindings_abs_path = bindings_entry_path.to_string_lossy().replace('\\', "\\\\");
    let wrapper_script = format!(
        r#"
 const {{ Desktop }} = require("{bindings_abs_path}");

 // Create global objects
 global.desktop = new Desktop();
 global.log = console.log;
 global.sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));

 // Execute user script
 (async () => {{
     try {{
         const result = await (async () => {{
             {script}
         }})();
         
         // Send result back (normalize undefined to null)
         const resultToSend = result === undefined ? null : result;
         process.stdout.write('__RESULT__' + JSON.stringify(resultToSend) + '__END__\n');
     }} catch (error) {{
         process.stdout.write('__ERROR__' + JSON.stringify({{
             message: error.message,
             stack: error.stack
         }}) + '__END__\n');
     }}
 }})();
 "#
    );

    // Write script to the directory with local bindings
    // Use unique filename to avoid race conditions between concurrent tests
    let unique_filename = format!(
        "main_{}.js",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let script_path = script_dir.join(&unique_filename);
    tokio::fs::write(&script_path, wrapper_script)
        .await
        .map_err(|e| {
            McpError::internal_error(
                "Failed to write script file",
                Some(json!({"error": e.to_string()})),
            )
        })?;

    info!(
        "[Node.js Local] Script written to: {}",
        script_path.display()
    );

    // Find the runtime executable for spawning
    let runtime_exe = find_executable(runtime).ok_or_else(|| {
        McpError::internal_error(
            format!("Could not find {runtime} executable for spawning"),
            None,
        )
    })?;

    info!("[Node.js Local] Spawning {} at: {}", runtime, runtime_exe);

    // Check if executable is a batch file on Windows
    let is_batch_file =
        cfg!(windows) && (runtime_exe.ends_with(".cmd") || runtime_exe.ends_with(".bat"));

    // Spawn Node.js/Bun process from the script directory
    let mut child = if runtime == "bun" && !is_batch_file {
        // Bun can be executed directly if it's not a batch file
        Command::new(&runtime_exe)
            .current_dir(&script_dir)
            .arg(&unique_filename)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    } else if cfg!(windows) && is_batch_file {
        // Use cmd.exe for batch files on Windows
        Command::new("cmd")
            .current_dir(&script_dir)
            .args(["/c", &runtime_exe, &unique_filename])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    } else if cfg!(windows) && runtime_exe.ends_with(".exe") {
        // Direct execution should work for .exe files
        Command::new(&runtime_exe)
            .current_dir(&script_dir)
            .arg(&unique_filename)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    } else {
        Command::new(&runtime_exe)
            .current_dir(&script_dir)
            .arg(&unique_filename)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }
    .map_err(|e| {
        McpError::internal_error(
            format!("Failed to spawn {runtime} process"),
            Some(json!({"error": e.to_string()})),
        )
    })?;

    let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut result = None;

    // Handle communication with Node.js process
    while let Ok(Some(line)) = stdout.next_line().await {
        if line.starts_with("__RESULT__") && line.ends_with("__END__") {
            // Parse final result
            let result_json = line.replace("__RESULT__", "").replace("__END__", "");
            result = serde_json::from_str(&result_json).ok();
            break;
        } else if line.starts_with("__ERROR__") && line.ends_with("__END__") {
            // Parse error
            let error_json = line.replace("__ERROR__", "").replace("__END__", "");
            if let Ok(error_data) = serde_json::from_str::<serde_json::Value>(&error_json) {
                return Err(McpError::internal_error(
                    "JavaScript execution error with local bindings",
                    Some(error_data),
                ));
            }
            break;
        } else {
            // Regular console output
            info!("[Node.js Local] {}", line);
        }
    }

    // Wait for process to complete
    let status = child.wait().await.map_err(|e| {
        McpError::internal_error(
            "Node.js process failed",
            Some(json!({"error": e.to_string()})),
        )
    })?;

    // Clean up only on success; keep files on error for debugging
    if status.success() {
        tokio::fs::remove_dir_all(&script_dir).await.ok();
        info!("[Node.js Local] Cleaned up script directory");
    } else {
        warn!(
            "[Node.js Local] Keeping script directory for debugging: {}",
            script_dir.display()
        );
    }

    if !status.success() {
        return Err(McpError::internal_error(
            "Node.js process exited with error",
            Some(json!({
                "exit_code": status.code(),
                "script_dir": script_dir.to_string_lossy(),
                "script_path": script_path.to_string_lossy()
            })),
        ));
    }

    result.ok_or_else(|| McpError::internal_error("No result received from Node.js process", None))
}
