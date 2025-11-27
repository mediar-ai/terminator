//! Tests for TypeScript workflow telemetry integration
//!
//! This test verifies that TypeScript workflows properly propagate
//! execution_id and trace_id through tracing spans, enabling log
//! correlation in ClickHouse.

#[cfg(test)]
mod typescript_telemetry_tests {
    use std::sync::{Arc, Mutex};
    use tracing::{info, info_span, Instrument};
    use tracing_subscriber::layer::SubscriberExt;

    #[derive(Debug, Clone)]
    struct CapturedLog {
        message: String,
        execution_id: Option<String>,
        trace_id: Option<String>,
    }

    struct TestCapturingLayer {
        logs: Arc<Mutex<Vec<CapturedLog>>>,
    }

    impl<S> tracing_subscriber::Layer<S> for TestCapturingLayer
    where
        S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let mut message = String::new();
            let mut visitor = MessageVisitor(&mut message);
            event.record(&mut visitor);

            let mut execution_id = None;
            let mut trace_id = None;

            if let Some(scope) = ctx.event_scope(event) {
                for span in scope {
                    let extensions = span.extensions();
                    if let Some(fields) = extensions.get::<SpanFields>() {
                        if execution_id.is_none() {
                            execution_id = fields.execution_id.clone();
                        }
                        if trace_id.is_none() {
                            trace_id = fields.trace_id.clone();
                        }
                    }
                }
            }

            if let Ok(mut logs) = self.logs.lock() {
                logs.push(CapturedLog {
                    message,
                    execution_id,
                    trace_id,
                });
            }
        }

        fn on_new_span(
            &self,
            attrs: &tracing::span::Attributes<'_>,
            id: &tracing::span::Id,
            ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let mut fields = SpanFields::default();
            attrs.record(&mut FieldVisitor(&mut fields));

            if let Some(span) = ctx.span(id) {
                span.extensions_mut().insert(fields);
            }
        }
    }

    #[derive(Default)]
    struct SpanFields {
        execution_id: Option<String>,
        trace_id: Option<String>,
    }

    struct FieldVisitor<'a>(&'a mut SpanFields);

    impl<'a> tracing::field::Visit for FieldVisitor<'a> {
        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            match field.name() {
                "execution_id" => self.0.execution_id = Some(value.to_string()),
                "trace_id" => self.0.trace_id = Some(value.to_string()),
                _ => {}
            }
        }

        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            match field.name() {
                "execution_id" => self.0.execution_id = Some(format!("{value:?}")),
                "trace_id" => self.0.trace_id = Some(format!("{value:?}")),
                _ => {}
            }
        }
    }

    struct MessageVisitor<'a>(&'a mut String);

    impl<'a> tracing::field::Visit for MessageVisitor<'a> {
        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            if field.name() == "message" {
                *self.0 = value.to_string();
            }
        }

        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            if field.name() == "message" {
                *self.0 = format!("{value:?}");
            }
        }
    }

    #[test]
    fn test_execution_id_propagation_in_nested_spans() {
        let logs = Arc::new(Mutex::new(Vec::new()));
        let layer = TestCapturingLayer { logs: logs.clone() };

        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            let execution_id = "24601";
            let trace_id = "abc123def456";

            let parent_span = info_span!(
                "execute_typescript_workflow",
                execution_id = %execution_id,
                trace_id = %trace_id,
            );

            parent_span.in_scope(|| {
                info!(message = "Starting TypeScript workflow execution");
                let _nested_span = info_span!("workflow_output").entered();
                info!(message = "Step 1: Opening browser");
                info!(message = "Step 2: Navigating to URL");
            });
        });

        let captured = logs.lock().unwrap();
        assert!(
            captured.len() >= 3,
            "Expected at least 3 logs, got {}",
            captured.len()
        );

        for log in captured.iter() {
            assert_eq!(
                log.execution_id.as_deref(),
                Some("24601"),
                "Log '{}' missing execution_id",
                log.message
            );
            assert_eq!(
                log.trace_id.as_deref(),
                Some("abc123def456"),
                "Log '{}' missing trace_id",
                log.message
            );
        }
    }

    #[test]
    fn test_missing_execution_id_without_parent_span() {
        let logs = Arc::new(Mutex::new(Vec::new()));
        let layer = TestCapturingLayer { logs: logs.clone() };

        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            info!(message = "TypeScript workflow log without context");
        });

        let captured = logs.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert!(
            captured[0].execution_id.is_none(),
            "execution_id should be None without parent span"
        );
    }

    #[tokio::test]
    async fn test_async_span_propagation() {
        let logs = Arc::new(Mutex::new(Vec::new()));
        let layer = TestCapturingLayer { logs: logs.clone() };

        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        let execution_id = "24601";
        let trace_id = "async-trace-123";

        let parent_span = info_span!(
            "execute_typescript_workflow",
            execution_id = %execution_id,
            trace_id = %trace_id,
        );

        let handle = tokio::spawn(
            async move {
                info!(message = "Async log 1");
                info!(message = "Async log 2");
            }
            .instrument(parent_span),
        );

        handle.await.unwrap();

        let captured = logs.lock().unwrap();
        assert_eq!(captured.len(), 2, "Expected 2 async logs");

        for log in captured.iter() {
            assert_eq!(
                log.execution_id.as_deref(),
                Some("24601"),
                "Async log '{}' missing execution_id",
                log.message
            );
        }
    }
}
