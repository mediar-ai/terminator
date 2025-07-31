use std::fs;
use std::env;
use std::path::Path;
use std::process::Command;
use crate::command::run_command;

pub fn full_release(bump_type: &str) {
    println!("🚀 Starting full release process with {bump_type} bump...");
    bump_version(bump_type);
    tag_and_push();
}

pub fn ensure_project_root() {
    // Check if we're already in the project root
    if Path::new("Cargo.toml").exists() && Path::new("terminator").exists() {
        return;
    }

    // If we're in terminator-cli, go up one level
    if env::current_dir()
        .map(|p| {
            p.file_name()
                .map(|n| n == "terminator-cli")
                .unwrap_or(false)
        })
        .unwrap_or(false)
        && env::set_current_dir("..").is_err()
    {
        eprintln!("❌ Failed to change to project root directory");
        std::process::exit(1);
    }

    // Final check
    if !Path::new("Cargo.toml").exists() || !Path::new("terminator").exists() {
        eprintln!("❌ Not in Terminator project root. Please run from workspace root.");
        eprintln!("💡 Usage: terminator <command>");
        std::process::exit(1);
    }
}

pub fn get_workspace_version() -> Result<String, Box<dyn std::error::Error>> {
    let cargo_toml = fs::read_to_string("Cargo.toml")?;
    let mut in_workspace_package = false;

    for line in cargo_toml.lines() {
        let trimmed_line = line.trim();
        if trimmed_line == "[workspace.package]" {
            in_workspace_package = true;
            continue;
        }

        if in_workspace_package {
            if trimmed_line.starts_with('[') {
                // We've left the workspace.package section
                break;
            }
            if trimmed_line.starts_with("version") {
                if let Some(version_part) = trimmed_line.split('=').nth(1) {
                    if let Some(version) = version_part.trim().split('"').nth(1) {
                        return Ok(version.to_string());
                    }
                }
            }
        }
    }

    Err("Version not found in [workspace.package] in Cargo.toml".into())
}

pub fn sync_cargo_versions() -> Result<(), Box<dyn std::error::Error>> {
    println!("📦 Syncing Cargo.toml dependency versions...");
    let workspace_version = get_workspace_version()?;

    let cargo_toml = fs::read_to_string("Cargo.toml")?;
    let mut lines: Vec<String> = cargo_toml.lines().map(|s| s.to_string()).collect();
    let mut in_workspace_deps = false;
    let mut deps_version_updated = false;

    let tmp = 0..lines.len();
    for i in tmp {
        let line = &lines[i];
        let trimmed_line = line.trim();

        if trimmed_line.starts_with('[') {
            in_workspace_deps = trimmed_line == "[workspace.dependencies]";
            continue;
        }

        if in_workspace_deps && trimmed_line.starts_with("terminator =") {
            let line_clone = line.clone();
            if let Some(start) = line_clone.find("version = \"") {
                let version_start = start + "version = \"".len();
                if let Some(end_quote_offset) = line_clone[version_start..].find('"') {
                    let range = version_start..(version_start + end_quote_offset);
                    if &line_clone[range.clone()] != workspace_version.as_str() {
                        lines[i].replace_range(range, &workspace_version);
                        println!(
                            "✅ Updated 'terminator' dependency version to {workspace_version}."
                        );
                        deps_version_updated = true;
                    } else {
                        println!("✅ 'terminator' dependency version is already up to date.");
                        deps_version_updated = true; // Mark as done
                    }
                }
            }
            break; // Assume only one terminator dependency to update
        }
    }

    if deps_version_updated {
        fs::write("Cargo.toml", lines.join("\n") + "\n")?;
    } else {
        eprintln!(
            "⚠️  Warning: Could not find 'terminator' in [workspace.dependencies] to sync version."
        );
    }
    Ok(())
}

pub fn set_workspace_version(new_version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cargo_toml = fs::read_to_string("Cargo.toml")?;
    let mut lines: Vec<String> = cargo_toml.lines().map(|s| s.to_string()).collect();
    let mut in_workspace_package = false;
    let mut package_version_updated = false;

    let tmp = 0..lines.len();
    for i in tmp {
        let line = &lines[i];
        let trimmed_line = line.trim();

        if trimmed_line.starts_with('[') {
            in_workspace_package = trimmed_line == "[workspace.package]";
            continue;
        }

        if in_workspace_package && trimmed_line.starts_with("version =") {
            let indentation = line.len() - line.trim_start().len();
            lines[i] = format!("{}version = \"{}\"", " ".repeat(indentation), new_version);
            package_version_updated = true;
            break; // Exit after finding and updating the version
        }
    }

    if !package_version_updated {
        return Err("version key not found in [workspace.package] in Cargo.toml".into());
    }

    fs::write("Cargo.toml", lines.join("\n") + "\n")?;
    Ok(())
}

