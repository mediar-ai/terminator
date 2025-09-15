use anyhow::Result;
use axum::{routing::post, Router};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use super::traces::StepsTracker;
use super::traces::handle_traces;

pub struct TelemetryReceiver {
    port: u16,
}

impl TelemetryReceiver {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn start(self) -> Result<JoinHandle<()>> {
        let steps_state = Arc::new(Mutex::new(StepsTracker::new()));

        let app = Router::new()
            .route("/v1/traces", post(handle_traces))
            .with_state(steps_state);

        let addr = format!("127.0.0.1:{}", self.port);

        let handle = tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });

        // Give it a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(handle)
    }
}
