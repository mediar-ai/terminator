// Example demonstrating how to use the remote UI automation system
// This shows both local VM and Azure VM scenarios

use anyhow::Result;
use terminator_mcp_agent::{
    vm_connector::{VMConnectorFactory, VMConfig, HypervisorType, AzureConnectionMethod},
    remote_automation::{UIAutomation, LocalUIAutomation},
    remote_client::RemoteUIAutomationBuilder,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Example 1: Local Hyper-V VM
    println!("=== Example 1: Local Hyper-V VM ===");
    run_local_vm_example().await?;

    // Example 2: Azure VM with remote agent
    println!("\n=== Example 2: Azure VM ===");
    run_azure_vm_example().await?;

    // Example 3: Remote HTTP agent
    println!("\n=== Example 3: Remote HTTP Agent ===");
    run_remote_agent_example().await?;

    Ok(())
}

async fn run_local_vm_example() -> Result<()> {
    println!("Connecting to local Hyper-V VM...");

    // Create a local VM connector
    let vm_config = VMConfig::Local {
        hypervisor: HypervisorType::HyperV,
        vm_name: "Windows11-Dev".to_string(),
        connection_method: terminator_mcp_agent::vm_connector::LocalConnectionMethod::RDP {
            port: 3389,
            credentials: None,
        },
    };

    let connector = VMConnectorFactory::create_from_config(vm_config);

    // Check VM status
    let status = connector.get_status().await?;
    println!("VM Status: {:?}", status);

    // Start VM if not running
    match status {
        terminator_mcp_agent::vm_connector::VMStatus::Stopped => {
            println!("Starting VM...");
            connector.start_vm().await?;
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
        _ => {}
    }

    // Connect to the VM
    let connection_info = connector.connect().await?;
    println!("Connected to: {}", connection_info.vm_name);
    println!("Connection string: {}", connection_info.connection_string);

    // Deploy agent if needed
    if std::env::var("DEPLOY_AGENT").unwrap_or_default() == "true" {
        println!("Deploying remote UI automation agent...");
        connector.deploy_agent("./target/release/remote-ui-agent.exe").await?;
    }

    // Now connect to the remote agent on the VM
    let client = RemoteUIAutomationBuilder::new()
        .with_url(&format!("http://{}:8080", get_vm_ip(&connection_info.vm_name).await?))
        .build()?;

    // Perform UI automation
    println!("Getting applications...");
    let apps = client.get_applications().await?;
    println!("Found {} applications", apps.len());

    // Validate that we can find a window
    let validation = client.validate_element("role:Window").await?;
    println!("Window found: {}", validation.exists);

    Ok(())
}

async fn run_azure_vm_example() -> Result<()> {
    println!("Connecting to Azure VM...");

    // Create an Azure VM connector
    let vm_config = VMConfig::Azure {
        subscription_id: "5c0a60d0-92cf-47ca-9430-b462bc2fe194".to_string(),
        resource_group: "AVD-TERMINATOR-RG".to_string(),
        vm_name: "mcp-test-vm".to_string(),
        connection_method: AzureConnectionMethod::RemoteAgent { port: 8080 },
    };

    let connector = VMConnectorFactory::create_from_config(vm_config);

    // Check VM status
    let status = connector.get_status().await?;
    println!("Azure VM Status: {:?}", status);

    // Ensure VM is running
    match status {
        terminator_mcp_agent::vm_connector::VMStatus::Stopped => {
            println!("Starting Azure VM...");
            connector.start_vm().await?;

            // Wait for VM to start
            println!("Waiting for VM to start...");
            for _ in 0..60 {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                let status = connector.get_status().await?;
                if matches!(status, terminator_mcp_agent::vm_connector::VMStatus::Running) {
                    break;
                }
            }
        }
        _ => {}
    }

    // Connect to the VM
    let connection_info = connector.connect().await?;
    println!("Connected to Azure VM: {}", connection_info.vm_name);
    println!("Connection URL: {}", connection_info.connection_string);

    // Use the remote client to perform UI automation
    let client = RemoteUIAutomationBuilder::new()
        .with_url(&connection_info.connection_string)
        .with_api_key(&std::env::var("AZURE_VM_API_KEY").unwrap_or_default())
        .build()?;

    // Health check
    let health = client.health_check().await?;
    println!("Remote agent health: {:?}", health);

    // Example: Click on Start menu
    println!("Clicking Start menu...");
    client.click("role:Button|name:Start", None).await?;

    // Example: Type in search box
    println!("Typing in search...");
    client.type_text("role:Edit|name:Search", "notepad").await?;

    // Example: Take screenshot
    println!("Taking screenshot...");
    let screenshot = client.take_screenshot(None, false).await?;
    std::fs::write("azure_vm_screenshot.png", screenshot)?;
    println!("Screenshot saved to azure_vm_screenshot.png");

    Ok(())
}

async fn run_remote_agent_example() -> Result<()> {
    println!("Connecting to remote HTTP agent...");

    // Try to connect from environment variables first
    let connector = match VMConnectorFactory::create_from_env() {
        Ok(c) => c,
        Err(_) => {
            // Fallback to default remote configuration
            let vm_config = VMConfig::Remote {
                host: "localhost".to_string(),
                port: 8080,
                api_key: None,
            };
            VMConnectorFactory::create_from_config(vm_config)
        }
    };

    // Test connection
    let connection_info = connector.connect().await?;
    println!("Connected to: {}", connection_info.connection_string);

    // Create client
    let client = RemoteUIAutomationBuilder::new()
        .with_url(&connection_info.connection_string)
        .build()?;

    // Perform a series of automation tasks
    println!("Running automation sequence...");

    // 1. Find and validate calculator
    println!("Looking for Calculator...");
    let calc_validation = client.validate_element("role:Window|name:Calculator").await;

    if calc_validation.is_ok() && calc_validation.unwrap().exists {
        println!("Calculator found!");

        // 2. Click buttons
        println!("Clicking number buttons...");
        client.click("role:Button|name:7", None).await?;
        client.click("role:Button|name:Plus", None).await?;
        client.click("role:Button|name:3", None).await?;
        client.click("role:Button|name:Equals", None).await?;

        // 3. Get result
        let result_props = client.get_element_properties("role:Text|name:Display").await?;
        println!("Calculator result: {:?}", result_props);
    } else {
        println!("Calculator not found, opening it...");

        // Open calculator using Start menu
        client.press_key("role:Desktop", "{LWin}").await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        client.type_text("role:Edit", "calc").await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        client.press_key("role:Edit", "{Enter}").await?;

        // Wait for calculator to open
        client.wait_for_element(
            "role:Window|name:Calculator",
            terminator_mcp_agent::remote_common::WaitCondition::Exists,
            Some(5000),
        ).await?;

        println!("Calculator opened successfully!");
    }

    Ok(())
}

async fn get_vm_ip(vm_name: &str) -> Result<String> {
    // This is a simplified version - in real implementation,
    // you'd query the hypervisor for the VM's IP address

    let output = tokio::process::Command::new("powershell")
        .args(&[
            "-Command",
            &format!(
                "(Get-VM -Name '{}' | Get-VMNetworkAdapter).IPAddresses | Select-Object -First 1",
                vm_name
            ),
        ])
        .output()
        .await?;

    let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if ip.is_empty() {
        // Fallback to localhost for testing
        Ok("127.0.0.1".to_string())
    } else {
        Ok(ip)
    }
}