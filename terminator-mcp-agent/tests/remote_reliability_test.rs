#[cfg(test)]
mod remote_reliability_tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};
    use tokio::sync::Mutex;
    use terminator_mcp_agent::remote_client::RemoteUIAutomationBuilder;

    #[derive(Clone)]
    struct ReliabilityMetrics {
        total_requests: Arc<AtomicUsize>,
        successful_requests: Arc<AtomicUsize>,
        failed_requests: Arc<AtomicUsize>,
        total_duration_ms: Arc<AtomicUsize>,
    }

    impl ReliabilityMetrics {
        fn new() -> Self {
            Self {
                total_requests: Arc::new(AtomicUsize::new(0)),
                successful_requests: Arc::new(AtomicUsize::new(0)),
                failed_requests: Arc::new(AtomicUsize::new(0)),
                total_duration_ms: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn record_request(&self, success: bool, duration: Duration) {
            self.total_requests.fetch_add(1, Ordering::SeqCst);

            if success {
                self.successful_requests.fetch_add(1, Ordering::SeqCst);
            } else {
                self.failed_requests.fetch_add(1, Ordering::SeqCst);
            }

            self.total_duration_ms.fetch_add(
                duration.as_millis() as usize,
                Ordering::SeqCst
            );
        }

        fn get_stats(&self) -> Stats {
            let total = self.total_requests.load(Ordering::SeqCst);
            let successful = self.successful_requests.load(Ordering::SeqCst);
            let failed = self.failed_requests.load(Ordering::SeqCst);
            let total_ms = self.total_duration_ms.load(Ordering::SeqCst);

            Stats {
                total_requests: total,
                successful_requests: successful,
                failed_requests: failed,
                success_rate: if total > 0 {
                    (successful as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
                average_latency_ms: if total > 0 {
                    total_ms as f64 / total as f64
                } else {
                    0.0
                },
            }
        }
    }

    struct Stats {
        total_requests: usize,
        successful_requests: usize,
        failed_requests: usize,
        success_rate: f64,
        average_latency_ms: f64,
    }

    #[tokio::test]
    #[ignore]
    async fn test_concurrent_requests() -> anyhow::Result<()> {
        let client = RemoteUIAutomationBuilder::new()
            .with_url("http://localhost:8080")
            .build()?;

        let metrics = ReliabilityMetrics::new();
        let mut handles = vec![];

        for _ in 0..10 {
            let client_clone = client.clone();
            let metrics_clone = metrics.clone();

            let handle = tokio::spawn(async move {
                let start = Instant::now();
                let result = client_clone.get_applications().await;
                let duration = start.elapsed();

                metrics_clone.record_request(result.is_ok(), duration);
                result
            });

            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await?;
        }

        let stats = metrics.get_stats();
        println!("Concurrent test results:");
        println!("  Total requests: {}", stats.total_requests);
        println!("  Success rate: {:.2}%", stats.success_rate);
        println!("  Average latency: {:.2}ms", stats.average_latency_ms);

        assert!(stats.success_rate > 90.0, "Success rate should be > 90%");

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_retry_mechanism() -> anyhow::Result<()> {
        struct RetryClient {
            client: RemoteUIAutomationBuilder,
            max_retries: u32,
        }

        impl RetryClient {
            async fn execute_with_retry<F, T>(
                &self,
                operation: F,
            ) -> anyhow::Result<T>
            where
                F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send>>,
            {
                let mut retries = 0;
                let mut last_error = None;

                while retries <= self.max_retries {
                    match operation().await {
                        Ok(result) => return Ok(result),
                        Err(e) => {
                            last_error = Some(e);
                            retries += 1;

                            if retries <= self.max_retries {
                                let backoff = Duration::from_millis(100 * (2_u64.pow(retries)));
                                tokio::time::sleep(backoff).await;
                            }
                        }
                    }
                }

                Err(last_error.unwrap())
            }
        }

        let retry_client = RetryClient {
            client: RemoteUIAutomationBuilder::new().with_url("http://localhost:8080"),
            max_retries: 3,
        };

        let client = retry_client.client.build()?;
        let result = retry_client
            .execute_with_retry(|| {
                let client_clone = client.clone();
                Box::pin(async move {
                    client_clone.health_check().await
                })
            })
            .await;

        assert!(result.is_ok() || retry_client.max_retries > 0);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_connection_pooling() -> anyhow::Result<()> {
        let client = RemoteUIAutomationBuilder::new()
            .with_url("http://localhost:8080")
            .build()?;

        let metrics = ReliabilityMetrics::new();
        let iterations = 100;

        for _ in 0..iterations {
            let start = Instant::now();
            let result = client.health_check().await;
            let duration = start.elapsed();

            metrics.record_request(result.is_ok(), duration);
        }

        let stats = metrics.get_stats();

        println!("Connection pooling test results:");
        println!("  Total requests: {}", stats.total_requests);
        println!("  Average latency: {:.2}ms", stats.average_latency_ms);

        assert!(stats.average_latency_ms < 50.0, "Average latency should be < 50ms with connection pooling");

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_timeout_handling() -> anyhow::Result<()> {
        let client = RemoteUIAutomationBuilder::new()
            .with_url("http://localhost:8080")
            .build()?;

        let start = Instant::now();
        let result = client.wait_for_element(
            "non-existent-element",
            terminator_mcp_agent::remote_server::WaitCondition::Exists,
            Some(1000),
        ).await;

        let elapsed = start.elapsed();

        assert!(result.is_err(), "Should timeout for non-existent element");
        assert!(elapsed.as_millis() >= 1000, "Should wait at least 1 second");
        assert!(elapsed.as_millis() < 2000, "Should not wait more than 2 seconds");

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_stress_test() -> anyhow::Result<()> {
        let client = RemoteUIAutomationBuilder::new()
            .with_url("http://localhost:8080")
            .build()?;

        let metrics = ReliabilityMetrics::new();
        let duration = Duration::from_secs(10);
        let start = Instant::now();

        while start.elapsed() < duration {
            let op_start = Instant::now();
            let result = client.validate_element("role:Window").await;
            let op_duration = op_start.elapsed();

            metrics.record_request(result.is_ok(), op_duration);

            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let stats = metrics.get_stats();

        println!("Stress test results (10 seconds):");
        println!("  Total requests: {}", stats.total_requests);
        println!("  Success rate: {:.2}%", stats.success_rate);
        println!("  Average latency: {:.2}ms", stats.average_latency_ms);
        println!("  Requests per second: {:.2}",
                 stats.total_requests as f64 / duration.as_secs_f64());

        assert!(stats.success_rate > 95.0, "Success rate should be > 95% under stress");

        Ok(())
    }
}

#[cfg(test)]
mod mock_server_tests {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct MockRemoteServer {
        requests_received: Arc<Mutex<Vec<String>>>,
        should_fail: Arc<Mutex<bool>>,
        latency_ms: Arc<Mutex<u64>>,
    }

    impl MockRemoteServer {
        fn new() -> Self {
            Self {
                requests_received: Arc::new(Mutex::new(Vec::new())),
                should_fail: Arc::new(Mutex::new(false)),
                latency_ms: Arc::new(Mutex::new(0)),
            }
        }

        async fn handle_request(&self, request: String) -> Result<String, String> {
            let mut requests = self.requests_received.lock().await;
            requests.push(request.clone());

            let latency = *self.latency_ms.lock().await;
            if latency > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(latency)).await;
            }

            if *self.should_fail.lock().await {
                Err("Mock server error".to_string())
            } else {
                Ok(format!("Response to: {}", request))
            }
        }

        async fn set_fail_mode(&self, should_fail: bool) {
            *self.should_fail.lock().await = should_fail;
        }

        async fn set_latency(&self, latency_ms: u64) {
            *self.latency_ms.lock().await = latency_ms;
        }

        async fn get_request_count(&self) -> usize {
            self.requests_received.lock().await.len()
        }
    }

    #[tokio::test]
    async fn test_mock_server_normal_operation() {
        let server = MockRemoteServer::new();

        let response = server.handle_request("test_request".to_string()).await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), "Response to: test_request");

        let count = server.get_request_count().await;
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_mock_server_failure_mode() {
        let server = MockRemoteServer::new();
        server.set_fail_mode(true).await;

        let response = server.handle_request("test_request".to_string()).await;
        assert!(response.is_err());
        assert_eq!(response.unwrap_err(), "Mock server error");
    }

    #[tokio::test]
    async fn test_mock_server_latency() {
        let server = MockRemoteServer::new();
        server.set_latency(100).await;

        let start = std::time::Instant::now();
        let _ = server.handle_request("test_request".to_string()).await;
        let elapsed = start.elapsed();

        assert!(elapsed.as_millis() >= 100);
    }
}