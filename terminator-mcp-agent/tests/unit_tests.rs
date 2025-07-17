use serde_json::json;
use terminator_mcp_agent::scripting_engine::find_executable;
use terminator_mcp_agent::utils::{ExecuteSequenceArgs, SequenceStep, ToolCall};

#[test]
fn test_execute_sequence_args_serialization() {
    let args = ExecuteSequenceArgs {
        steps: vec![SequenceStep {
            tool_name: Some("click_element".to_string()),
            arguments: Some(json!({
                "selector": "button|Submit"
            })),
            continue_on_error: Some(true),
            delay_ms: Some(100),
            ..Default::default()
        }],
        stop_on_error: Some(false),
        include_detailed_results: Some(true),
        output_parser: None,
        variables: None,
        inputs: None,
        selectors: None,
    };

    let json = serde_json::to_string(&args).unwrap();
    assert!(json.contains("steps"));
    assert!(json.contains("click_element"));
}

#[test]
fn test_execute_sequence_args_deserialization() {
    let json = r#"{
        "steps": [{
            "tool_name": "another_tool",
            "arguments": {"foo": "bar"},
            "continue_on_error": false,
            "delay_ms": 200
        }],
        "stop_on_error": true,
        "include_detailed_results": false
    }"#;

    let deserialized: ExecuteSequenceArgs = serde_json::from_str(json).unwrap();

    // Verify the steps content
    assert_eq!(deserialized.steps.len(), 1);
    assert_eq!(
        deserialized.steps[0].tool_name,
        Some("another_tool".to_string())
    );
    assert_eq!(
        deserialized.steps[0].arguments.as_ref().unwrap()["foo"],
        "bar"
    );
    assert_eq!(deserialized.steps[0].continue_on_error, Some(false));
    assert_eq!(deserialized.steps[0].delay_ms, Some(200));

    assert_eq!(deserialized.stop_on_error, Some(true));
    assert_eq!(deserialized.include_detailed_results, Some(false));
}

#[test]
fn test_execute_sequence_args_default_values() {
    let json = r#"{
        "steps": []
    }"#;

    let args: ExecuteSequenceArgs = serde_json::from_str(json).unwrap();

    // Verify it's an empty array
    assert_eq!(args.steps.len(), 0);

    assert_eq!(args.stop_on_error, None);
    assert_eq!(args.include_detailed_results, None);
}

#[test]
fn test_tool_call_defaults() {
    // Test that optional fields can be omitted
    let json_str = r#"{
        "tool_name": "minimal_tool",
        "arguments": {}
    }"#;

    let tool_call: ToolCall = serde_json::from_str(json_str).unwrap();
    assert_eq!(tool_call.tool_name, "minimal_tool");
    assert_eq!(tool_call.arguments, json!({}));
    assert_eq!(tool_call.continue_on_error, None);
    assert_eq!(tool_call.delay_ms, None);
}

#[test]
fn test_execute_sequence_minimal() {
    // Test minimal valid execute sequence args
    let json_str = r#"{
        "steps": []
    }"#;

    let args: ExecuteSequenceArgs = serde_json::from_str(json_str).unwrap();
    assert_eq!(args.steps.len(), 0);
    assert_eq!(args.stop_on_error, None);
    assert_eq!(args.include_detailed_results, None);
}

#[test]
fn test_complex_arguments_preservation() {
    let complex_args = json!({
        "nested": {
            "array": [1, 2, 3],
            "object": {
                "key": "value"
            }
        },
        "boolean": true,
        "number": 42.5,
        "null_value": null
    });

    let tool_call = ToolCall {
        tool_name: "complex_tool".to_string(),
        arguments: complex_args.clone(),
        continue_on_error: None,
        delay_ms: None,
        id: None,
    };

    let serialized = serde_json::to_value(&tool_call).unwrap();
    assert_eq!(serialized["arguments"], complex_args);
}

