mod e2e_tests;

// Re-export test modules
pub use e2e_tests::*;

// Initialize tracing for tests
pub fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}

// Test helper functions
pub async fn setup_test_environment() -> crate::Desktop {
    // Initialize desktop with test settings
    crate::Desktop::new(false, false).await.unwrap()
}

pub async fn cleanup_test_environment() {
    // Add any cleanup code here
}

// Test constants
pub const TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
pub const TEST_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(500); 