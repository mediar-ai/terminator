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
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}
