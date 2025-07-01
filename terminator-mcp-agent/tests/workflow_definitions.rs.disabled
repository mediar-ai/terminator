use crate::workflow_accuracy_tests::*;
use rmcp::object;
use std::collections::HashMap;

/// Create a PDF to form data entry workflow
pub fn create_pdf_data_entry_workflow() -> ComplexWorkflow {
    ComplexWorkflow {
        name: "PDF Invoice to Accounting Form".to_string(),
        description: "Extract data from PDF invoice and enter into accounting software form"
            .to_string(),
        category: WorkflowCategory::DataEntry,
        steps: vec![
            WorkflowStep {
                name: "Open PDF Reader".to_string(),
                tool_name: "open_application".to_string(),
                arguments: object!({
                    "application_name": "Adobe Acrobat Reader",
                    "wait_for_ready": true,
                    "timeout_ms": 5000
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "status": "application_opened" }),
                },
                validation_criteria: vec![ValidationCriterion::ExactMatch {
                    field: "status".to_string(),
                    expected: "success".to_string(),
                }],
                timeout_ms: 5000,
                retry_count: 2,
            },
            WorkflowStep {
                name: "Open Invoice PDF".to_string(),
                tool_name: "execute_sequence".to_string(),
                arguments: object!({
                    "tools": [
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "name:File"
                            }
                        },
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "name:Open"
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:File name",
                                "text_to_type": "C:\\test_data\\invoice_001.pdf",
                                "clear_before_typing": true
                            }
                        },
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "name:Open"
                            }
                        }
                    ],
                    "stop_on_error": false,
                    "include_detailed_results": true
                }),
                expected_outcome: ExpectedOutcome::FileProcessed {
                    file_type: "PDF".to_string(),
                },
                validation_criteria: vec![ValidationCriterion::PartialMatch {
                    field: "status".to_string(),
                    contains: "success".to_string(),
                }],
                timeout_ms: 10000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Extract Invoice Data".to_string(),
                tool_name: "extract_text_from_region".to_string(),
                arguments: object!({
                    "regions": [
                        {
                            "name": "invoice_number",
                            "selector": "region:100,50,300,100"
                        },
                        {
                            "name": "invoice_date",
                            "selector": "region:100,120,300,170"
                        },
                        {
                            "name": "total_amount",
                            "selector": "region:400,500,600,550"
                        },
                        {
                            "name": "vendor_name",
                            "selector": "region:100,200,400,250"
                        }
                    ],
                    "use_ocr_if_needed": true
                }),
                expected_outcome: ExpectedOutcome::DataExtracted {
                    fields: HashMap::from([
                        ("invoice_number".to_string(), "INV-2024-001".to_string()),
                        ("invoice_date".to_string(), "2024-01-15".to_string()),
                        ("total_amount".to_string(), "$1,234.56".to_string()),
                        ("vendor_name".to_string(), "Acme Corp".to_string()),
                    ]),
                },
                validation_criteria: vec![
                    ValidationCriterion::RegexMatch {
                        field: "invoice_number".to_string(),
                        pattern: r"INV-\d{4}-\d{3}".to_string(),
                    },
                    ValidationCriterion::RegexMatch {
                        field: "total_amount".to_string(),
                        pattern: r"\$[\d,]+\.\d{2}".to_string(),
                    },
                ],
                timeout_ms: 5000,
                retry_count: 2,
            },
            WorkflowStep {
                name: "Open Accounting Software".to_string(),
                tool_name: "open_application".to_string(),
                arguments: object!({
                    "application_name": "QuickBooks",
                    "wait_for_ready": true,
                    "timeout_ms": 8000
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "status": "application_opened" }),
                },
                validation_criteria: vec![ValidationCriterion::ElementExists {
                    selector: "name:QuickBooks".to_string(),
                }],
                timeout_ms: 8000,
                retry_count: 2,
            },
            WorkflowStep {
                name: "Navigate to Invoice Entry".to_string(),
                tool_name: "execute_sequence".to_string(),
                arguments: object!({
                    "tools": [
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "name:Vendors"
                            }
                        },
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "name:Enter Bills"
                            }
                        }
                    ],
                    "delay_between_tools_ms": 500
                }),
                expected_outcome: ExpectedOutcome::NavigationCompleted {
                    url: "bills_entry".to_string(),
                },
                validation_criteria: vec![ValidationCriterion::ElementExists {
                    selector: "name:Bill #".to_string(),
                }],
                timeout_ms: 5000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Fill Invoice Form".to_string(),
                tool_name: "execute_sequence".to_string(),
                arguments: object!({
                    "tools": [
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:Vendor",
                                "text_to_type": "{{vendor_name}}",
                                "clear_before_typing": true
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:Bill #",
                                "text_to_type": "{{invoice_number}}",
                                "clear_before_typing": true
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:Date",
                                "text_to_type": "{{invoice_date}}",
                                "clear_before_typing": true
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:Amount Due",
                                "text_to_type": "{{total_amount}}",
                                "clear_before_typing": true
                            }
                        }
                    ],
                    "stop_on_error": false,
                    "include_detailed_results": true
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "fields_filled": 4 }),
                },
                validation_criteria: vec![ValidationCriterion::ElementHasText {
                    selector: "name:Bill #".to_string(),
                    text: "INV-2024-001".to_string(),
                }],
                timeout_ms: 10000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Save Invoice".to_string(),
                tool_name: "click_element".to_string(),
                arguments: object!({
                    "selector": "name:Save & Close",
                    "alternative_selectors": "name:Save,button|Save"
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "saved": true }),
                },
                validation_criteria: vec![ValidationCriterion::ResponseTime { max_ms: 3000 }],
                timeout_ms: 5000,
                retry_count: 2,
            },
        ],
        test_data: TestData {
            input_files: vec!["invoice_001.pdf".to_string()],
            expected_outputs: HashMap::from([
                ("invoice_saved".to_string(), object!(true)),
                (
                    "extracted_fields".to_string(),
                    object!({
                        "invoice_number": "INV-2024-001",
                        "invoice_date": "2024-01-15",
                        "total_amount": "$1,234.56",
                        "vendor_name": "Acme Corp"
                    }),
                ),
            ]),
            mock_data: HashMap::new(),
        },
        accuracy_threshold: 85.0,
    }
}

