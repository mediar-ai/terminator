use anyhow::{Context, Result};
use rmcp::model::ProtocolVersion;
use rmcp::transport::TokioChildProcess;
use rmcp::{model::CallToolRequestParam, object, ServiceExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::process::Command;

/// Represents a single workflow step with expected outcomes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub name: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub expected_outcome: ExpectedOutcome,
    pub validation_criteria: Vec<ValidationCriterion>,
    pub timeout_ms: u64,
    pub retry_count: u32,
}

/// Expected outcome of a workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpectedOutcome {
    Success { expected_data: serde_json::Value },
    ElementFound { selector: String },
    TextEntered { field: String, value: String },
    NavigationCompleted { url: String },
    FileProcessed { file_type: String },
    DataExtracted { fields: HashMap<String, String> },
}

/// Validation criteria for measuring accuracy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationCriterion {
    ExactMatch { field: String, expected: String },
    PartialMatch { field: String, contains: String },
    RegexMatch { field: String, pattern: String },
    NumericRange { field: String, min: f64, max: f64 },
    ElementExists { selector: String },
    ElementHasText { selector: String, text: String },
    ResponseTime { max_ms: u64 },
}

/// Complex workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexWorkflow {
    pub name: String,
    pub description: String,
    pub category: WorkflowCategory,
    pub steps: Vec<WorkflowStep>,
    pub test_data: TestData,
    pub accuracy_threshold: f64,
}

/// Categories of workflows to test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowCategory {
    DataEntry,
    FormFilling,
    DocumentProcessing,
    WebNavigation,
    DataExtraction,
    MultiStepProcess,
}

/// Test data for workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestData {
    pub input_files: Vec<String>,
    pub expected_outputs: HashMap<String, serde_json::Value>,
    pub mock_data: HashMap<String, String>,
}

/// Result of a workflow execution
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub workflow_name: String,
    pub total_steps: usize,
    pub successful_steps: usize,
    pub failed_steps: usize,
    pub accuracy_percentage: f64,
    pub execution_time_ms: u64,
    pub step_results: Vec<StepResult>,
    pub error_summary: Vec<String>,
}

/// Result of a single step execution
#[derive(Debug, Serialize, Deserialize)]
pub struct StepResult {
    pub step_name: String,
    pub success: bool,
    pub validation_results: Vec<ValidationResult>,
    pub execution_time_ms: u64,
    pub error: Option<String>,
    pub retry_attempts: u32,
}

/// Result of a validation check
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    pub criterion: String,
    pub passed: bool,
    pub actual_value: Option<String>,
    pub expected_value: Option<String>,
    pub details: Option<String>,
}

/// Main test runner for workflow accuracy
pub struct WorkflowAccuracyTester {
    service: rmcp::Service,
    workflows: Vec<ComplexWorkflow>,
    results: Vec<WorkflowResult>,
}

impl WorkflowAccuracyTester {
    /// Create a new tester with MCP service
    pub async fn new() -> Result<Self> {
        let agent_path = get_agent_binary_path();
        if !agent_path.exists() {
            anyhow::bail!("MCP agent binary not found at {:?}", agent_path);
        }

        let mut cmd = Command::new(&agent_path);
        cmd.args(["-t", "stdio"]);
        let service = ().serve(TokioChildProcess::new(cmd)?).await?;

        Ok(Self {
            service,
            workflows: Vec::new(),
            results: Vec::new(),
        })
    }

    /// Add a workflow to test
    pub fn add_workflow(&mut self, workflow: ComplexWorkflow) {
        self.workflows.push(workflow);
    }

