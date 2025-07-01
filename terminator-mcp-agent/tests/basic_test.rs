#[cfg(test)]
mod tests {
    use terminator::{Desktop, DesktopAutomationError};

    #[tokio::test]
    async fn test_desktop_creation() {
        // Simple test to verify we can create a desktop instance
        let result = Desktop::new().await;
        assert!(result.is_ok(), "Should be able to create desktop instance");
    }

    #[tokio::test]
    async fn test_list_applications() {
        // Test listing applications
        let desktop = match Desktop::new().await {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Failed to create desktop: {:?}", e);
                return;
            }
        };

        let apps = desktop.list_applications().await;
        assert!(apps.is_ok(), "Should be able to list applications");

        let app_list = apps.unwrap();
        println!("Found {} applications", app_list.len());
    }

    #[test]
    fn test_simple_math() {
        // Basic test to ensure testing framework works
        assert_eq!(2 + 2, 4);
        assert_eq!(10 * 5, 50);
    }
}
