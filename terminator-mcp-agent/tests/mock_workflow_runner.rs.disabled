use crate::workflow_accuracy_tests::*;
use anyhow::Result;
use rand::Rng;
use rmcp::object;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

/// Mock MCP service for testing workflow accuracy without real UI
pub struct MockMcpService {
    /// Simulated success rates for different tools
    tool_success_rates: HashMap<String, f64>,
    /// Simulated response times for tools
    tool_response_times: HashMap<String, u64>,
    /// Mock data store for simulating state
    mock_state: HashMap<String, Value>,
}

impl MockMcpService {
    pub fn new() -> Self {
        let mut tool_success_rates = HashMap::new();

        // Set realistic success rates for different tools
        tool_success_rates.insert("open_application".to_string(), 0.95);
        tool_success_rates.insert("click_element".to_string(), 0.90);
        tool_success_rates.insert("type_into_element".to_string(), 0.92);
        tool_success_rates.insert("navigate_browser".to_string(), 0.98);
        tool_success_rates.insert("extract_text_from_region".to_string(), 0.85);
        tool_success_rates.insert("validate_element".to_string(), 0.88);
        tool_success_rates.insert("execute_sequence".to_string(), 0.87);
        tool_success_rates.insert("select_option".to_string(), 0.91);
        tool_success_rates.insert("wait_for_element".to_string(), 0.89);
        tool_success_rates.insert("extract_element_text".to_string(), 0.86);
        tool_success_rates.insert("switch_to_application".to_string(), 0.96);
        tool_success_rates.insert("send_keys".to_string(), 0.94);

        let mut tool_response_times = HashMap::new();

        // Set realistic response times (ms)
        tool_response_times.insert("open_application".to_string(), 2000);
        tool_response_times.insert("click_element".to_string(), 150);
        tool_response_times.insert("type_into_element".to_string(), 300);
        tool_response_times.insert("navigate_browser".to_string(), 1500);
        tool_response_times.insert("extract_text_from_region".to_string(), 500);
        tool_response_times.insert("validate_element".to_string(), 100);
        tool_response_times.insert("execute_sequence".to_string(), 1000);
        tool_response_times.insert("select_option".to_string(), 200);
        tool_response_times.insert("wait_for_element".to_string(), 500);
        tool_response_times.insert("extract_element_text".to_string(), 300);
        tool_response_times.insert("switch_to_application".to_string(), 100);
        tool_response_times.insert("send_keys".to_string(), 50);

        Self {
            tool_success_rates,
            tool_response_times,
            mock_state: HashMap::new(),
        }
    }