/// Create a complex multi-application workflow
pub fn create_insurance_quote_workflow() -> ComplexWorkflow {
    ComplexWorkflow {
        name: "Insurance Quote Generation".to_string(),
        description:
            "Navigate insurance website, fill complex form with validation, generate quote"
                .to_string(),
        category: WorkflowCategory::MultiStepProcess,
        steps: vec![
            WorkflowStep {
                name: "Open Browser".to_string(),
                tool_name: "open_application".to_string(),
                arguments: object!({
                    "application_name": "Google Chrome",
                    "wait_for_ready": true,
                    "timeout_ms": 5000
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "status": "browser_opened" }),
                },
                validation_criteria: vec![ValidationCriterion::ElementExists {
                    selector: "name:Address and search bar".to_string(),
                }],
                timeout_ms: 5000,
                retry_count: 2,
            },
            WorkflowStep {
                name: "Navigate to Insurance Site".to_string(),
                tool_name: "navigate_browser".to_string(),
                arguments: object!({
                    "url": "https://demo-insurance.example.com/quote",
                    "wait_for_load": true
                }),
                expected_outcome: ExpectedOutcome::NavigationCompleted {
                    url: "https://demo-insurance.example.com/quote".to_string(),
                },
                validation_criteria: vec![ValidationCriterion::ElementExists {
                    selector: "name:Get Your Quote".to_string(),
                }],
                timeout_ms: 8000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Accept Cookies".to_string(),
                tool_name: "click_element".to_string(),
                arguments: object!({
                    "selector": "name:Accept All Cookies",
                    "alternative_selectors": "button|Accept,name:Accept",
                    "optional": true
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "clicked": true }),
                },
                validation_criteria: vec![],
                timeout_ms: 2000,
                retry_count: 0,
            },
            WorkflowStep {
                name: "Fill Personal Information".to_string(),
                tool_name: "execute_sequence".to_string(),
                arguments: object!({
                    "tools": [
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:First Name",
                                "text_to_type": "John",
                                "clear_before_typing": true
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:Last Name",
                                "text_to_type": "Doe",
                                "clear_before_typing": true
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:Date of Birth",
                                "text_to_type": "01/15/1980",
                                "clear_before_typing": true
                            }
                        },
                        {
                            "tool_name": "select_option",
                            "arguments": {
                                "selector": "name:Gender",
                                "option_name": "Male"
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:Email",
                                "text_to_type": "john.doe@example.com",
                                "clear_before_typing": true
                            }
                        }
                    ],
                    "delay_between_tools_ms": 200,
                    "stop_on_error": false
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "fields_filled": 5 }),
                },
                validation_criteria: vec![ValidationCriterion::ElementHasText {
                    selector: "name:Email".to_string(),
                    text: "john.doe@example.com".to_string(),
                }],
                timeout_ms: 15000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Fill Health Information".to_string(),
                tool_name: "execute_sequence".to_string(),
                arguments: object!({
                    "tools": [
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:Height",
                                "text_to_type": "5'10\"",
                                "clear_before_typing": true
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:Weight",
                                "text_to_type": "175",
                                "clear_before_typing": true
                            }
                        },
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "name:Non-smoker",
                                "element_type": "radio"
                            }
                        },
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "name:No pre-existing conditions",
                                "element_type": "checkbox"
                            }
                        }
                    ],
                    "delay_between_tools_ms": 300
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "health_info_complete": true }),
                },
                validation_criteria: vec![ValidationCriterion::ElementHasText {
                    selector: "name:Weight".to_string(),
                    text: "175".to_string(),
                }],
                timeout_ms: 10000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Select Coverage Type".to_string(),
                tool_name: "execute_sequence".to_string(),
                arguments: object!({
                    "tools": [
                        {
                            "tool_name": "select_option",
                            "arguments": {
                                "selector": "name:Coverage Type",
                                "option_name": "Term Life Insurance"
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:Coverage Amount",
                                "text_to_type": "500000",
                                "clear_before_typing": true
                            }
                        },
                        {
                            "tool_name": "select_option",
                            "arguments": {
                                "selector": "name:Term Length",
                                "option_name": "20 years"
                            }
                        }
                    ],
                    "delay_between_tools_ms": 200
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "coverage_selected": true }),
                },
                validation_criteria: vec![ValidationCriterion::NumericRange {
                    field: "coverage_amount".to_string(),
                    min: 100000.0,
                    max: 1000000.0,
                }],
                timeout_ms: 8000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Submit for Quote".to_string(),
                tool_name: "click_element".to_string(),
                arguments: object!({
                    "selector": "name:Get My Quote",
                    "alternative_selectors": "button|Calculate Quote,name:Submit"
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "quote_requested": true }),
                },
                validation_criteria: vec![ValidationCriterion::ResponseTime { max_ms: 5000 }],
                timeout_ms: 5000,
                retry_count: 2,
            },
            WorkflowStep {
                name: "Wait for Quote".to_string(),
                tool_name: "wait_for_element".to_string(),
                arguments: object!({
                    "selector": "name:Your Quote",
                    "timeout_ms": 10000
                }),
                expected_outcome: ExpectedOutcome::ElementFound {
                    selector: "name:Your Quote".to_string(),
                },
                validation_criteria: vec![ValidationCriterion::ElementExists {
                    selector: "name:Monthly Premium".to_string(),
                }],
                timeout_ms: 10000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Extract Quote Details".to_string(),
                tool_name: "extract_element_text".to_string(),
                arguments: object!({
                    "selectors": [
                        "name:Monthly Premium",
                        "name:Annual Premium",
                        "name:Coverage Amount",
                        "name:Policy Term"
                    ]
                }),
                expected_outcome: ExpectedOutcome::DataExtracted {
                    fields: HashMap::from([
                        ("monthly_premium".to_string(), "$45.00".to_string()),
                        ("annual_premium".to_string(), "$540.00".to_string()),
                        ("coverage_amount".to_string(), "$500,000".to_string()),
                        ("policy_term".to_string(), "20 years".to_string()),
                    ]),
                },
                validation_criteria: vec![ValidationCriterion::RegexMatch {
                    field: "monthly_premium".to_string(),
                    pattern: r"\$\d+\.\d{2}".to_string(),
                }],
                timeout_ms: 5000,
                retry_count: 1,
            },
        ],
        test_data: TestData {
            input_files: vec![],
            expected_outputs: HashMap::from([
                ("quote_generated".to_string(), object!(true)),
                (
                    "quote_data".to_string(),
                    object!({
                        "monthly_premium": "$45.00",
                        "coverage_amount": "$500,000",
                        "policy_term": "20 years"
                    }),
                ),
            ]),
            mock_data: HashMap::from([
                ("first_name".to_string(), "John".to_string()),
                ("last_name".to_string(), "Doe".to_string()),
                ("email".to_string(), "john.doe@example.com".to_string()),
            ]),
        },
        accuracy_threshold: 90.0,
    }
}