pub fn parse_version(version: &str) -> Result<(u32, u32, u32), Box<dyn std::error::Error>> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return Err("Invalid version format".into());
    }

    let major = parts[0].parse::<u32>()?;
    let minor = parts[1].parse::<u32>()?;
    let patch = parts[2].parse::<u32>()?;

    Ok((major, minor, patch))
}

pub fn bump_version(bump_type: &str) {
    println!("🔄 Bumping {bump_type} version...");

    let current_version = match get_workspace_version() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ Failed to get current version: {e}");
            return;
        }
    };

    let (major, minor, patch) = match parse_version(&current_version) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ Failed to parse version {current_version}: {e}");
            return;
        }
    };

    let new_version = match bump_type {
        "patch" => format!("{}.{}.{}", major, minor, patch + 1),
        "minor" => format!("{}.{}.0", major, minor + 1),
        "major" => format!("{}.0.0", major + 1),
        _ => {
            eprintln!("❌ Invalid bump type: {bump_type}");
            return;
        }
    };

    println!("📝 {current_version} → {new_version}");

    if let Err(e) = set_workspace_version(&new_version) {
        eprintln!("❌ Failed to update workspace version: {e}");
        return;
    }

    println!("✅ Updated workspace version to {new_version}");
    sync_all_versions();
}

pub fn sync_all_versions() {
    println!("🔄 Syncing all package versions...");

    // First, sync versions within Cargo.toml
    if let Err(e) = sync_cargo_versions() {
        eprintln!("❌ Failed to sync versions in Cargo.toml: {e}");
        return;
    }

    let workspace_version = match get_workspace_version() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ Failed to get workspace version: {e}");
            return;
        }
    };

    println!("📦 Workspace version: {workspace_version}");

    // Sync Node.js bindings
    sync_nodejs_bindings(&workspace_version);

    // Sync MCP agent
    sync_mcp_agent(&workspace_version);

    // Update Cargo.lock
    println!("🔒 Updating Cargo.lock...");
    if let Err(e) = run_command("cargo", &["check", "--quiet"]) {
        eprintln!("⚠️  Warning: Failed to update Cargo.lock: {e}");
    }

    println!("✅ All versions synchronized!");
}

pub fn sync_nodejs_bindings(version: &str) {
    println!("📦 Syncing Node.js bindings to version {version}...");

    let nodejs_dir = Path::new("bindings/nodejs");
    if !nodejs_dir.exists() {
        println!("⚠️  Node.js bindings directory not found, skipping");
        return;
    }

    // Update main package.json directly
    if let Err(e) = update_package_json("bindings/nodejs/package.json", version) {
        eprintln!("⚠️  Warning: Failed to update Node.js package.json directly: {e}");
    } else {
        println!("✅ Updated Node.js package.json to {version}");
    }

    // ALSO update CPU/platform-specific packages under bindings/nodejs/npm
    let npm_dir = nodejs_dir.join("npm");
    if npm_dir.exists() {
        if let Ok(entries) = fs::read_dir(&npm_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let package_json = entry.path().join("package.json");
                    if package_json.exists() {
                        if let Err(e) =
                            update_package_json(&package_json.to_string_lossy(), version)
                        {
                            eprintln!(
                                "⚠️  Warning: Failed to update {}: {}",
                                package_json.display(),
                                e
                            );
                        } else {
                            println!("📦 Updated {}", entry.file_name().to_string_lossy());
                        }
                    }
                }
            }
        }
    }

    // Run sync script if it exists (still useful for additional tasks like N-API metadata)
    let original_dir = match env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("❌ Could not get current directory: {e}");
            return;
        }
    };

    if env::set_current_dir(nodejs_dir).is_ok() {
        println!("🔄 Running npm run sync-version...");
        if run_command("npm", &["run", "sync-version"]).is_ok() {
            println!("✅ Node.js sync script completed");
        } else {
            eprintln!("⚠️  Warning: npm run sync-version failed");
        }
        // Always change back to the original directory
        if let Err(e) = env::set_current_dir(&original_dir) {
            eprintln!("❌ Failed to restore original directory: {e}");
            std::process::exit(1); // Exit if we can't get back, to avoid further errors
        }
    } else {
        eprintln!("⚠️  Warning: Could not switch to Node.js directory");
    }
}

