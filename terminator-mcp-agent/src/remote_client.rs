use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::debug;
use uuid::Uuid;

use crate::remote_server::{
    MouseButton, RemoteAction, RemoteRequest, RemoteResponse, WaitCondition,
};

#[derive(Clone)]
pub struct RemoteUIAutomationClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ElementProperties {
    pub name: String,
    pub role: String,
    pub is_enabled: bool,
    pub is_visible: bool,
    pub bounds: Option<serde_json::Value>,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    pub exists: bool,
    pub name: Option<String>,
    pub role: Option<String>,
    pub is_enabled: Option<bool>,
    pub is_visible: Option<bool>,
}

impl RemoteUIAutomationClient {
    pub fn new(base_url: String, api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        })
    }

    async fn send_request(&self, action: RemoteAction) -> Result<serde_json::Value> {
        let request_id = Uuid::new_v4().to_string();

        let request = RemoteRequest {
            action,
            request_id: request_id.clone(),
        };

        let mut url = format!("{}/execute", self.base_url);
        if let Some(api_key) = &self.api_key {
            url.push_str(&format!("?api_key={}", api_key));
        }

        debug!("Sending request to {}: {:?}", url, request);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Request failed with status {}: {}",
                status,
                error_text
            ));
        }

        let remote_response: RemoteResponse = response
            .json()
            .await
            .context("Failed to parse response")?;

        if !remote_response.success {
            return Err(anyhow::anyhow!(
                "Remote operation failed: {}",
                remote_response.error.unwrap_or_else(|| "Unknown error".to_string())
            ));
        }

        remote_response
            .data
            .ok_or_else(|| anyhow::anyhow!("No data in successful response"))
    }

    pub async fn get_window_tree(
        &self,
        pid: Option<u32>,
        include_detailed_attributes: bool,
    ) -> Result<serde_json::Value> {
        self.send_request(RemoteAction::GetWindowTree {
            pid,
            include_detailed_attributes: Some(include_detailed_attributes),
        })
        .await
    }

    pub async fn get_applications(&self) -> Result<Vec<serde_json::Value>> {
        let data = self.send_request(RemoteAction::GetApplications).await?;
        serde_json::from_value(data).context("Failed to parse applications")
    }

    pub async fn click(
        &self,
        selector: &str,
        button: Option<MouseButton>,
    ) -> Result<()> {
        self.send_request(RemoteAction::Click {
            selector: selector.to_string(),
            button,
        })
        .await?;
        Ok(())
    }

    pub async fn type_text(&self, selector: &str, text: &str) -> Result<()> {
        self.send_request(RemoteAction::TypeText {
            selector: selector.to_string(),
            text: text.to_string(),
        })
        .await?;
        Ok(())
    }

    pub async fn press_key(&self, selector: &str, key: &str) -> Result<()> {
        self.send_request(RemoteAction::PressKey {
            selector: selector.to_string(),
            key: key.to_string(),
        })
        .await?;
        Ok(())
    }

    pub async fn get_element_properties(
        &self,
        selector: &str,
    ) -> Result<ElementProperties> {
        let data = self
            .send_request(RemoteAction::GetElementProperties {
                selector: selector.to_string(),
            })
            .await?;

        serde_json::from_value(data).context("Failed to parse element properties")
    }

    pub async fn wait_for_element(
        &self,
        selector: &str,
        condition: WaitCondition,
        timeout_ms: Option<u64>,
    ) -> Result<()> {
        self.send_request(RemoteAction::WaitForElement {
            selector: selector.to_string(),
            condition,
            timeout_ms,
        })
        .await?;
        Ok(())
    }

    pub async fn take_screenshot(
        &self,
        selector: Option<&str>,
        full_page: bool,
    ) -> Result<Vec<u8>> {
        let data = self
            .send_request(RemoteAction::TakeScreenshot {
                selector: selector.map(|s| s.to_string()),
                full_page: Some(full_page),
            })
            .await?;

        let screenshot_data = data
            .get("screenshot")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("No screenshot data in response"))?;

        use base64::{engine::general_purpose::STANDARD, Engine as _};
        STANDARD.decode(screenshot_data).context("Failed to decode screenshot data")
    }

    pub async fn set_value(&self, selector: &str, value: &str) -> Result<()> {
        self.send_request(RemoteAction::SetValue {
            selector: selector.to_string(),
            value: value.to_string(),
        })
        .await?;
        Ok(())
    }

    pub async fn invoke_element(&self, selector: &str) -> Result<()> {
        self.send_request(RemoteAction::InvokeElement {
            selector: selector.to_string(),
        })
        .await?;
        Ok(())
    }

    pub async fn validate_element(&self, selector: &str) -> Result<ValidationResult> {
        let data = self
            .send_request(RemoteAction::ValidateElement {
                selector: selector.to_string(),
            })
            .await?;

        serde_json::from_value(data).context("Failed to parse validation result")
    }

    pub async fn health_check(&self) -> Result<serde_json::Value> {
        let url = format!("{}/health", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send health check")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Health check failed with status {}",
                response.status()
            ));
        }

        response.json().await.context("Failed to parse health check response")
    }
}

pub struct RemoteUIAutomationBuilder {
    base_url: Option<String>,
    api_key: Option<String>,
}

impl RemoteUIAutomationBuilder {
    pub fn new() -> Self {
        Self {
            base_url: None,
            api_key: None,
        }
    }

    pub fn with_url(mut self, url: &str) -> Self {
        self.base_url = Some(url.to_string());
        self
    }

    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.api_key = Some(api_key.to_string());
        self
    }

    pub fn build(self) -> Result<RemoteUIAutomationClient> {
        let base_url = self.base_url.ok_or_else(|| {
            anyhow::anyhow!("Base URL is required")
        })?;

        RemoteUIAutomationClient::new(base_url, self.api_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_pattern() {
        let result = RemoteUIAutomationBuilder::new()
            .with_url("http://localhost:8080")
            .with_api_key("test-key")
            .build();

        assert!(result.is_ok());
        let client = result.unwrap();
        assert_eq!(client.base_url, "http://localhost:8080");
        assert_eq!(client.api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_builder_without_url_fails() {
        let result = RemoteUIAutomationBuilder::new()
            .with_api_key("test-key")
            .build();

        assert!(result.is_err());
    }
}