//! Child Process Registry (Windows-only)
//!
//! Tracks spawned child processes (bun/node workflow executors) and provides
//! cleanup functionality to kill them on MCP shutdown.
//!
//! This prevents dangling processes when:
//! - MCP server shuts down gracefully (Ctrl+C)
//! - MCP server crashes
//! - Parent process dies unexpectedly
//!
//! Note: This module only supports Windows. On other platforms, the functions
//! are no-ops.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use tracing::{debug, info, warn};

/// Information about a tracked child process
#[derive(Debug, Clone)]
pub struct ChildProcessInfo {
    pub pid: u32,
    pub execution_id: Option<String>,
    pub started_at: std::time::Instant,
}

/// Global registry of active child processes
static CHILD_PROCESSES: OnceLock<RwLock<HashMap<u32, ChildProcessInfo>>> = OnceLock::new();

/// Get the child process registry, initializing if needed
fn get_registry() -> &'static RwLock<HashMap<u32, ChildProcessInfo>> {
    CHILD_PROCESSES.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Register a child process for tracking
pub fn register(pid: u32, execution_id: Option<String>) {
    let info = ChildProcessInfo {
        pid,
        execution_id: execution_id.clone(),
        started_at: std::time::Instant::now(),
    };

    if let Ok(mut registry) = get_registry().write() {
        debug!(
            "Registering child process PID {} (execution_id: {:?})",
            pid, execution_id
        );
        registry.insert(pid, info);
    }
}

/// Unregister a child process (called when it exits naturally)
pub fn unregister(pid: u32) {
    if let Ok(mut registry) = get_registry().write() {
        if registry.remove(&pid).is_some() {
            debug!("Unregistered child process PID {}", pid);
        }
    }
}

/// Get the count of active child processes
pub fn active_count() -> usize {
    get_registry().read().map(|r| r.len()).unwrap_or(0)
}

/// Kill all tracked child processes
///
/// Called during MCP shutdown to ensure no dangling processes remain.
/// Uses platform-specific methods to terminate processes.
pub fn kill_all() {
    let pids_to_kill: Vec<ChildProcessInfo> = {
        match get_registry().write() {
            Ok(mut registry) => registry.drain().map(|(_, info)| info).collect(),
            Err(e) => {
                warn!("Failed to acquire child process registry lock: {}", e);
                return;
            }
        }
    };

    if pids_to_kill.is_empty() {
        debug!("No child processes to clean up");
        return;
    }

    info!(
        "Cleaning up {} child process(es) on shutdown",
        pids_to_kill.len()
    );

    for info in pids_to_kill {
        kill_process(info.pid, info.execution_id.as_deref());
    }
}

/// Kill a specific process by PID (Windows-only)
#[cfg(target_os = "windows")]
fn kill_process(pid: u32, execution_id: Option<&str>) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    unsafe {
        match OpenProcess(PROCESS_TERMINATE, false, pid) {
            Ok(handle) => {
                if !handle.is_invalid() {
                    match TerminateProcess(handle, 1) {
                        Ok(_) => {
                            info!(
                                "Terminated child process PID {} (execution_id: {:?})",
                                pid, execution_id
                            );
                        }
                        Err(e) => {
                            warn!("Failed to terminate process PID {}: {:?}", pid, e);
                        }
                    }
                    let _ = CloseHandle(handle);
                }
            }
            Err(e) => {
                // Process may have already exited
                debug!(
                    "Could not open process PID {} for termination (may have already exited): {:?}",
                    pid, e
                );
            }
        }
    }
}

/// No-op on non-Windows platforms
#[cfg(not(target_os = "windows"))]
fn kill_process(_pid: u32, _execution_id: Option<&str>) {
    // No-op on non-Windows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_unregister() {
        // Clear any existing state
        if let Ok(mut registry) = get_registry().write() {
            registry.clear();
        }

        register(12345, Some("test-exec-1".to_string()));
        assert_eq!(active_count(), 1);

        register(12346, None);
        assert_eq!(active_count(), 2);

        unregister(12345);
        assert_eq!(active_count(), 1);

        unregister(12346);
        assert_eq!(active_count(), 0);
    }

    #[test]
    fn test_unregister_nonexistent() {
        // Should not panic
        unregister(99999);
    }

    #[test]
    fn test_kill_all_empty() {
        // Clear any existing state
        if let Ok(mut registry) = get_registry().write() {
            registry.clear();
        }

        // Should not panic with empty registry
        kill_all();
        assert_eq!(active_count(), 0);
    }
}
