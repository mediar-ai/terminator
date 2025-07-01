use crate::workflow_accuracy_tests::*;
use rmcp::object;
use std::collections::HashMap;

/// Simple calculator test - actually tests what we can do
pub fn create_calculator_test_workflow() -> ComplexWorkflow {
    ComplexWorkflow {
        name: "Windows Calculator Basic Math".to_string(),
        description: "Test basic calculator operations - simple and verifiable".to_string(),
        category: WorkflowCategory::DataEntry,
        steps: vec![
            WorkflowStep {
                name: "Open Calculator".to_string(),
                tool_name: "open_application".to_string(),
                arguments: object!({
                    "application_name": "uwp:Microsoft.WindowsCalculator",
                    "wait_for_ready": true,
                    "timeout_ms": 5000
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "status": "application_opened" }),
                },
                validation_criteria: vec![],
                timeout_ms: 5000,
                retry_count: 2,
            },
            WorkflowStep {
                name: "Get Window Tree".to_string(),
                tool_name: "get_window_tree".to_string(),
                arguments: object!({
                    "pid": "{{calculator_pid}}",
                    "title": "Calculator"
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "status": "success" }),
                },
                validation_criteria: vec![],
                timeout_ms: 3000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Click Button 5".to_string(),
                tool_name: "click_element".to_string(),
                arguments: object!({
                    "selector": "button|Five",
                    "alternative_selectors": "Name:Five,#num5Button"
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "clicked": true }),
                },
                validation_criteria: vec![],
                timeout_ms: 2000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Click Plus".to_string(),
                tool_name: "click_element".to_string(),
                arguments: object!({
                    "selector": "button|Plus",
                    "alternative_selectors": "Name:Plus,#plusButton"
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "clicked": true }),
                },
                validation_criteria: vec![],
                timeout_ms: 2000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Click Button 3".to_string(),
                tool_name: "click_element".to_string(),
                arguments: object!({
                    "selector": "button|Three",
                    "alternative_selectors": "Name:Three,#num3Button"
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "clicked": true }),
                },
                validation_criteria: vec![],
                timeout_ms: 2000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Click Equals".to_string(),
                tool_name: "click_element".to_string(),
                arguments: object!({
                    "selector": "button|Equals",
                    "alternative_selectors": "Name:Equals,#equalButton"
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "clicked": true }),
                },
                validation_criteria: vec![],
                timeout_ms: 2000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Validate Result".to_string(),
                tool_name: "validate_element".to_string(),
                arguments: object!({
                    "selector": "nativeid:CalculatorResults",
                    "timeout_ms": 2000
                }),
                expected_outcome: ExpectedOutcome::ElementFound {
                    selector: "nativeid:CalculatorResults".to_string(),
                },
                validation_criteria: vec![ValidationCriterion::PartialMatch {
                    field: "text".to_string(),
                    contains: "8".to_string(),
                }],
                timeout_ms: 3000,
                retry_count: 1,
            },
        ],
        test_data: TestData {
            input_files: vec![],
            expected_outputs: HashMap::from([
                ("calculation_correct".to_string(), object!(true)),
                ("result".to_string(), object!("8")),
            ]),
            mock_data: HashMap::new(),
        },
        accuracy_threshold: 90.0,
    }
}

/// Simple Notepad test
pub fn create_notepad_test_workflow() -> ComplexWorkflow {
    ComplexWorkflow {
        name: "Notepad Text Entry".to_string(),
        description: "Test basic text entry in Notepad".to_string(),
        category: WorkflowCategory::DataEntry,
        steps: vec![
            WorkflowStep {
                name: "Open Notepad".to_string(),
                tool_name: "open_application".to_string(),
                arguments: object!({
                    "application_name": "notepad.exe",
                    "wait_for_ready": true,
                    "timeout_ms": 3000
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "status": "application_opened" }),
                },
                validation_criteria: vec![],
                timeout_ms: 3000,
                retry_count: 2,
            },
            WorkflowStep {
                name: "Type Text".to_string(),
                tool_name: "type_into_element".to_string(),
                arguments: object!({
                    "selector": "document|Text Editor",
                    "alternative_selectors": "Name:Text Editor,role:document",
                    "text_to_type": "Hello from MCP accuracy test!",
                    "clear_before_typing": false
                }),
                expected_outcome: ExpectedOutcome::TextEntered {
                    field: "editor".to_string(),
                    value: "Hello from MCP accuracy test!".to_string(),
                },
                validation_criteria: vec![],
                timeout_ms: 3000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Save File".to_string(),
                tool_name: "press_key_global".to_string(),
                arguments: object!({
                    "key": "{Ctrl}s"
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "key_pressed": true }),
                },
                validation_criteria: vec![],
                timeout_ms: 2000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Type Filename".to_string(),
                tool_name: "type_into_element".to_string(),
                arguments: object!({
                    "selector": "edit|File name:",
                    "alternative_selectors": "Name:File name:",
                    "text_to_type": "mcp_test_output.txt",
                    "clear_before_typing": true
                }),
                expected_outcome: ExpectedOutcome::TextEntered {
                    field: "filename".to_string(),
                    value: "mcp_test_output.txt".to_string(),
                },
                validation_criteria: vec![],
                timeout_ms: 3000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Click Save".to_string(),
                tool_name: "click_element".to_string(),
                arguments: object!({
                    "selector": "button|Save",
                    "alternative_selectors": "Name:Save"
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "file_saved": true }),
                },
                validation_criteria: vec![],
                timeout_ms: 2000,
                retry_count: 1,
            },
        ],
        test_data: TestData {
            input_files: vec![],
            expected_outputs: HashMap::from([
                ("file_created".to_string(), object!(true)),
                (
                    "content".to_string(),
                    object!("Hello from MCP accuracy test!"),
                ),
            ]),
            mock_data: HashMap::new(),
        },
        accuracy_threshold: 85.0,
    }
}
