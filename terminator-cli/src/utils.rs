use tokio::process::Command;

/// Check if the path is a Windows batch file
pub fn is_batch_file(path: &str) -> bool {
    path.ends_with(".bat") || path.ends_with(".cmd")
}

/// Create command with proper handling for batch files on Windows
pub fn create_command(executable: &str, args: &[String]) -> Command {
    let mut cmd = if cfg!(windows) && is_batch_file(executable) {
        // For batch files on Windows, use cmd.exe /c
        let mut cmd = Command::new("cmd");
        cmd.arg("/c");
        cmd.arg(executable);
        cmd
    } else {
        Command::new(executable)
    };

    if !args.is_empty() {
        cmd.args(args);
    }

    cmd
}

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
                    return Some(full_path.to_string_lossy().to_string());
                }
            }
        }
    }

    // Fallback: try the name as-is (might work on some systems)
    Some(name.to_string())
}

pub fn init_logging() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let _ = tracing_subscriber::registry()
        .with(
            // Respect RUST_LOG if provided, else default to info
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

// Helper function to parse step start logs
#[allow(dead_code)]
fn parse_step_log(line: &str) -> Option<(String, String, String)> {
    // Parse lines like: "Step 0 BEGIN tool='open_application' id='open_notepad' ..."
    if let Some(step_idx) = line.find("Step ") {
        let after_step = &line[step_idx + 5..];
        if let Some(space_idx) = after_step.find(' ') {
            let step_num = &after_step[..space_idx];
            if let Some(tool_idx) = line.find("tool='") {
                let after_tool = &line[tool_idx + 6..];
                if let Some(quote_idx) = after_tool.find('\'') {
                    let tool_name = &after_tool[..quote_idx];
                    return Some((
                        step_num.to_string(),
                        "?".to_string(), // We don't have total from logs
                        tool_name.to_string(),
                    ));
                }
            } else if let Some(group_idx) = line.find("group='") {
                let after_group = &line[group_idx + 7..];
                if let Some(quote_idx) = after_group.find('\'') {
                    let group_name = &after_group[..quote_idx];
                    return Some((
                        step_num.to_string(),
                        "?".to_string(),
                        format!("[{group_name}]"),
                    ));
                }
            }
        }
    }
    None
}

// Helper function to parse step end logs
#[allow(dead_code)]
fn parse_step_end_log(line: &str) -> Option<(String, String)> {
    // Parse lines like: "Step 0 END tool='open_application' id='open_notepad' status=success"
    if let Some(step_idx) = line.find("Step ") {
        let after_step = &line[step_idx + 5..];
        if let Some(space_idx) = after_step.find(' ') {
            let step_num = &after_step[..space_idx];
            if let Some(status_idx) = line.find("status=") {
                let after_status = &line[status_idx + 7..];
                let status = after_status.split_whitespace().next().unwrap_or("unknown");
                return Some((step_num.to_string(), status.to_string()));
            }
        }
    }
    None
}