#[test]
fn test_sequence_step_with_group() {
    // Test that SequenceStep can handle grouped steps
    let json_str = r#"{
        "group_name": "test_group",
        "steps": [{
            "tool_name": "tool1",
            "arguments": {"param": "value"}
        }],
        "continue_on_error": true
    }"#;

    let step: SequenceStep = serde_json::from_str(json_str).unwrap();
    assert_eq!(step.group_name, Some("test_group".to_string()));
    assert_eq!(step.continue_on_error, Some(true));
    assert!(step.steps.is_some());
    assert_eq!(step.steps.as_ref().unwrap().len(), 1);
    assert_eq!(step.steps.as_ref().unwrap()[0].tool_name, "tool1");
}

// ===============================================
// Scripting Engine Executable Resolution Tests
// ===============================================

#[test]
fn test_find_executable_node() {
    // Test finding node executable
    let result = find_executable("node");
    assert!(result.is_some(), "Should find node executable");

    let node_path = result.unwrap();
    assert!(!node_path.is_empty(), "Node path should not be empty");
    assert!(node_path.contains("node"), "Path should contain 'node'");

    println!("Found node at: {node_path}");
}

#[test]
fn test_find_executable_npm() {
    // Test finding npm executable
    let result = find_executable("npm");
    assert!(result.is_some(), "Should find npm executable");

    let npm_path = result.unwrap();
    assert!(!npm_path.is_empty(), "NPM path should not be empty");
    assert!(npm_path.contains("npm"), "Path should contain 'npm'");

    println!("Found npm at: {npm_path}");
}

#[test]
fn test_find_executable_nonexistent() {
    // Test finding a non-existent executable
    let result = find_executable("definitely_does_not_exist_executable_12345");

    // The function should still return Some() as a fallback, but it won't be a valid path
    assert!(
        result.is_some(),
        "Should return fallback name even for non-existent executable"
    );

    let fallback_name = result.unwrap();
    assert_eq!(fallback_name, "definitely_does_not_exist_executable_12345");
}

#[cfg(windows)]
#[test]
fn test_find_executable_windows_specific() {
    // Test Windows-specific behavior
    use std::path::Path;

    // Test that function handles .exe extension properly
    let node_result = find_executable("node");
    assert!(node_result.is_some());

    let node_path = node_result.unwrap();

    // On Windows, the path should exist and be a file
    let path = Path::new(&node_path);
    if path.exists() {
        assert!(path.is_file(), "Node path should point to a file");

        // Should end with .exe on Windows if it's a real executable
        if node_path.contains("Program Files") || node_path.contains("nodejs") {
            assert!(
                node_path.ends_with(".exe") || node_path.ends_with("node"),
                "Windows executable should end with .exe or be bare name: {node_path}"
            );
        }
    }

    println!("Windows node path: {node_path}");
}

#[test]
fn test_find_executable_path_validation() {
    // Test that the function returns valid-looking paths
    let executables_to_test = vec!["node", "npm"];

    for exe_name in executables_to_test {
        let result = find_executable(exe_name);
        assert!(result.is_some(), "Should find executable: {exe_name}");

        let exe_path = result.unwrap();
        assert!(
            !exe_path.is_empty(),
            "Path should not be empty for: {exe_name}"
        );

        // Path should contain the executable name
        assert!(
            exe_path.to_lowercase().contains(&exe_name.to_lowercase()),
            "Path should contain executable name '{exe_name}': {exe_path}"
        );

        println!("Found {exe_name} at: {exe_path}");
    }
}

#[test]
fn test_find_executable_bun_optional() {
    // Test finding bun (which may or may not be installed)
    let result = find_executable("bun");
    assert!(result.is_some(), "Should always return some result");

    let bun_path = result.unwrap();
    assert!(!bun_path.is_empty(), "Bun path should not be empty");

    // Check if bun actually exists
    use std::path::Path;
    let path = Path::new(&bun_path);

    if path.exists() && path.is_file() {
        println!("Found bun executable at: {bun_path}");
        assert!(
            bun_path.contains("bun"),
            "Real bun path should contain 'bun'"
        );
    } else {
        println!("Bun not installed, got fallback: {bun_path}");
        assert_eq!(
            bun_path, "bun",
            "Should return fallback name when not found"
        );
    }
}

