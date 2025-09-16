#[cfg(test)]
mod remote_integration_tests {
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use terminator_mcp_agent::remote_client::{RemoteUIAutomationBuilder, RemoteUIAutomationClient};
    use terminator_mcp_agent::remote_server::{start_remote_server, WaitCondition, MouseButton};
    use terminator_mcp_agent::utils::DesktopWrapper;

    struct TestServer {
        port: u16,
        shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    }

    impl TestServer {
        async fn start() -> anyhow::Result<Self> {
            let port = get_available_port();
            let desktop = Arc::new(Mutex::new(DesktopWrapper::new().await?));
            let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

            let desktop_clone = desktop.clone();
            tokio::spawn(async move {
                tokio::select! {
                    result = start_remote_server(desktop_clone, port) => {
                        if let Err(e) = result {
                            eprintln!("Server error: {}", e);
                        }
                    }
                    _ = &mut shutdown_rx => {
                        println!("Server shutting down");
                    }
                }
            });

            tokio::time::sleep(Duration::from_millis(500)).await;

            Ok(TestServer {
                port,
                shutdown_tx: Some(shutdown_tx),
            })
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            if let Some(tx) = self.shutdown_tx.take() {
                let _ = tx.send(());
            }
        }
    }

    fn get_available_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        port
    }

    #[tokio::test]
    async fn test_health_check() -> anyhow::Result<()> {
        let server = TestServer::start().await?;

        let client = RemoteUIAutomationBuilder::new()
            .with_url(&format!("http://localhost:{}", server.port))
            .build()?;

        let health = client.health_check().await?;

        assert_eq!(health["status"], "healthy");
        assert_eq!(health["service"], "remote-ui-automation");

        Ok(())
    }

    #[tokio::test]
    async fn test_get_applications() -> anyhow::Result<()> {
        let server = TestServer::start().await?;

        let client = RemoteUIAutomationBuilder::new()
            .with_url(&format!("http://localhost:{}", server.port))
            .build()?;

        let apps = client.get_applications().await?;

        assert!(!apps.is_empty(), "Should have at least one application running");

        Ok(())
    }

    #[tokio::test]
    async fn test_validate_element() -> anyhow::Result<()> {
        let server = TestServer::start().await?;

        let client = RemoteUIAutomationBuilder::new()
            .with_url(&format!("http://localhost:{}", server.port))
            .build()?;

        let validation = client.validate_element("role:Window").await?;

        if validation.exists {
            assert!(validation.name.is_some());
            assert!(validation.role.is_some());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_api_key_authentication() -> anyhow::Result<()> {
        std::env::set_var("REMOTE_API_KEY", "test-secret-key");

        let server = TestServer::start().await?;

        let client_with_key = RemoteUIAutomationBuilder::new()
            .with_url(&format!("http://localhost:{}", server.port))
            .with_api_key("test-secret-key")
            .build()?;

        let health = client_with_key.health_check().await?;
        assert_eq!(health["status"], "healthy");

        let client_without_key = RemoteUIAutomationBuilder::new()
            .with_url(&format!("http://localhost:{}", server.port))
            .build()?;

        let apps_result = client_without_key.get_applications().await;
        assert!(apps_result.is_err());

        std::env::remove_var("REMOTE_API_KEY");

        Ok(())
    }

    #[tokio::test]
    async fn test_batch_operations() -> anyhow::Result<()> {
        let server = TestServer::start().await?;

        let client = RemoteUIAutomationBuilder::new()
            .with_url(&format!("http://localhost:{}", server.port))
            .build()?;

        let validation1 = client.validate_element("role:Window").await?;
        let validation2 = client.validate_element("role:Application").await?;

        assert!(validation1.exists || validation2.exists);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_full_automation_flow() -> anyhow::Result<()> {
        let server = TestServer::start().await?;

        let client = RemoteUIAutomationBuilder::new()
            .with_url(&format!("http://localhost:{}", server.port))
            .build()?;

        let apps = client.get_applications().await?;
        if !apps.is_empty() {
            let tree = client.get_window_tree(None, true).await?;
            assert!(!tree.is_null());
        }

        Ok(())
    }
}

#[cfg(test)]
mod remote_unit_tests {
    use terminator_mcp_agent::remote_protocol::*;

    #[test]
    fn test_message_metadata_default() {
        let metadata = MessageMetadata::default();

        assert_eq!(metadata.retry_count, 0);
        assert!(metadata.session_id.is_none());
        assert!(metadata.headers.is_empty());
        assert!(!metadata.correlation_id.is_empty());
    }

    #[test]
    fn test_event_type_serialization() {
        let event = EventType::ElementChanged;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#""element_changed""#);

        let custom_event = EventType::Custom("my_custom_event".to_string());
        let json = serde_json::to_string(&custom_event).unwrap();
        assert!(json.contains("my_custom_event"));
    }

    #[test]
    fn test_capability_info() {
        let capability = Capability {
            name: "screenshot".to_string(),
            supported: true,
            options: Some(serde_json::json!({
                "formats": ["png", "jpeg"],
                "max_resolution": "4K"
            })),
        };

        let info = CapabilityInfo {
            capabilities: vec![capability],
            platform: PlatformInfo {
                os: "Windows".to_string(),
                arch: "x86_64".to_string(),
                ui_framework: "UIAutomation".to_string(),
            },
            version: "1.0.0".to_string(),
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: CapabilityInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, "1.0.0");
        assert_eq!(deserialized.platform.os, "Windows");
        assert_eq!(deserialized.capabilities.len(), 1);
    }
}