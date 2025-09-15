use serde_json::Value;

/// Extract cron expression from workflow YAML
pub fn extract_cron_from_workflow(workflow: &Value) -> Option<String> {
    // Primary format: cron field at root level (simpler format)
    if let Some(cron) = workflow.get("cron") {
        if let Some(cron_str) = cron.as_str() {
            return Some(cron_str.to_string());
        }
    }

    // Alternative: GitHub Actions style: on.schedule.cron
    if let Some(on) = workflow.get("on") {
        if let Some(schedule) = on.get("schedule") {
            // Handle both single cron and array of crons
            if let Some(cron_array) = schedule.as_array() {
                // If it's an array, take the first cron expression
                if let Some(first_schedule) = cron_array.first() {
                    if let Some(cron) = first_schedule.get("cron") {
                        if let Some(cron_str) = cron.as_str() {
                            return Some(cron_str.to_string());
                        }
                    }
                }
            } else if let Some(cron) = schedule.get("cron") {
                // Handle single cron expression
                if let Some(cron_str) = cron.as_str() {
                    return Some(cron_str.to_string());
                }
            }
        }
    }

    None
}
