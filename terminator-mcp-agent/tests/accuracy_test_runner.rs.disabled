mod workflow_accuracy_tests;
mod workflow_definitions;
mod workflows;

use anyhow::Result;
use std::fs;
use std::path::Path;
use workflow_accuracy_tests::{AccuracyReport, WorkflowAccuracyTester};
use workflow_definitions::*;
use workflows::*;

#[tokio::test]
async fn test_pdf_data_entry_accuracy() -> Result<()> {
    let mut tester = WorkflowAccuracyTester::new().await?;

    // Add PDF data entry workflow
    tester.add_workflow(create_pdf_data_entry_workflow());

    // Run and measure accuracy
    let report = tester.run_all_workflows().await?;

    // Assert minimum accuracy threshold
    assert!(
        report.overall_accuracy_percentage >= 85.0,
        "PDF data entry accuracy too low: {:.2}%",
        report.overall_accuracy_percentage
    );

    // Save report
    save_accuracy_report(&report, "pdf_data_entry_accuracy_report.json")?;

    tester.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_insurance_quote_accuracy() -> Result<()> {
    let mut tester = WorkflowAccuracyTester::new().await?;

    // Add insurance quote workflow
    tester.add_workflow(create_insurance_quote_workflow());

    // Run and measure accuracy
    let report = tester.run_all_workflows().await?;

    // Assert minimum accuracy threshold
    assert!(
        report.overall_accuracy_percentage >= 90.0,
        "Insurance quote accuracy too low: {:.2}%",
        report.overall_accuracy_percentage
    );

    // Save report
    save_accuracy_report(&report, "insurance_quote_accuracy_report.json")?;

    tester.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_data_collection_accuracy() -> Result<()> {
    let mut tester = WorkflowAccuracyTester::new().await?;

    // Add research data collection workflow
    tester.add_workflow(create_research_data_collection_workflow());

    // Run and measure accuracy
    let report = tester.run_all_workflows().await?;

    // Assert minimum accuracy threshold
    assert!(
        report.overall_accuracy_percentage >= 80.0,
        "Data collection accuracy too low: {:.2}%",
        report.overall_accuracy_percentage
    );

    // Save report
    save_accuracy_report(&report, "data_collection_accuracy_report.json")?;

    tester.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_all_workflows_accuracy() -> Result<()> {
    let mut tester = WorkflowAccuracyTester::new().await?;

    // Add original workflows
    tester.add_workflow(create_pdf_data_entry_workflow());
    tester.add_workflow(create_insurance_quote_workflow());
    tester.add_workflow(create_research_data_collection_workflow());

    // Add e-commerce workflows
    tester.add_workflow(create_amazon_shopping_workflow());
    tester.add_workflow(create_ebay_auction_workflow());

    // Add government workflows
    tester.add_workflow(create_dmv_appointment_workflow());
    tester.add_workflow(create_irs_tax_form_workflow());

    // Add banking workflows
    tester.add_workflow(create_bank_transfer_workflow());
    tester.add_workflow(create_credit_card_application_workflow());

    // Add social media workflows
    tester.add_workflow(create_linkedin_job_application_workflow());
    tester.add_workflow(create_twitter_posting_workflow());

    // Run all workflows and measure overall accuracy
    let report = tester.run_all_workflows().await?;

    // Print detailed results
    println!("\n=== Workflow Accuracy Report ===");
    println!(
        "Overall Accuracy: {:.2}%",
        report.overall_accuracy_percentage
    );
    println!("Total Workflows: {}", report.total_workflows);
    println!("Execution Time: {}ms", report.execution_time_ms);

    for workflow_result in &report.workflow_results {
        println!("\n--- {} ---", workflow_result.workflow_name);
        println!("  Accuracy: {:.2}%", workflow_result.accuracy_percentage);
        println!(
            "  Steps: {}/{} successful",
            workflow_result.successful_steps, workflow_result.total_steps
        );
        println!("  Time: {}ms", workflow_result.execution_time_ms);

        if !workflow_result.error_summary.is_empty() {
            println!("  Errors:");
            for error in &workflow_result.error_summary {
                println!("    - {}", error);
            }
        }
    }

    // Assert overall minimum accuracy
    assert!(
        report.overall_accuracy_percentage >= 85.0,
        "Overall accuracy too low: {:.2}%",
        report.overall_accuracy_percentage
    );

    // Save comprehensive report
    save_accuracy_report(&report, "comprehensive_accuracy_report.json")?;

    // Generate markdown report
    generate_markdown_report(&report)?;

    tester.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_ecommerce_workflows_accuracy() -> Result<()> {
    let mut tester = WorkflowAccuracyTester::new().await?;

    // Add e-commerce workflows
    tester.add_workflow(create_amazon_shopping_workflow());
    tester.add_workflow(create_ebay_auction_workflow());

    // Run and measure accuracy
    let report = tester.run_all_workflows().await?;

    // Assert minimum accuracy threshold
    assert!(
        report.overall_accuracy_percentage >= 75.0,
        "E-commerce workflows accuracy too low: {:.2}%",
        report.overall_accuracy_percentage
    );

    // Save report
    save_accuracy_report(&report, "ecommerce_accuracy_report.json")?;

    tester.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_government_workflows_accuracy() -> Result<()> {
    let mut tester = WorkflowAccuracyTester::new().await?;

    // Add government workflows
    tester.add_workflow(create_dmv_appointment_workflow());
    tester.add_workflow(create_irs_tax_form_workflow());

    // Run and measure accuracy
    let report = tester.run_all_workflows().await?;

    // Assert minimum accuracy threshold
    assert!(
        report.overall_accuracy_percentage >= 80.0,
        "Government workflows accuracy too low: {:.2}%",
        report.overall_accuracy_percentage
    );

    // Save report
    save_accuracy_report(&report, "government_accuracy_report.json")?;

    tester.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_banking_workflows_accuracy() -> Result<()> {
    let mut tester = WorkflowAccuracyTester::new().await?;

    // Add banking workflows
    tester.add_workflow(create_bank_transfer_workflow());
    tester.add_workflow(create_credit_card_application_workflow());

    // Run and measure accuracy
    let report = tester.run_all_workflows().await?;

    // Assert minimum accuracy threshold - banking needs high accuracy
    assert!(
        report.overall_accuracy_percentage >= 85.0,
        "Banking workflows accuracy too low: {:.2}%",
        report.overall_accuracy_percentage
    );

    // Save report
    save_accuracy_report(&report, "banking_accuracy_report.json")?;

    tester.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_social_media_workflows_accuracy() -> Result<()> {
    let mut tester = WorkflowAccuracyTester::new().await?;

    // Add social media workflows
    tester.add_workflow(create_linkedin_job_application_workflow());
    tester.add_workflow(create_twitter_posting_workflow());

    // Run and measure accuracy
    let report = tester.run_all_workflows().await?;

    // Assert minimum accuracy threshold
    assert!(
        report.overall_accuracy_percentage >= 80.0,
        "Social media workflows accuracy too low: {:.2}%",
        report.overall_accuracy_percentage
    );

    // Save report
    save_accuracy_report(&report, "social_media_accuracy_report.json")?;

    tester.shutdown().await?;
    Ok(())
}

/// Save accuracy report to JSON file
fn save_accuracy_report(report: &AccuracyReport, filename: &str) -> Result<()> {
    let reports_dir = Path::new("target/accuracy_reports");
    fs::create_dir_all(&reports_dir)?;

    let report_path = reports_dir.join(filename);
    let json = serde_json::to_string_pretty(report)?;
    fs::write(report_path, json)?;

    Ok(())
}

/// Generate a markdown report for easy viewing
fn generate_markdown_report(report: &AccuracyReport) -> Result<()> {
    let reports_dir = Path::new("target/accuracy_reports");
    fs::create_dir_all(&reports_dir)?;

    let mut markdown = String::new();

    // Header
    markdown.push_str("# MCP Workflow Accuracy Report\n\n");
    markdown.push_str(&format!("**Generated:** {}\n\n", report.timestamp));
    markdown.push_str(&format!(
        "**Overall Accuracy:** {:.2}%\n\n",
        report.overall_accuracy_percentage
    ));
    markdown.push_str(&format!(
        "**Total Workflows:** {}\n\n",
        report.total_workflows
    ));
    markdown.push_str(&format!(
        "**Total Execution Time:** {}ms\n\n",
        report.execution_time_ms
    ));

    // Summary table
    markdown.push_str("## Workflow Summary\n\n");
    markdown.push_str("| Workflow | Accuracy | Success/Total | Time (ms) |\n");
    markdown.push_str("|----------|----------|---------------|----------|\n");

    for workflow in &report.workflow_results {
        markdown.push_str(&format!(
            "| {} | {:.2}% | {}/{} | {} |\n",
            workflow.workflow_name,
            workflow.accuracy_percentage,
            workflow.successful_steps,
            workflow.total_steps,
            workflow.execution_time_ms
        ));
    }

    // Detailed results
    markdown.push_str("\n## Detailed Results\n\n");

    for workflow in &report.workflow_results {
        markdown.push_str(&format!("### {}\n\n", workflow.workflow_name));
        markdown.push_str(&format!(
            "**Accuracy:** {:.2}%\n\n",
            workflow.accuracy_percentage
        ));

        if !workflow.error_summary.is_empty() {
            markdown.push_str("**Errors:**\n");
            for error in &workflow.error_summary {
                markdown.push_str(&format!("- {}\n", error));
            }
            markdown.push_str("\n");
        }

        // Step details
        markdown.push_str("**Step Results:**\n\n");
        markdown.push_str("| Step | Success | Validation Results | Time (ms) |\n");
        markdown.push_str("|------|---------|-------------------|----------|\n");

        for step in &workflow.step_results {
            let validation_summary = if step.validation_results.is_empty() {
                "N/A".to_string()
            } else {
                let passed = step.validation_results.iter().filter(|v| v.passed).count();
                format!("{}/{} passed", passed, step.validation_results.len())
            };

            markdown.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                step.step_name,
                if step.success { "✓" } else { "✗" },
                validation_summary,
                step.execution_time_ms
            ));
        }

        markdown.push_str("\n");
    }

    let report_path = reports_dir.join("accuracy_report.md");
    fs::write(report_path, markdown)?;

    Ok(())
}
