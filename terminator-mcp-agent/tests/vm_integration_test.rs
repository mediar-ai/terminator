#[cfg(test)]
mod vm_integration_tests {
    use terminator_mcp_agent::vm_connector::*;
    use terminator_mcp_agent::remote_automation::*;
    use terminator_mcp_agent::remote_client::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    #[ignore] // Run with: cargo test --ignored test_local_hyperv_connection
    async fn test_local_hyperv_connection() -> anyhow::Result<()> {
        let connector = LocalVMConnector::new(
            HypervisorType::HyperV,
            "TestVM".to_string(),
        );

        // Check VM status
        let status = connector.get_status().await?;
        println!("VM Status: {:?}", status);

        // Start VM if needed
        if matches!(status, VMStatus::Stopped) {
            connector.start_vm().await?;

            // Wait for VM to start
            let mut retries = 0;
            while retries < 30 {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let status = connector.get_status().await?;
                if matches!(status, VMStatus::Running) {
                    break;
                }
                retries += 1;
            }
        }

        // Connect to VM
        let connection_info = connector.connect().await?;
        assert_eq!(connection_info.vm_name, "TestVM");
        assert!(matches!(connection_info.status, VMStatus::Running));

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test --ignored test_azure_vm_connection
    async fn test_azure_vm_connection() -> anyhow::Result<()> {
        // Ensure Azure CLI is logged in
        let output = tokio::process::Command::new("az")
            .args(&["account", "show"])
            .output()
            .await?;

        if !output.status.success() {
            panic!("Azure CLI not logged in. Run 'az login' first.");
        }

        let connector = AzureVMConnector::new(
            "5c0a60d0-92cf-47ca-9430-b462bc2fe194".to_string(),
            "AVD-TERMINATOR-RG".to_string(),
            "mcp-test-vm".to_string(),
        );

        // Check VM status
        let status = connector.get_status().await?;
        println!("Azure VM Status: {:?}", status);

        // Connect to VM
        let connection_info = connector.connect().await?;
        assert_eq!(connection_info.vm_name, "mcp-test-vm");

        Ok(())
    }

    #[tokio::test]
    async fn test_vm_config_serialization() {
        let config = VMConfig::Local {
            hypervisor: HypervisorType::HyperV,
            vm_name: "TestVM".to_string(),
            connection_method: LocalConnectionMethod::VMConnect,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: VMConfig = serde_json::from_str(&json).unwrap();

        match deserialized {
            VMConfig::Local { vm_name, .. } => {
                assert_eq!(vm_name, "TestVM");
            }
            _ => panic!("Wrong config type"),
        }
    }

    #[tokio::test]
    async fn test_vm_connector_factory() {
        // Test local VM creation
        let local_config = VMConfig::Local {
            hypervisor: HypervisorType::HyperV,
            vm_name: "TestVM".to_string(),
            connection_method: LocalConnectionMethod::VMConnect,
        };

        let connector = VMConnectorFactory::create_from_config(local_config);
        assert!(connector.get_status().await.is_ok());

        // Test Azure VM creation
        let azure_config = VMConfig::Azure {
            subscription_id: "test-sub".to_string(),
            resource_group: "test-rg".to_string(),
            vm_name: "test-vm".to_string(),
            connection_method: AzureConnectionMethod::RemoteAgent { port: 8080 },
        };

        let connector = VMConnectorFactory::create_from_config(azure_config);
        // This will fail without proper Azure credentials, but should not panic
        let _ = connector.get_status().await;

        // Test remote HTTP creation
        let remote_config = VMConfig::Remote {
            host: "localhost".to_string(),
            port: 8080,
            api_key: None,
        };

        let connector = VMConnectorFactory::create_from_config(remote_config);
        // This will fail if no server is running, but should not panic
        let _ = connector.get_status().await;
    }

    #[tokio::test]
    async fn test_environment_config() {
        // Test with local VM environment
        std::env::set_var("VM_TYPE", "local");
        std::env::set_var("HYPERVISOR", "hyperv");
        std::env::set_var("VM_NAME", "EnvTestVM");

        let connector = VMConnectorFactory::create_from_env();
        assert!(connector.is_ok());

        // Clean up environment
        std::env::remove_var("VM_TYPE");
        std::env::remove_var("HYPERVISOR");
        std::env::remove_var("VM_NAME");

        // Test with Azure VM environment
        std::env::set_var("VM_TYPE", "azure");
        std::env::set_var("AZURE_SUBSCRIPTION_ID", "test-sub");
        std::env::set_var("AZURE_RESOURCE_GROUP", "test-rg");
        std::env::set_var("AZURE_VM_NAME", "test-vm");

        let connector = VMConnectorFactory::create_from_env();
        assert!(connector.is_ok());

        // Clean up environment
        std::env::remove_var("VM_TYPE");
        std::env::remove_var("AZURE_SUBSCRIPTION_ID");
        std::env::remove_var("AZURE_RESOURCE_GROUP");
        std::env::remove_var("AZURE_VM_NAME");
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test --ignored test_full_automation_flow
    async fn test_full_automation_flow() -> anyhow::Result<()> {
        // This test demonstrates the full flow from VM connection to UI automation

        // Step 1: Create VM connector based on environment
        let connector = VMConnectorFactory::create_from_env()
            .unwrap_or_else(|_| {
                VMConnectorFactory::create_from_config(VMConfig::Remote {
                    host: "localhost".to_string(),
                    port: 8080,
                    api_key: None,
                })
            });

        // Step 2: Ensure VM is running
        let status = connector.get_status().await?;
        if !matches!(status, VMStatus::Running) {
            connector.start_vm().await?;

            // Wait for VM to be ready
            for _ in 0..30 {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if matches!(connector.get_status().await?, VMStatus::Running) {
                    break;
                }
            }
        }

        // Step 3: Connect to VM
        let connection_info = connector.connect().await?;
        println!("Connected to: {}", connection_info.vm_name);

        // Step 4: Deploy agent if needed
        if std::env::var("DEPLOY_AGENT").unwrap_or_default() == "true" {
            connector.deploy_agent("./target/release/remote-ui-agent.exe").await?;
        }

        // Step 5: Create UI automation client
        let client = RemoteUIAutomationBuilder::new()
            .with_url(&connection_info.connection_string)
            .build()?;

        // Step 6: Perform UI automation
        let health = client.health_check().await?;
        assert_eq!(health["status"], "healthy");

        let apps = client.get_applications().await?;
        assert!(!apps.is_empty());

        // Step 7: Disconnect
        connector.disconnect().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_credentials_handling() {
        let creds = Credentials {
            username: "testuser".to_string(),
            password: Some("testpass".to_string()),
            use_key_vault: false,
            key_vault_secret: None,
        };

        // Serialize (password should be skipped)
        let json = serde_json::to_string(&creds).unwrap();
        assert!(!json.contains("testpass"));
        assert!(json.contains("testuser"));

        // Test with Key Vault
        let vault_creds = Credentials {
            username: "vaultuser".to_string(),
            password: None,
            use_key_vault: true,
            key_vault_secret: Some("secret-name".to_string()),
        };

        let json = serde_json::to_string(&vault_creds).unwrap();
        assert!(json.contains("secret-name"));
        assert!(json.contains("true")); // use_key_vault
    }

    #[tokio::test]
    #[ignore] // Run manually when Azure VMs are available
    async fn test_azure_vm_operations() -> anyhow::Result<()> {
        let connector = AzureVMConnector::new(
            "5c0a60d0-92cf-47ca-9430-b462bc2fe194".to_string(),
            "MCP-SIMPLE-RG".to_string(),
            "mcp-simple-vm".to_string(),
        );

        // Test all VM operations
        println!("Testing Azure VM operations...");

        // 1. Get initial status
        let initial_status = connector.get_status().await?;
        println!("Initial status: {:?}", initial_status);

        // 2. Stop VM if running
        if matches!(initial_status, VMStatus::Running) {
            println!("Stopping VM...");
            connector.stop_vm().await?;

            // Wait for stop
            for _ in 0..60 {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                if matches!(connector.get_status().await?, VMStatus::Stopped) {
                    break;
                }
            }
        }

        // 3. Start VM
        println!("Starting VM...");
        connector.start_vm().await?;

        // Wait for start
        for _ in 0..60 {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if matches!(connector.get_status().await?, VMStatus::Running) {
                break;
            }
        }

        // 4. Restart VM
        println!("Restarting VM...");
        connector.restart_vm().await?;

        // 5. Final status check
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        let final_status = connector.get_status().await?;
        assert!(matches!(final_status, VMStatus::Running));

        println!("Azure VM operations test completed successfully!");

        Ok(())
    }
}