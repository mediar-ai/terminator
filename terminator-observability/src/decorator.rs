//! Decorator pattern implementations for adding observability to Terminator SDK types

use crate::{
    context::ObservabilityContext,
    session::Session,
    telemetry::{SpanBuilder, SpanKind, SpanStatus},
    Result,
};
use std::sync::Arc;
use std::time::Instant;
use terminator::{AutomationError, ClickResult, Desktop, Locator, Selector, UIElement};
use tracing::{info_span, Instrument};

/// Observable wrapper for Desktop
#[derive(Clone)]
pub struct ObservableDesktop {
    inner: Desktop,
    context: Arc<ObservabilityContext>,
    session: Option<Arc<Session>>,
}

impl ObservableDesktop {
    /// Create a new observable desktop
    pub fn new(desktop: Desktop, context: Arc<ObservabilityContext>) -> Self {
        Self {
            inner: desktop,
            context,
            session: None,
        }
    }

    /// Start a new observation session
    pub fn start_session(&mut self, name: impl Into<String>) -> Arc<Session> {
        let session = Arc::new(Session::new(name.into(), self.context.clone()));
        self.session = Some(session.clone());
        session
    }

    /// Get the current session
    pub fn session(&self) -> Option<Arc<Session>> {
        self.session.clone()
    }

    /// Create an observable locator
    pub fn locator(&self, selector: impl Into<Selector>) -> ObservableLocator {
        ObservableLocator::new(
            self.inner.locator(selector),
            self.context.clone(),
            self.session.clone(),
        )
    }

    /// Open an application with observability
    pub async fn open_application(&self, app_name: &str) -> Result<ObservableUIElement> {
        let span = self
            .create_span("open_application")
            .with_kind(SpanKind::Client)
            .with_attribute("app_name", app_name)
            .start();

        let start = Instant::now();
        let result = self.inner.open_application(app_name);
        let duration = start.elapsed();

        match &result {
            Ok(element) => {
                span.set_status(SpanStatus::Ok);
                span.set_attribute("element.role", element.role());
                if let Some(name) = element.name() {
                    span.set_attribute("element.name", name);
                }
                self.record_success("open_application", duration);
            }
            Err(e) => {
                span.set_status(SpanStatus::Error {
                    description: e.to_string(),
                });
                self.record_failure("open_application", duration, e);
            }
        }

        span.end();

        result
            .map(|element| ObservableUIElement::new(element, self.context.clone(), self.session.clone()))
            .map_err(|e| e.into())
    }

    /// Run a command with observability
    pub async fn run_command(
        &self,
        windows_command: Option<&str>,
        unix_command: Option<&str>,
    ) -> Result<terminator::CommandOutput> {
        let span = self
            .create_span("run_command")
            .with_kind(SpanKind::Client)
            .start();

        if let Some(cmd) = windows_command {
            span.set_attribute("command.windows", cmd);
        }
        if let Some(cmd) = unix_command {
            span.set_attribute("command.unix", cmd);
        }

        let start = Instant::now();
        let result = self.inner.run_command(windows_command, unix_command).await;
        let duration = start.elapsed();

        match &result {
            Ok(output) => {
                span.set_status(SpanStatus::Ok);
                span.set_attribute("exit_status", output.exit_status.unwrap_or(-1));
                span.set_attribute("stdout_length", output.stdout.len() as i64);
                span.set_attribute("stderr_length", output.stderr.len() as i64);
                self.record_success("run_command", duration);
            }
            Err(e) => {
                span.set_status(SpanStatus::Error {
                    description: e.to_string(),
                });
                self.record_failure("run_command", duration, e);
            }
        }

        span.end();
        result.map_err(|e| e.into())
    }

    /// Get the inner desktop for direct access
    pub fn inner(&self) -> &Desktop {
        &self.inner
    }

    fn create_span(&self, operation: &str) -> SpanBuilder {
        if let Some(session) = &self.session {
            session.create_span(operation)
        } else {
            self.context.create_span(operation)
        }
    }

    fn record_success(&self, operation: &str, duration: std::time::Duration) {
        self.context.record_metric(
            &format!("terminator.{}.duration", operation),
            duration.as_millis() as f64,
            &[("status", "success")],
        );
        self.context.record_metric(
            &format!("terminator.{}.count", operation),
            1.0,
            &[("status", "success")],
        );
    }