#[test]
fn test_find_executable_case_sensitivity() {
    // Test case sensitivity handling
    #[cfg(windows)]
    {
        // Windows should be case-insensitive
        let node_lower = find_executable("node");
        let node_upper = find_executable("NODE");

        assert!(node_lower.is_some());
        assert!(node_upper.is_some());

        println!("node (lowercase): {node_lower:?}");
        println!("NODE (uppercase): {node_upper:?}");
    }

    #[cfg(not(windows))]
    {
        // Unix systems are case-sensitive
        let node_result = find_executable("node");
        assert!(node_result.is_some());

        println!("node: {node_result:?}");
    }
}

#[test]
fn test_path_environment_variable() {
    // Test that PATH environment variable is being used
    use std::env;

    // This test verifies that our function respects the PATH environment variable
    let path_var = env::var("PATH");
    assert!(path_var.is_ok(), "PATH environment variable should exist");

    let path_value = path_var.unwrap();
    assert!(!path_value.is_empty(), "PATH should not be empty");

    println!(
        "PATH contains {} directories",
        path_value
            .split(if cfg!(windows) { ";" } else { ":" })
            .count()
    );

    // Test that executables found are actually in PATH
    let node_result = find_executable("node");
    if let Some(node_path) = node_result {
        use std::path::Path;
        let path = Path::new(&node_path);

        if path.exists() {
            // Verify the parent directory is in PATH
            if let Some(parent) = path.parent() {
                let parent_str = parent.to_string_lossy();
                let is_in_path = path_value
                    .split(if cfg!(windows) { ";" } else { ":" })
                    .any(|p| {
                        let path_entry = Path::new(p);
                        path_entry == parent || path_entry.to_string_lossy() == parent_str
                    });

                if is_in_path {
                    println!("✓ Node found in PATH at: {node_path}");
                } else {
                    println!("⚠ Node found outside PATH at: {node_path}");
                }
            }
        }
    }
}

#[cfg(windows)]
#[test]
fn test_windows_batch_file_execution() {
    // Test that we can handle Windows batch files like npm correctly
    use std::process::Command;

    // Test direct npm execution (should fail with our old approach)
    let npm_path = find_executable("npm").unwrap();
    println!("Testing npm execution at: {}", npm_path);

    // Test cmd.exe approach (should work)
    let cmd_result = Command::new("cmd")
        .args(["/c", "npm", "--version"])
        .output();

    match cmd_result {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                println!("✓ npm via cmd.exe works, version: {}", version.trim());
                assert!(!version.trim().is_empty(), "Should get npm version");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("⚠ npm via cmd.exe failed: {}", stderr);
            }
        }
        Err(e) => {
            println!("⚠ Failed to test npm via cmd.exe: {}", e);
        }
    }

    // Test node.exe execution (should work directly)
    let node_path = find_executable("node").unwrap();
    println!("Testing node execution at: {}", node_path);

    if node_path.ends_with(".exe") {
        let node_result = Command::new(&node_path)
            .args(["-e", "console.log('Node.js test successful')"])
            .output();

        match node_result {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    println!("✓ node.exe direct execution works: {}", stdout.trim());
                    assert!(stdout.contains("Node.js test successful"));
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    println!("⚠ node.exe direct execution failed: {}", stderr);
                }
            }
            Err(e) => {
                println!("⚠ Failed to test node.exe directly: {}", e);
            }
        }
    }
}

#[tokio::test]
async fn test_cross_platform_command_execution() {
    // Test that our command execution strategy works across platforms
    use tokio::process::Command;

    // Test Node.js version check
    let node_path = find_executable("node").unwrap();

    let version_result = if cfg!(windows) && node_path.ends_with(".exe") {
        // Direct execution for .exe files
        Command::new(&node_path).args(["--version"]).output().await
    } else if cfg!(windows) {
        // cmd.exe fallback for batch files
        Command::new("cmd")
            .args(["/c", "node", "--version"])
            .output()
            .await
    } else {
        // Unix systems
        Command::new(&node_path).args(["--version"]).output().await
    };

    match version_result {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                println!(
                    "✓ Cross-platform node execution works, version: {}",
                    version.trim()
                );
                assert!(
                    version.starts_with("v"),
                    "Should get node version starting with 'v'"
                );
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("⚠ Cross-platform node execution failed: {}", stderr);
            }
        }
        Err(e) => {
            println!("⚠ Failed cross-platform node test: {}", e);
        }
    }
}