    /// Simulate calling an MCP tool
    pub async fn call_tool(&mut self, tool_name: &str, arguments: &Value) -> Result<Value> {
        // Simulate response time
        let response_time = self
            .tool_response_times
            .get(tool_name)
            .copied()
            .unwrap_or(200);

        let mut rng = rand::thread_rng();
        let actual_time = response_time + rng.gen_range(0..100);
        sleep(Duration::from_millis(actual_time)).await;

        // Check success rate
        let success_rate = self
            .tool_success_rates
            .get(tool_name)
            .copied()
            .unwrap_or(0.85);

        let success = rng.gen_bool(success_rate);

        if !success {
            anyhow::bail!("Tool {} failed (simulated failure)", tool_name);
        }

        // Generate appropriate mock response based on tool
        match tool_name {
            "open_application" => {
                let app_name = arguments
                    .get("application_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");

                self.mock_state
                    .insert(format!("app_{}", app_name), object!({ "opened": true }));

                Ok(object!({
                    "action": "open_application",
                    "status": "success",
                    "application": app_name
                }))
            }

            "click_element" => {
                let selector = arguments
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                Ok(object!({
                    "action": "click_element",
                    "status": "success",
                    "clicked": true,
                    "selector": selector
                }))
            }

            "type_into_element" => {
                let text = arguments
                    .get("text_to_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let selector = arguments
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                self.mock_state
                    .insert(format!("field_{}", selector), object!({ "value": text }));

                Ok(object!({
                    "action": "type_into_element",
                    "status": "success",
                    "typed": text,
                    "selector": selector
                }))
            }

            "navigate_browser" => {
                let url = arguments.get("url").and_then(|v| v.as_str()).unwrap_or("");

                self.mock_state
                    .insert("current_url".to_string(), object!({ "url": url }));

                Ok(object!({
                    "action": "navigate_browser",
                    "status": "success",
                    "navigated_to": url
                }))
            }

            "extract_text_from_region" => {
                // Simulate OCR/text extraction with some realistic data
                let regions = arguments
                    .get("regions")
                    .and_then(|v| v.as_array())
                    .map(|v| v.clone())
                    .unwrap_or_default();

                let mut extracted_data = HashMap::new();

                for region in regions {
                    if let Some(name) = region.get("name").and_then(|v| v.as_str()) {
                        let mock_value = match name {
                            "invoice_number" => "INV-2024-001",
                            "invoice_date" => "2024-01-15",
                            "total_amount" => "$1,234.56",
                            "vendor_name" => "Acme Corp",
                            _ => "Mock Data",
                        };
                        extracted_data.insert(name.to_string(), mock_value.to_string());
                    }
                }

                Ok(object!({
                    "action": "extract_text_from_region",
                    "status": "success",
                    "extracted_data": extracted_data
                }))
            }

            "validate_element" => {
                let selector = arguments
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Simulate element existence based on selector pattern
                let exists = !selector.contains("nonexistent") && rng.gen_bool(0.8);

                Ok(object!({
                    "action": "validate_element",
                    "status": if exists { "success" } else { "failed" },
                    "exists": exists,
                    "selector": selector
                }))
            }

            "execute_sequence" => {
                let tools = arguments
                    .get("tools")
                    .and_then(|v| v.as_array())
                    .map(|v| v.len())
                    .unwrap_or(0);

                let successful = (tools as f64 * success_rate) as usize;

                Ok(object!({
                    "action": "execute_sequence",
                    "status": "success",
                    "total_tools": tools,
                    "executed_tools": tools,
                    "successful_tools": successful,
                    "fields_filled": successful
                }))
            }

            "select_option" => {
                let option = arguments
                    .get("option_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                Ok(object!({
                    "action": "select_option",
                    "status": "success",
                    "selected": option
                }))
            }

            "wait_for_element" => {
                let selector = arguments
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                Ok(object!({
                    "action": "wait_for_element",
                    "status": "success",
                    "found": true,
                    "selector": selector
                }))
            }

            "extract_element_text" => {
                // Simulate extracting text from elements
                let selectors = arguments
                    .get("selectors")
                    .and_then(|v| v.as_array())
                    .map(|v| v.clone())
                    .unwrap_or_default();

                let mut extracted = HashMap::new();

                for selector in selectors {
                    if let Some(sel_str) = selector.as_str() {
                        let mock_text = match sel_str {
                            s if s.contains("Premium") => "$45.00",
                            s if s.contains("Coverage") => "$500,000",
                            s if s.contains("price") => "$175.43",
                            s if s.contains("market-cap") => "$2.7T",
                            _ => "Mock Text",
                        };
                        extracted.insert(sel_str.to_string(), mock_text.to_string());
                    }
                }

                Ok(object!({
                    "action": "extract_element_text",
                    "status": "success",
                    "monthly_premium": "$45.00",
                    "annual_premium": "$540.00",
                    "coverage_amount": "$500,000",
                    "policy_term": "20 years",
                    "company": "Apple Inc.",
                    "price": "$175.43",
                    "market_cap": "$2.7T",
                    "extracted": extracted
                }))
            }

            "switch_to_application" => {
                let app_name = arguments
                    .get("application_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");

                Ok(object!({
                    "action": "switch_to_application",
                    "status": "success",
                    "switched": true,
                    "application": app_name
                }))
            }

            "send_keys" => {
                let keys = arguments.get("keys").and_then(|v| v.as_str()).unwrap_or("");

                Ok(object!({
                    "action": "send_keys",
                    "status": "success",
                    "keys_sent": keys
                }))
            }

            _ => Ok(object!({
                "action": tool_name,
                "status": "success",
                "message": "Mock tool execution"
            })),
        }
    }
}