    fn record_failure(&self, operation: &str, duration: std::time::Duration, error: &AutomationError) {
        self.context.record_metric(
            &format!("terminator.{}.duration", operation),
            duration.as_millis() as f64,
            &[("status", "error")],
        );
        self.context.record_metric(
            &format!("terminator.{}.count", operation),
            1.0,
            &[("status", "error"), ("error_type", &format!("{:?}", error))],
        );
    }
}

/// Observable wrapper for Locator
pub struct ObservableLocator {
    inner: Locator,
    context: Arc<ObservabilityContext>,
    session: Option<Arc<Session>>,
}

impl ObservableLocator {
    /// Create a new observable locator
    pub fn new(
        locator: Locator,
        context: Arc<ObservabilityContext>,
        session: Option<Arc<Session>>,
    ) -> Self {
        Self {
            inner: locator,
            context,
            session,
        }
    }

    /// Find the first matching element
    pub async fn first(
        &self,
        timeout: Option<std::time::Duration>,
    ) -> Result<ObservableUIElement> {
        let span = self
            .create_span("locate_element")
            .with_kind(SpanKind::Client)
            .with_attribute("selector", format!("{:?}", self.inner))
            .start();

        if let Some(t) = timeout {
            span.set_attribute("timeout_ms", t.as_millis() as i64);
        }

        let start = Instant::now();
        let result = self.inner.first(timeout).await;
        let duration = start.elapsed();

        match &result {
            Ok(element) => {
                span.set_status(SpanStatus::Ok);
                span.set_attribute("element.role", element.role());
                if let Some(name) = element.name() {
                    span.set_attribute("element.name", name);
                }
                self.record_element_found(duration);
            }
            Err(e) => {
                span.set_status(SpanStatus::Error {
                    description: e.to_string(),
                });
                self.record_element_not_found(duration);
            }
        }

        span.end();

        result
            .map(|element| ObservableUIElement::new(element, self.context.clone(), self.session.clone()))
            .map_err(|e| e.into())
    }

    /// Wait for element to appear
    pub async fn wait(
        &self,
        timeout: Option<std::time::Duration>,
    ) -> Result<ObservableUIElement> {
        let span = self
            .create_span("wait_for_element")
            .with_kind(SpanKind::Client)
            .with_attribute("selector", format!("{:?}", self.inner))
            .start();

        if let Some(t) = timeout {
            span.set_attribute("timeout_ms", t.as_millis() as i64);
        }

        let start = Instant::now();
        let result = self.inner.wait(timeout).await;
        let duration = start.elapsed();

        match &result {
            Ok(element) => {
                span.set_status(SpanStatus::Ok);
                span.set_attribute("wait_duration_ms", duration.as_millis() as i64);
                self.context.record_metric(
                    "terminator.wait_element.duration",
                    duration.as_millis() as f64,
                    &[("status", "found")],
                );
            }
            Err(e) => {
                span.set_status(SpanStatus::Error {
                    description: e.to_string(),
                });
                self.context.record_metric(
                    "terminator.wait_element.duration",
                    duration.as_millis() as f64,
                    &[("status", "timeout")],
                );
            }
        }

        span.end();

        result
            .map(|element| ObservableUIElement::new(element, self.context.clone(), self.session.clone()))
            .map_err(|e| e.into())
    }

    fn create_span(&self, operation: &str) -> SpanBuilder {
        if let Some(session) = &self.session {
            session.create_span(operation)
        } else {
            self.context.create_span(operation)
        }
    }

    fn record_element_found(&self, duration: std::time::Duration) {
        self.context.record_metric(
            "terminator.locate_element.duration",
            duration.as_millis() as f64,
            &[("result", "found")],
        );
    }

    fn record_element_not_found(&self, duration: std::time::Duration) {
        self.context.record_metric(
            "terminator.locate_element.duration",
            duration.as_millis() as f64,
            &[("result", "not_found")],
        );
    }
}

/// Observable wrapper for UIElement
pub struct ObservableUIElement {
    inner: UIElement,
    context: Arc<ObservabilityContext>,
    session: Option<Arc<Session>>,
}

