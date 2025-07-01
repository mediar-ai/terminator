mod simple_accuracy_test;
mod workflow_accuracy_tests;

use anyhow::Result;
use simple_accuracy_test::*;
use std::fs;
use std::path::Path;
use workflow_accuracy_tests::{AccuracyReport, WorkflowAccuracyTester};

#[tokio::test]
async fn test_calculator_accuracy() -> Result<()> {
    let mut tester = WorkflowAccuracyTester::new().await?;

    tester.add_workflow(create_calculator_test_workflow());

    let report = tester.run_all_workflows().await?;

    println!("\n=== Calculator Test Results ===");
    println!("Accuracy: {:.2}%", report.overall_accuracy_percentage);

    assert!(
        report.overall_accuracy_percentage >= 80.0,
        "Calculator accuracy too low: {:.2}%",
        report.overall_accuracy_percentage
    );

    save_accuracy_report(&report, "calculator_accuracy.json")?;
    tester.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_notepad_accuracy() -> Result<()> {
    let mut tester = WorkflowAccuracyTester::new().await?;

    tester.add_workflow(create_notepad_test_workflow());

    let report = tester.run_all_workflows().await?;

    println!("\n=== Notepad Test Results ===");
    println!("Accuracy: {:.2}%", report.overall_accuracy_percentage);

    assert!(
        report.overall_accuracy_percentage >= 80.0,
        "Notepad accuracy too low: {:.2}%",
        report.overall_accuracy_percentage
    );

    save_accuracy_report(&report, "notepad_accuracy.json")?;
    tester.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_basic_workflows() -> Result<()> {
    let mut tester = WorkflowAccuracyTester::new().await?;

    // Add only simple, verifiable workflows
    tester.add_workflow(create_calculator_test_workflow());
    tester.add_workflow(create_notepad_test_workflow());

    let report = tester.run_all_workflows().await?;

    // Print simple results
    println!("\n=== MCP Accuracy Test Results ===");
    println!(
        "Overall Accuracy: {:.2}%",
        report.overall_accuracy_percentage
    );
    println!("Total Workflows: {}", report.total_workflows);

    for workflow in &report.workflow_results {
        println!(
            "\n{}: {:.2}% ({}/{})",
            workflow.workflow_name,
            workflow.accuracy_percentage,
            workflow.successful_steps,
            workflow.total_steps
        );

        if !workflow.error_summary.is_empty() {
            println!("  Errors:");
            for error in &workflow.error_summary {
                println!("    - {}", error);
            }
        }
    }

    save_accuracy_report(&report, "basic_accuracy_report.json")?;
    tester.shutdown().await?;
    Ok(())
}

fn save_accuracy_report(report: &AccuracyReport, filename: &str) -> Result<()> {
    let reports_dir = Path::new("target/accuracy_reports");
    fs::create_dir_all(&reports_dir)?;

    let report_path = reports_dir.join(filename);
    let json = serde_json::to_string_pretty(report)?;
    fs::write(report_path, json)?;

    Ok(())
}
