mod mock_workflow_runner;
mod workflow_accuracy_tests;
mod workflow_definitions;
mod workflows;

use anyhow::Result;
use mock_workflow_runner::MockWorkflowAccuracyTester;
use workflow_definitions::*;
use workflows::*;

#[tokio::test]
async fn test_mock_workflow_accuracy() -> Result<()> {
    // Create mock tester
    let mut tester = MockWorkflowAccuracyTester::new();

    // Add all workflows
    tester.add_workflow(create_pdf_data_entry_workflow());
    tester.add_workflow(create_insurance_quote_workflow());
    tester.add_workflow(create_research_data_collection_workflow());

    // Add new real-world workflows
    tester.add_workflow(create_amazon_shopping_workflow());
    tester.add_workflow(create_dmv_appointment_workflow());
    tester.add_workflow(create_bank_transfer_workflow());
    tester.add_workflow(create_linkedin_job_application_workflow());

    // Run workflows with mock service
    let report = tester.run_all_workflows().await?;

    // Print results
    println!("\n=== Mock Workflow Accuracy Report ===");
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

        // Show step details
        for step in &workflow_result.step_results {
            println!(
                "    - {}: {} ({}ms)",
                step.step_name,
                if step.success { "✓" } else { "✗" },
                step.execution_time_ms
            );
        }
    }

    // The mock should achieve reasonable accuracy based on configured success rates
    assert!(
        report.overall_accuracy_percentage >= 70.0,
        "Mock accuracy too low: {:.2}%",
        report.overall_accuracy_percentage
    );

    Ok(())
}