    /// Run all workflows and measure accuracy
    pub async fn run_all_workflows(&mut self) -> Result<AccuracyReport> {
        let start_time = Instant::now();
        let mut total_accuracy = 0.0;

        for workflow in &self.workflows {
            println!("Running workflow: {}", workflow.name);
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

    /// Run a single workflow
    async fn run_workflow(&self, workflow: &ComplexWorkflow) -> Result<WorkflowResult> {
        let start_time = Instant::now();
        let mut step_results = Vec::new();
        let mut successful_steps = 0;
        let mut error_summary = Vec::new();

        for step in &workflow.steps {
            let step_result = self.execute_step(step).await;
            match &step_result {
                Ok(result) => {
                    if result.success {
                        successful_steps += 1;
                    } else if let Some(error) = &result.error {
                        error_summary.push(format!("{}: {}", step.name, error));
                    }
                    step_results.push(result.clone());
                }
                Err(e) => {
                    error_summary.push(format!("{}: {}", step.name, e));
                    step_results.push(StepResult {
                        step_name: step.name.clone(),
                        success: false,
                        validation_results: Vec::new(),
                        execution_time_ms: 0,
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

    /// Execute a single workflow step
    async fn execute_step(&self, step: &WorkflowStep) -> Result<StepResult> {
        let start_time = Instant::now();
        let mut retry_attempts = 0;
        let mut last_error = None;

        while retry_attempts <= step.retry_count {
            match self.call_tool(&step.tool_name, &step.arguments).await {
                Ok(response) => {
                    let validation_results = self
                        .validate_step_outcome(step, &response)
                        .await
                        .unwrap_or_default();

                    let success = validation_results.iter().all(|v| v.passed);

                    return Ok(StepResult {
                        step_name: step.name.clone(),
                        success,
                        validation_results,
                        execution_time_ms: start_time.elapsed().as_millis() as u64,
                        error: if success {
                            None
                        } else {
                            Some("Validation failed".to_string())
                        },
                        retry_attempts,
                    });
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    retry_attempts += 1;
                    if retry_attempts <= step.retry_count {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        }

        Ok(StepResult {
            step_name: step.name.clone(),
            success: false,
            validation_results: Vec::new(),
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            error: last_error,
            retry_attempts,
        })
    }

    /// Call an MCP tool
    async fn call_tool(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let result = self
            .service
            .call_tool(CallToolRequestParam {
                name: tool_name.into(),
                arguments: Some(arguments.clone()),
            })
            .await
            .context("Failed to call MCP tool")?;

        if result.content.is_empty() {
            anyhow::bail!("Empty response from tool");
        }

        let content = &result.content[0];
        let json_str = serde_json::to_string(&content)?;
        let parsed: serde_json::Value = serde_json::from_str(&json_str)?;

        if let Some(text) = parsed.get("text").and_then(|t| t.as_str()) {
            Ok(serde_json::from_str(text)?)
        } else {
            Ok(parsed)
        }
    }

    /// Validate step outcome against criteria
    async fn validate_step_outcome(
        &self,
        step: &WorkflowStep,
        response: &serde_json::Value,
    ) -> Result<Vec<ValidationResult>> {
        let mut results = Vec::new();

        for criterion in &step.validation_criteria {
            let result = match criterion {
                ValidationCriterion::ExactMatch { field, expected } => {
                    self.validate_exact_match(response, field, expected)
                }
                ValidationCriterion::PartialMatch { field, contains } => {
                    self.validate_partial_match(response, field, contains)
                }
                ValidationCriterion::ElementExists { selector } => {
                    self.validate_element_exists(selector).await
                }
                ValidationCriterion::ResponseTime { max_ms } => {
                    self.validate_response_time(*max_ms, 0) // TODO: Pass actual time
                }
                _ => ValidationResult {
                    criterion: format!("{:?}", criterion),
                    passed: false,
                    actual_value: None,
                    expected_value: None,
                    details: Some("Validation not implemented".to_string()),
                },
            };
            results.push(result);
        }

        Ok(results)
    }

    fn validate_exact_match(
        &self,
        response: &serde_json::Value,
        field: &str,
        expected: &str,
    ) -> ValidationResult {
        let actual = response
            .get(field)
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        ValidationResult {
            criterion: format!("ExactMatch({})", field),
            passed: actual == expected,
            actual_value: Some(actual.to_string()),
            expected_value: Some(expected.to_string()),
            details: None,
        }
    }

    fn validate_partial_match(
        &self,
        response: &serde_json::Value,
        field: &str,
        contains: &str,
    ) -> ValidationResult {
        let actual = response
            .get(field)
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        ValidationResult {
            criterion: format!("PartialMatch({})", field),
            passed: actual.contains(contains),
            actual_value: Some(actual.to_string()),
            expected_value: Some(format!("contains '{}'", contains)),
            details: None,
        }
    }

    async fn validate_element_exists(&self, selector: &str) -> ValidationResult {
        match self
            .call_tool(
                "validate_element",
                &object!({
                    "selector": selector,
                    "timeout_ms": 1000
                }),
            )
            .await
        {
            Ok(response) => {
                let exists = response
                    .get("exists")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                ValidationResult {
                    criterion: format!("ElementExists({})", selector),
                    passed: exists,
                    actual_value: Some(exists.to_string()),
                    expected_value: Some("true".to_string()),
                    details: None,
                }
            }
            Err(e) => ValidationResult {
                criterion: format!("ElementExists({})", selector),
                passed: false,
                actual_value: None,
                expected_value: Some("true".to_string()),
                details: Some(e.to_string()),
            },
        }
    }

    fn validate_response_time(&self, max_ms: u64, actual_ms: u64) -> ValidationResult {
        ValidationResult {
            criterion: format!("ResponseTime(<{}ms)", max_ms),
            passed: actual_ms <= max_ms,
            actual_value: Some(format!("{}ms", actual_ms)),
            expected_value: Some(format!("<{}ms", max_ms)),
            details: None,
        }
    }

    /// Cleanup and shutdown
    pub async fn shutdown(self) -> Result<()> {
        self.service.cancel().await?;
        Ok(())
    }
}

/// Overall accuracy report
#[derive(Debug, Serialize, Deserialize)]
pub struct AccuracyReport {
    pub total_workflows: usize,
    pub overall_accuracy_percentage: f64,
    pub execution_time_ms: u64,
    pub workflow_results: Vec<WorkflowResult>,
    pub timestamp: String,
}

/// Helper to get the path to the MCP agent binary
fn get_agent_binary_path() -> PathBuf {
    let mut path = env::current_exe().unwrap();
    path.pop(); // Remove the test binary name
    path.pop(); // Remove 'deps'
    path.push("terminator-mcp-agent");
    #[cfg(target_os = "windows")]
    path.set_extension("exe");
    path
}