pub fn sync_mcp_agent(version: &str) {
    println!("📦 Syncing MCP agent...");

    let mcp_dir = Path::new("terminator-mcp-agent");
    if !mcp_dir.exists() {
        return;
    }

    // Update main package.json
    if let Err(e) = update_package_json("terminator-mcp-agent/package.json", version) {
        eprintln!("⚠️  Warning: Failed to update MCP agent package.json: {e}");
        return;
    }

    // Update platform packages
    let npm_dir = mcp_dir.join("npm");
    if npm_dir.exists() {
        if let Ok(entries) = fs::read_dir(npm_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let package_json = entry.path().join("package.json");
                    if package_json.exists() {
                        if let Err(e) =
                            update_package_json(&package_json.to_string_lossy(), version)
                        {
                            eprintln!(
                                "⚠️  Warning: Failed to update {}: {}",
                                entry.path().display(),
                                e
                            );
                        } else {
                            println!("📦 Updated {}", entry.file_name().to_string_lossy());
                        }
                    }
                }
            }
        }
    }

    // Update package-lock.json
    let original_dir = match env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("❌ Could not get current directory: {e}");
            return;
        }
    };

    if env::set_current_dir(mcp_dir).is_ok() {
        if run_command("npm", &["install", "--package-lock-only", "--silent"]).is_ok() {
            println!("✅ MCP package-lock.json updated.");
        } else {
            eprintln!("⚠️  Warning: Failed to update MCP agent package-lock.json");
        }
        // Always change back to the original directory
        if let Err(e) = env::set_current_dir(&original_dir) {
            eprintln!("❌ Failed to restore original directory: {e}");
            std::process::exit(1);
        }
    }

    println!("✅ MCP agent synced");
}

pub fn update_package_json(path: &str, version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut pkg: serde_json::Value = serde_json::from_str(&content)?;

    // Update main version
    pkg["version"] = serde_json::Value::String(version.to_string());

    // Update optional dependencies that start with terminator-mcp- or terminator.js-
    if let Some(deps) = pkg
        .get_mut("optionalDependencies")
        .and_then(|v| v.as_object_mut())
    {
        for (key, value) in deps.iter_mut() {
            if key.starts_with("terminator-mcp-") || key.starts_with("terminator.js-") {
                *value = serde_json::Value::String(version.to_string());
            }
        }
    }

    // Write back with pretty formatting
    let formatted = serde_json::to_string_pretty(&pkg)?;
    fs::write(path, formatted + "\n")?;

    Ok(())
}

pub fn show_status() {
    println!("📊 Terminator Project Status");
    println!("============================");

    let workspace_version = get_workspace_version().unwrap_or_else(|_| "ERROR".to_string());
    println!("📦 Workspace version: {workspace_version}");

    // Show package versions
    let nodejs_version = get_package_version("bindings/nodejs/package.json");
    let mcp_version = get_package_version("terminator-mcp-agent/package.json");

    println!();
    println!("Package versions:");
    println!("  Node.js bindings: {nodejs_version}");
    println!("  MCP agent:        {mcp_version}");

    // Git status
    println!();
    println!("Git status:");
    if let Ok(output) = Command::new("git").args(["status", "--porcelain"]).output() {
        let status = String::from_utf8_lossy(&output.stdout);
        if status.trim().is_empty() {
            println!("  ✅ Working directory clean");
        } else {
            println!("  ⚠️  Uncommitted changes:");
            for line in status.lines().take(5) {
                println!("     {line}");
            }
        }
    }
}

pub fn get_package_version(path: &str) -> String {
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(pkg) => pkg
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "No version field".to_string()),
            Err(_) => "Parse error".to_string(),
        },
        Err(_) => "Not found".to_string(),
    }
}

pub fn tag_and_push() {
    let version = match get_workspace_version() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ Failed to get current version: {e}");
            return;
        }
    };

    println!("🏷️  Tagging and pushing version {version}...");

    // Check for uncommitted changes
    if let Ok(output) = Command::new("git").args(["diff", "--name-only"]).output() {
        let diff = String::from_utf8_lossy(&output.stdout);
        if !diff.trim().is_empty() {
            println!("⚠️  Uncommitted changes detected. Committing...");
            if let Err(e) = run_command("git", &["add", "."]) {
                eprintln!("❌ Failed to git add: {e}");
                return;
            }
            if let Err(e) = run_command(
                "git",
                &["commit", "-m", &format!("Bump version to {version}")],
            ) {
                eprintln!("❌ Failed to git commit: {e}");
                return;
            }
        }
    }

    // Create tag
    let tag = format!("v{version}");
    if let Err(e) = run_command(
        "git",
        &[
            "tag",
            "-a",
            &tag,
            "-m",
            &format!("Release version {version}"),
        ],
    ) {
        eprintln!("❌ Failed to create tag: {e}");
        return;
    }

    // Push changes and tag
    if let Err(e) = run_command("git", &["push", "origin", "main"]) {
        eprintln!("❌ Failed to push changes: {e}");
        return;
    }

    if let Err(e) = run_command("git", &["push", "origin", &tag]) {
        eprintln!("❌ Failed to push tag: {e}");
        return;
    }

    println!("✅ Successfully released version {version}!");
    println!("🔗 Check CI: https://github.com/mediar-ai/terminator/actions");
}