impl ObservableUIElement {
    /// Create a new observable UI element
    pub fn new(
        element: UIElement,
        context: Arc<ObservabilityContext>,
        session: Option<Arc<Session>>,
    ) -> Self {
        Self {
            inner: element,
            context,
            session,
        }
    }

    /// Click the element
    pub async fn click(&self) -> Result<ClickResult> {
        let span = self
            .create_span("click_element")
            .with_kind(SpanKind::Client)
            .with_attribute("element.role", self.inner.role())
            .start();

        if let Some(name) = self.inner.name() {
            span.set_attribute("element.name", name);
        }

        let start = Instant::now();
        let result = self.inner.click();
        let duration = start.elapsed();

        match &result {
            Ok(click_result) => {
                span.set_status(SpanStatus::Ok);
                span.set_attribute("click.method", &click_result.method);
                if let Some((x, y)) = click_result.coordinates {
                    span.set_attribute("click.x", x);
                    span.set_attribute("click.y", y);
                }
                self.record_action_success("click", duration);
            }
            Err(e) => {
                span.set_status(SpanStatus::Error {
                    description: e.to_string(),
                });
                self.record_action_failure("click", duration);
            }
        }

        span.end();
        result.map_err(|e| e.into())
    }

    /// Type text into the element
    pub async fn type_text(&self, text: &str) -> Result<()> {
        let span = self
            .create_span("type_text")
            .with_kind(SpanKind::Client)
            .with_attribute("text_length", text.len() as i64)
            .with_attribute("element.role", self.inner.role())
            .start();

        let start = Instant::now();
        let result = self.inner.type_text(text, false);
        let duration = start.elapsed();

        match &result {
            Ok(_) => {
                span.set_status(SpanStatus::Ok);
                let chars_per_second = if duration.as_secs_f64() > 0.0 {
                    text.len() as f64 / duration.as_secs_f64()
                } else {
                    0.0
                };
                span.set_attribute("chars_per_second", chars_per_second);
                self.record_typing_success(text.len(), duration);
            }
            Err(e) => {
                span.set_status(SpanStatus::Error {
                    description: e.to_string(),
                });
                self.record_action_failure("type_text", duration);
            }
        }

        span.end();
        result.map_err(|e| e.into())
    }

    /// Press a key
    pub async fn press_key(&self, key: &str) -> Result<()> {
        let span = self
            .create_span("press_key")
            .with_kind(SpanKind::Client)
            .with_attribute("key", key)
            .with_attribute("element.role", self.inner.role())
            .start();

        let start = Instant::now();
        let result = self.inner.press_key(key);
        let duration = start.elapsed();

        match &result {
            Ok(_) => {
                span.set_status(SpanStatus::Ok);
                self.record_action_success("press_key", duration);
            }
            Err(e) => {
                span.set_status(SpanStatus::Error {
                    description: e.to_string(),
                });
                self.record_action_failure("press_key", duration);
            }
        }

        span.end();
        result.map_err(|e| e.into())
    }

    /// Get the inner element for direct access
    pub fn inner(&self) -> &UIElement {
        &self.inner
    }

    fn create_span(&self, operation: &str) -> SpanBuilder {
        if let Some(session) = &self.session {
            session.create_span(operation)
        } else {
            self.context.create_span(operation)
        }
    }

    fn record_action_success(&self, action: &str, duration: std::time::Duration) {
        self.context.record_metric(
            &format!("terminator.{}.duration", action),
            duration.as_millis() as f64,
            &[("status", "success")],
        );
    }

    fn record_action_failure(&self, action: &str, duration: std::time::Duration) {
        self.context.record_metric(
            &format!("terminator.{}.duration", action),
            duration.as_millis() as f64,
            &[("status", "error")],
        );
    }

    fn record_typing_success(&self, char_count: usize, duration: std::time::Duration) {
        let chars_per_second = if duration.as_secs_f64() > 0.0 {
            char_count as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        self.context.record_metric(
            "terminator.typing.chars_per_second",
            chars_per_second,
            &[],
        );
        self.context.record_metric(
            "terminator.typing.char_count",
            char_count as f64,
            &[],
        );
    }
}