/// Create a data extraction workflow from multiple sources
pub fn create_research_data_collection_workflow() -> ComplexWorkflow {
    ComplexWorkflow {
        name: "Research Data Collection".to_string(),
        description: "Collect data from multiple websites and compile into spreadsheet".to_string(),
        category: WorkflowCategory::DataExtraction,
        steps: vec![
            WorkflowStep {
                name: "Open Spreadsheet".to_string(),
                tool_name: "open_application".to_string(),
                arguments: object!({
                    "application_name": "Microsoft Excel",
                    "wait_for_ready": true,
                    "timeout_ms": 5000
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "status": "excel_opened" }),
                },
                validation_criteria: vec![ValidationCriterion::ElementExists {
                    selector: "name:Microsoft Excel".to_string(),
                }],
                timeout_ms: 5000,
                retry_count: 2,
            },
            WorkflowStep {
                name: "Create Headers".to_string(),
                tool_name: "execute_sequence".to_string(),
                arguments: object!({
                    "tools": [
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "name:A1"
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "focused",
                                "text_to_type": "Company Name"
                            }
                        },
                        {
                            "tool_name": "send_keys",
                            "arguments": {
                                "keys": "Tab"
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "focused",
                                "text_to_type": "Stock Price"
                            }
                        },
                        {
                            "tool_name": "send_keys",
                            "arguments": {
                                "keys": "Tab"
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "focused",
                                "text_to_type": "Market Cap"
                            }
                        }
                    ],
                    "delay_between_tools_ms": 100
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "headers_created": 3 }),
                },
                validation_criteria: vec![],
                timeout_ms: 8000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Open Browser for Research".to_string(),
                tool_name: "open_application".to_string(),
                arguments: object!({
                    "application_name": "Google Chrome",
                    "wait_for_ready": true,
                    "timeout_ms": 5000
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "status": "browser_opened" }),
                },
                validation_criteria: vec![],
                timeout_ms: 5000,
                retry_count: 2,
            },
            WorkflowStep {
                name: "Search Company Data".to_string(),
                tool_name: "execute_sequence".to_string(),
                arguments: object!({
                    "tools": [
                        {
                            "tool_name": "navigate_browser",
                            "arguments": {
                                "url": "https://finance.yahoo.com",
                                "wait_for_load": true
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:Search",
                                "text_to_type": "AAPL",
                                "clear_before_typing": true
                            }
                        },
                        {
                            "tool_name": "send_keys",
                            "arguments": {
                                "keys": "Enter"
                            }
                        }
                    ],
                    "delay_between_tools_ms": 500
                }),
                expected_outcome: ExpectedOutcome::NavigationCompleted {
                    url: "quote".to_string(),
                },
                validation_criteria: vec![],
                timeout_ms: 10000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Extract Stock Data".to_string(),
                tool_name: "extract_element_text".to_string(),
                arguments: object!({
                    "selectors": [
                        "selector:data-symbol=AAPL",
                        "selector:.quote-price",
                        "selector:.market-cap"
                    ],
                    "wait_for_elements": true
                }),
                expected_outcome: ExpectedOutcome::DataExtracted {
                    fields: HashMap::from([
                        ("company".to_string(), "Apple Inc.".to_string()),
                        ("price".to_string(), "$175.43".to_string()),
                        ("market_cap".to_string(), "$2.7T".to_string()),
                    ]),
                },
                validation_criteria: vec![ValidationCriterion::RegexMatch {
                    field: "price".to_string(),
                    pattern: r"\$\d+\.\d{2}".to_string(),
                }],
                timeout_ms: 5000,
                retry_count: 2,
            },
            WorkflowStep {
                name: "Switch to Excel".to_string(),
                tool_name: "switch_to_application".to_string(),
                arguments: object!({
                    "application_name": "Microsoft Excel"
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "switched": true }),
                },
                validation_criteria: vec![],
                timeout_ms: 2000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Enter Data in Spreadsheet".to_string(),
                tool_name: "execute_sequence".to_string(),
                arguments: object!({
                    "tools": [
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "name:A2"
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "focused",
                                "text_to_type": "{{company}}"
                            }
                        },
                        {
                            "tool_name": "send_keys",
                            "arguments": {
                                "keys": "Tab"
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "focused",
                                "text_to_type": "{{price}}"
                            }
                        },
                        {
                            "tool_name": "send_keys",
                            "arguments": {
                                "keys": "Tab"
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "focused",
                                "text_to_type": "{{market_cap}}"
                            }
                        }
                    ],
                    "delay_between_tools_ms": 100
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "data_entered": true }),
                },
                validation_criteria: vec![],
                timeout_ms: 8000,
                retry_count: 1,
            },
            WorkflowStep {
                name: "Save Spreadsheet".to_string(),
                tool_name: "execute_sequence".to_string(),
                arguments: object!({
                    "tools": [
                        {
                            "tool_name": "send_keys",
                            "arguments": {
                                "keys": "Ctrl+S"
                            }
                        },
                        {
                            "tool_name": "type_into_element",
                            "arguments": {
                                "selector": "name:File name",
                                "text_to_type": "stock_research_data.xlsx",
                                "clear_before_typing": true
                            }
                        },
                        {
                            "tool_name": "click_element",
                            "arguments": {
                                "selector": "name:Save"
                            }
                        }
                    ],
                    "delay_between_tools_ms": 300
                }),
                expected_outcome: ExpectedOutcome::Success {
                    expected_data: object!({ "file_saved": true }),
                },
                validation_criteria: vec![ValidationCriterion::ResponseTime { max_ms: 3000 }],
                timeout_ms: 5000,
                retry_count: 2,
            },
        ],
        test_data: TestData {
            input_files: vec![],
            expected_outputs: HashMap::from([
                ("spreadsheet_created".to_string(), object!(true)),
                (
                    "data_collected".to_string(),
                    object!({
                        "companies": ["Apple Inc."],
                        "data_points": 3
                    }),
                ),
            ]),
            mock_data: HashMap::new(),
        },
        accuracy_threshold: 80.0,
    }
}