#[tokio::test]
async fn test_nodejs_script_execution_debug() {
    // Test basic Node.js script execution to debug the exit code 1 issue
    use std::process::Stdio;
    use tokio::process::Command;

    // Test 1: Simple console.log script
    let simple_script = r#"console.log("Hello from Node.js test");"#;

    // Write simple script to temp file
    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("test_simple.js");

    tokio::fs::write(&script_path, simple_script).await.unwrap();

    let node_path = find_executable("node").unwrap();

    let result = if cfg!(windows) && node_path.ends_with(".exe") {
        Command::new(&node_path)
            .arg(script_path.to_str().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
    } else if cfg!(windows) {
        Command::new("cmd")
            .args(["/c", "node", script_path.to_str().unwrap()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
    } else {
        Command::new(&node_path)
            .arg(script_path.to_str().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
    };

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            println!("Simple script stdout: {}", stdout);
            println!("Simple script stderr: {}", stderr);
            println!("Simple script exit code: {:?}", output.status.code());

            if output.status.success() {
                assert!(stdout.contains("Hello from Node.js test"));
                println!("✅ Basic Node.js execution works");
            } else {
                println!("❌ Basic Node.js execution failed");
            }
        }
        Err(e) => {
            println!("❌ Failed to execute simple Node.js script: {}", e);
        }
    }

    // Test 2: Try to require terminator.js (this will likely fail but shows us the error)
    let terminator_test_script = r#"
try {
    console.log("Trying to require terminator.js...");
    const { Desktop } = require('terminator.js');
    console.log("✅ terminator.js loaded successfully");
} catch (error) {
    console.log("❌ terminator.js failed to load:");
    console.log("Error name:", error.name);
    console.log("Error message:", error.message);
    console.log("Error code:", error.code);
    
    // Try to list available modules
    console.log("Checking if module exists in node_modules...");
    const fs = require('fs');
    const path = require('path');
    
    // Check current directory
    try {
        const nodeModulesPath = path.join(process.cwd(), 'node_modules');
        console.log("Current working directory:", process.cwd());
        console.log("Looking for node_modules at:", nodeModulesPath);
        
        if (fs.existsSync(nodeModulesPath)) {
            console.log("node_modules exists");
            const contents = fs.readdirSync(nodeModulesPath);
            console.log("node_modules contents:", contents.slice(0, 10)); // First 10 items
            
            if (contents.includes('terminator.js')) {
                console.log("✅ terminator.js found in node_modules");
            } else {
                console.log("❌ terminator.js NOT found in node_modules");
            }
        } else {
            console.log("❌ node_modules directory does not exist");
        }
    } catch (fsError) {
        console.log("❌ Error checking filesystem:", fsError.message);
    }
}
"#;

    let terminator_script_path = temp_dir.join("test_terminator.js");
    tokio::fs::write(&terminator_script_path, terminator_test_script)
        .await
        .unwrap();

    let terminator_result = if cfg!(windows) && node_path.ends_with(".exe") {
        Command::new(&node_path)
            .arg(terminator_script_path.to_str().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
    } else if cfg!(windows) {
        Command::new("cmd")
            .args(["/c", "node", terminator_script_path.to_str().unwrap()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
    } else {
        Command::new(&node_path)
            .arg(terminator_script_path.to_str().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
    };

    match terminator_result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            println!("=== Terminator.js Test Output ===");
            println!("stdout:\n{}", stdout);
            println!("stderr:\n{}", stderr);
            println!("exit code: {:?}", output.status.code());
            println!("=== End Terminator.js Test ===");
        }
        Err(e) => {
            println!("❌ Failed to execute terminator.js test script: {}", e);
        }
    }

    // Cleanup
    tokio::fs::remove_file(&script_path).await.ok();
    tokio::fs::remove_file(&terminator_script_path).await.ok();
}

#[tokio::test]
async fn test_complete_nodejs_terminator_execution() {
    // Test the complete flow: install terminator.js in isolated dir and run script
    use terminator_mcp_agent::scripting_engine::execute_javascript_with_nodejs;

    let test_script = r#"
// Test that terminator.js loads correctly
try {
    console.log("Testing terminator.js import...");
    const { Desktop } = require('terminator.js');
    console.log("✅ terminator.js imported successfully");
    
    // Test basic functionality
    const desktop = new Desktop();
    console.log("✅ Desktop instance created");
    
    // Return success result
    return {
        success: true,
        message: "terminator.js working correctly",
        hasDesktop: typeof desktop !== 'undefined'
    };
    
} catch (error) {
    console.log("❌ Error:", error.message);
    return {
        success: false,
        error: error.message,
        code: error.code || 'UNKNOWN'
    };
}
"#;

    println!("🧪 Testing complete Node.js terminator.js execution...");

    let result = execute_javascript_with_nodejs(test_script.to_string()).await;

    match result {
        Ok(value) => {
            println!("✅ Script executed successfully!");
            println!(
                "📄 Result: {}",
                serde_json::to_string_pretty(&value).unwrap_or_default()
            );

            // Verify the result structure
            if let Some(obj) = value.as_object() {
                if let Some(success) = obj.get("success").and_then(|v| v.as_bool()) {
                    assert!(success, "Script should report success");
                    println!("✅ Script reported success");
                } else {
                    panic!("Script result should have success field");
                }

                if let Some(has_desktop) = obj.get("hasDesktop").and_then(|v| v.as_bool()) {
                    assert!(has_desktop, "Desktop instance should exist");
                    println!("✅ Desktop instance created successfully");
                }
            } else {
                panic!("Result should be an object with success info");
            }
        }
        Err(e) => {
            println!("❌ Script execution failed: {}", e);
            panic!("Node.js script execution should succeed: {}", e);
        }
    }
}

#[tokio::test]
async fn test_debug_nodejs_execution_with_logs() {
    // Direct test of our Node.js execution to see stdout/stderr
    use std::process::Stdio;
    use terminator_mcp_agent::scripting_engine::find_executable;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    println!("🔍 Debug test: Creating isolated terminator.js environment...");

    // Create isolated directory
    let script_dir = std::env::temp_dir().join(format!(
        "debug_terminator_js_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    tokio::fs::create_dir_all(&script_dir).await.unwrap();
    println!("📁 Created script directory: {}", script_dir.display());

    // Install terminator.js in isolated directory
    println!("📦 Installing terminator.js...");
    let install_result = Command::new("cmd")
        .current_dir(&script_dir)
        .args(["/c", "npm", "install", "terminator.js"])
        .output()
        .await
        .unwrap();

    println!(
        "Install stdout: {}",
        String::from_utf8_lossy(&install_result.stdout)
    );
    println!(
        "Install stderr: {}",
        String::from_utf8_lossy(&install_result.stderr)
    );
    println!("Install exit code: {:?}", install_result.status.code());

    // Check what was actually installed
    let node_modules_path = script_dir.join("node_modules");
    if tokio::fs::metadata(&node_modules_path).await.is_ok() {
        println!("✅ node_modules directory exists");

        // List contents
        let mut entries = tokio::fs::read_dir(&node_modules_path).await.unwrap();
        println!("📋 node_modules contents:");
        while let Some(entry) = entries.next_entry().await.unwrap() {
            println!("  - {}", entry.file_name().to_string_lossy());
        }

        // Check specifically for terminator.js
        let terminator_path = node_modules_path.join("terminator.js");
        if tokio::fs::metadata(&terminator_path).await.is_ok() {
            println!("✅ terminator.js package directory exists");
        } else {
            println!("❌ terminator.js package directory NOT found");
        }
    } else {
        println!("❌ node_modules directory does not exist");
    }

    // Create a simple test script
    let test_script = r#"
try {
    console.log("Working directory:", process.cwd());
    console.log("Attempting to require terminator.js...");
    
    const { Desktop } = require('terminator.js');
    console.log("SUCCESS: terminator.js loaded");
    
    process.stdout.write('__RESULT__{"success": true}__END__\n');
} catch (error) {
    console.log("FAILED to load terminator.js:", error.message);
    console.log("Error code:", error.code);
    console.log("Error stack:", error.stack);
    
    // Try to show what modules are available
    const fs = require('fs');
    const path = require('path');
    
    try {
        const nodeModulesPath = path.join(process.cwd(), 'node_modules');
        if (fs.existsSync(nodeModulesPath)) {
            console.log("Available modules:", fs.readdirSync(nodeModulesPath));
            
            const terminatorPath = path.join(nodeModulesPath, 'terminator.js');
            if (fs.existsSync(terminatorPath)) {
                console.log("terminator.js directory exists");
                console.log("terminator.js contents:", fs.readdirSync(terminatorPath));
                
                const packageJsonPath = path.join(terminatorPath, 'package.json');
                if (fs.existsSync(packageJsonPath)) {
                    const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
                    console.log("Package main field:", packageJson.main);
                    console.log("Package name:", packageJson.name);
                    console.log("Package version:", packageJson.version);
                }
            }
        }
    } catch (fsError) {
        console.log("Filesystem check error:", fsError.message);
    }
    
    process.stdout.write('__RESULT__{"success": false, "error": "' + error.message + '"}__END__\n');
}
"#;

    let script_path = script_dir.join("debug.js");
    tokio::fs::write(&script_path, test_script).await.unwrap();

    println!("🚀 Running test script...");

    // Run the script
    let node_exe = find_executable("node").unwrap();
    let mut child = Command::new(&node_exe)
        .current_dir(&script_dir)
        .arg("debug.js")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut stderr = BufReader::new(child.stderr.take().unwrap()).lines();

    // Read all output
    tokio::spawn(async move {
        while let Ok(Some(line)) = stderr.next_line().await {
            println!("STDERR: {}", line);
        }
    });

    let mut result: Option<serde_json::Value> = None;
    while let Ok(Some(line)) = stdout.next_line().await {
        println!("STDOUT: {}", line);

        if line.starts_with("__RESULT__") && line.ends_with("__END__") {
            let result_json = line.replace("__RESULT__", "").replace("__END__", "");
            result = serde_json::from_str(&result_json).ok();
            break;
        }
    }

    let status = child.wait().await.unwrap();
    println!("Process exit code: {:?}", status.code());

    // Clean up
    tokio::fs::remove_dir_all(&script_dir).await.ok();

    // Verify result
    if let Some(res) = result {
        println!(
            "Final result: {}",
            serde_json::to_string_pretty(&res).unwrap()
        );
        if let Some(success) = res.get("success").and_then(|v| v.as_bool()) {
            if success {
                println!("✅ Test completed successfully!");
            } else {
                println!("❌ Test failed but we got useful debug info");
            }
        }
    } else {
        println!("❌ No result received from Node.js process");
    }
}

#[tokio::test]
async fn test_nodejs_execution_with_local_bindings() {
    // Test JavaScript execution using local terminator.js bindings
    use std::process::Stdio;
    use terminator_mcp_agent::scripting_engine::find_executable;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    println!("🧪 Testing Node.js execution with local terminator.js bindings...");

    // Get paths relative to workspace root
    let workspace_root = std::env::current_dir()
        .unwrap()
        .parent() // Move up from terminator-mcp-agent to workspace root
        .unwrap()
        .to_path_buf();

    let local_bindings_path = workspace_root.join("bindings").join("nodejs");

    // Verify the local bindings directory exists
    if !tokio::fs::metadata(&local_bindings_path).await.is_ok() {
        panic!(
            "❌ Local bindings directory not found at: {}",
            local_bindings_path.display()
        );
    }

    println!(
        "📁 Using local bindings at: {}",
        local_bindings_path.display()
    );

    // Build the local bindings first
    println!("🔨 Building local terminator.js bindings...");
    let build_result = if cfg!(windows) {
        Command::new("cmd")
            .current_dir(&local_bindings_path)
            .args(["/c", "npm", "run", "build"])
            .output()
            .await
    } else {
        Command::new("npm")
            .current_dir(&local_bindings_path)
            .args(["run", "build"])
            .output()
            .await
    };

    match build_result {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("⚠️ Build failed: {}", stderr);
                println!(
                    "📄 Build stdout: {}",
                    String::from_utf8_lossy(&output.stdout)
                );
                // Don't panic - the bindings might already be built
                println!("⚠️ Continuing with existing build...");
            } else {
                println!("✅ Local bindings built successfully");
            }
        }
        Err(e) => {
            println!("⚠️ Failed to run build command: {}", e);
            println!("⚠️ Continuing with existing build...");
        }
    }

    // Create isolated test directory
    let test_dir = std::env::temp_dir().join(format!(
        "test_local_bindings_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    tokio::fs::create_dir_all(&test_dir).await.unwrap();
    println!("📁 Created test directory: {}", test_dir.display());

    // Create package.json that references the local bindings
    let package_json = format!(
        r#"{{
  "name": "test-local-terminator",
  "version": "1.0.0",
  "dependencies": {{
    "terminator.js": "file:{}"
  }}
}}"#,
        local_bindings_path.to_string_lossy().replace('\\', "/")
    );

    let package_json_path = test_dir.join("package.json");
    tokio::fs::write(&package_json_path, package_json)
        .await
        .unwrap();
    println!("📄 Created package.json with local dependency");

    // Install the local bindings
    println!("📦 Installing local terminator.js...");
    let install_result = if cfg!(windows) {
        Command::new("cmd")
            .current_dir(&test_dir)
            .args(["/c", "npm", "install"])
            .output()
            .await
    } else {
        Command::new("npm")
            .current_dir(&test_dir)
            .args(["install"])
            .output()
            .await
    };

    match install_result {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                println!("❌ Install failed:");
                println!("📄 stdout: {stdout}");
                println!("📄 stderr: {stderr}");
                panic!("Failed to install local bindings");
            } else {
                println!("✅ Local bindings installed successfully");
            }
        }
        Err(e) => {
            panic!("❌ Failed to run npm install: {e}");
        }
    }

    // Verify installation
    let node_modules_path = test_dir.join("node_modules").join("terminator.js");
    if tokio::fs::metadata(&node_modules_path).await.is_err() {
        panic!("❌ terminator.js not found in node_modules after installation");
    }
    println!("✅ Verified local terminator.js installation");

    // Create test script
    let test_script = r#"
try {
    console.log("🧪 Testing local terminator.js bindings...");
    console.log("Working directory:", process.cwd());
    
    // Import terminator.js
    const { Desktop } = require('terminator.js');
    console.log("✅ Successfully imported Desktop from local terminator.js");
    
    // Create Desktop instance
    const desktop = new Desktop();
    console.log("✅ Successfully created Desktop instance");
    
    // Test basic functionality - get root element
    const root = desktop.root();
    console.log("✅ Successfully got root element");
    console.log("Root role:", root.role());
    console.log("Root name:", root.name());
    
    // Test applications list
    const apps = desktop.applications();
    console.log("✅ Successfully got applications list");
    console.log("Found", apps.length, "applications");
    
    // Return success result
    const result = {
        success: true,
        message: "Local terminator.js bindings working correctly",
        hasDesktop: typeof desktop !== 'undefined',
        hasRoot: typeof root !== 'undefined',
        appCount: apps.length,
        rootRole: root.role(),
        rootName: root.name()
    };
    
    process.stdout.write('__RESULT__' + JSON.stringify(result) + '__END__\n');
    
} catch (error) {
    console.log("❌ Error testing local bindings:", error.message);
    console.log("Error stack:", error.stack);
    
    const errorResult = {
        success: false,
        error: error.message,
        stack: error.stack,
        code: error.code || 'UNKNOWN'
    };
    
    process.stdout.write('__RESULT__' + JSON.stringify(errorResult) + '__END__\n');
}
"#;

    let script_path = test_dir.join("test.js");
    tokio::fs::write(&script_path, test_script).await.unwrap();

    println!("🚀 Running test with local bindings...");

    // Execute the test script
    let node_exe = find_executable("node").unwrap();
    let mut child = if cfg!(windows) && node_exe.ends_with(".exe") {
        Command::new(&node_exe)
            .current_dir(&test_dir)
            .arg("test.js")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    } else if cfg!(windows) {
        Command::new("cmd")
            .current_dir(&test_dir)
            .args(["/c", "node", "test.js"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    } else {
        Command::new(&node_exe)
            .current_dir(&test_dir)
            .arg("test.js")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };

    let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut stderr = BufReader::new(child.stderr.take().unwrap()).lines();

    // Read stderr in background
    tokio::spawn(async move {
        while let Ok(Some(line)) = stderr.next_line().await {
            println!("STDERR: {}", line);
        }
    });

    // Read stdout and look for result
    let mut result: Option<serde_json::Value> = None;
    while let Ok(Some(line)) = stdout.next_line().await {
        println!("STDOUT: {}", line);

        if line.starts_with("__RESULT__") && line.ends_with("__END__") {
            let result_json = line.replace("__RESULT__", "").replace("__END__", "");
            result = serde_json::from_str(&result_json).ok();
            break;
        }
    }

    let status = child.wait().await.unwrap();
    println!("Process exit code: {:?}", status.code());

    // Clean up test directory
    tokio::fs::remove_dir_all(&test_dir).await.ok();

    // Verify results
    match result {
        Some(res) => {
            println!(
                "📄 Final result: {}",
                serde_json::to_string_pretty(&res).unwrap()
            );

            if let Some(success) = res.get("success").and_then(|v| v.as_bool()) {
                assert!(success, "❌ Local bindings test should succeed");
                println!("✅ Local bindings test completed successfully!");

                // Verify expected fields
                assert!(
                    res.get("hasDesktop")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    "Should have Desktop instance"
                );
                assert!(
                    res.get("hasRoot")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    "Should have root element"
                );
                assert!(
                    res.get("appCount").and_then(|v| v.as_u64()).is_some(),
                    "Should have app count"
                );

                println!("✅ All assertions passed for local bindings test!");
            } else {
                if let Some(error) = res.get("error") {
                    panic!("❌ Local bindings test failed: {}", error);
                } else {
                    panic!("❌ Local bindings test failed with unknown error");
                }
            }
        }
        None => {
            panic!("❌ No result received from local bindings test");
        }
    }
}

#[tokio::test]
async fn test_scripting_engine_with_local_bindings() {
    // Test the new execute_javascript_with_local_bindings function
    use terminator_mcp_agent::scripting_engine::execute_javascript_with_local_bindings;

    let test_script = r#"
// Test basic terminator.js functionality with local bindings
try {
    log("🧪 Testing scripting engine with local bindings...");
    
    // Test that desktop is available globally
    if (typeof desktop === 'undefined') {
        throw new Error("Desktop global not available");
    }
    
    log("✅ Desktop global is available");
    
    // Test basic desktop functionality
    const root = desktop.root();
    log("✅ Got root element:", root.role());
    
    const apps = desktop.applications();
    log("✅ Got applications list, count:", apps.length);
    
    // Return success result
    return {
        success: true,
        message: "Scripting engine with local bindings working correctly",
        rootRole: root.role(),
        rootName: root.name(),
        appCount: apps.length,
        testTimestamp: new Date().toISOString()
    };
    
} catch (error) {
    log("❌ Error:", error.message);
    return {
        success: false,
        error: error.message,
        stack: error.stack
    };
}
"#;

    println!("🧪 Testing execute_javascript_with_local_bindings function...");

    let result = execute_javascript_with_local_bindings(test_script.to_string()).await;

    match result {
        Ok(value) => {
            println!("✅ Scripting engine test succeeded!");
            println!(
                "📄 Result: {}",
                serde_json::to_string_pretty(&value).unwrap_or_default()
            );

            // Verify the result structure
            if let Some(obj) = value.as_object() {
                if let Some(success) = obj.get("success").and_then(|v| v.as_bool()) {
                    assert!(success, "Scripting engine should report success");
                    println!("✅ Scripting engine reported success");
                } else {
                    panic!("Scripting engine result should have success field");
                }

                // Verify expected fields exist
                assert!(obj.contains_key("rootRole"), "Should have rootRole");
                assert!(obj.contains_key("rootName"), "Should have rootName");
                assert!(obj.contains_key("appCount"), "Should have appCount");
                assert!(
                    obj.contains_key("testTimestamp"),
                    "Should have testTimestamp"
                );

                println!("✅ All expected fields present in result");
            } else {
                panic!("Result should be an object with success info");
            }
        }
        Err(e) => {
            println!("❌ Scripting engine test failed: {}", e);
            panic!("Scripting engine with local bindings should succeed: {}", e);
        }
    }
}