/// Create a mock workflow accuracy tester for testing without real MCP
pub struct MockWorkflowAccuracyTester {
    mock_service: MockMcpService,
    workflows: Vec<ComplexWorkflow>,
    results: Vec<WorkflowResult>,
}

impl MockWorkflowAccuracyTester {
    pub fn new() -> Self {
        Self {
            mock_service: MockMcpService::new(),
            workflows: Vec::new(),
            results: Vec::new(),
        }
    }

    pub fn add_workflow(&mut self, workflow: ComplexWorkflow) {
        self.workflows.push(workflow);
    }

    pub async fn run_all_workflows(&mut self) -> Result<AccuracyReport> {
        let start_time = std::time::Instant::now();
        let mut total_accuracy = 0.0;

        for workflow in &self.workflows {
            println!("Running mock workflow: {}", workflow.name);
            let result = self.run_workflow(workflow).await?;
            total_accuracy += result.accuracy_percentage;
            self.results.push(result);
        }

        let overall_accuracy = if self.results.is_empty() {
            0.0
        } else {
            total_accuracy / self.results.len() as f64
        };

        Ok(AccuracyReport {
            total_workflows: self.workflows.len(),
            overall_accuracy_percentage: overall_accuracy,
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            workflow_results: self.results.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    async fn run_workflow(&mut self, workflow: &ComplexWorkflow) -> Result<WorkflowResult> {
        let start_time = std::time::Instant::now();
        let mut step_results = Vec::new();
        let mut successful_steps = 0;
        let mut error_summary = Vec::new();

        for step in &workflow.steps {
            let step_start = std::time::Instant::now();

            match self
                .mock_service
                .call_tool(&step.tool_name, &step.arguments)
                .await
            {
                Ok(response) => {
                    // Simple validation - just check if response has expected fields
                    let success = response
                        .get("status")
                        .and_then(|v| v.as_str())
                        .map(|s| s == "success")
                        .unwrap_or(false);

                    if success {
                        successful_steps += 1;
                    }

                    step_results.push(StepResult {
                        step_name: step.name.clone(),
                        success,
                        validation_results: vec![ValidationResult {
                            criterion: "MockValidation".to_string(),
                            passed: success,
                            actual_value: Some(response.to_string()),
                            expected_value: Some("success".to_string()),
                            details: None,
                        }],
                        execution_time_ms: step_start.elapsed().as_millis() as u64,
                        error: if success {
                            None
                        } else {
                            Some("Mock failure".to_string())
                        },
                        retry_attempts: 0,
                    });
                }
                Err(e) => {
                    error_summary.push(format!("{}: {}", step.name, e));
                    step_results.push(StepResult {
                        step_name: step.name.clone(),
                        success: false,
                        validation_results: Vec::new(),
                        execution_time_ms: step_start.elapsed().as_millis() as u64,
                        error: Some(e.to_string()),
                        retry_attempts: 0,
                    });
                }
            }
        }

        let accuracy = if workflow.steps.is_empty() {
            0.0
        } else {
            (successful_steps as f64 / workflow.steps.len() as f64) * 100.0
        };

        Ok(WorkflowResult {
            workflow_name: workflow.name.clone(),
            total_steps: workflow.steps.len(),
            successful_steps,
            failed_steps: workflow.steps.len() - successful_steps,
            accuracy_percentage: accuracy,
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            step_results,
            error_summary,
        })
    }
}
