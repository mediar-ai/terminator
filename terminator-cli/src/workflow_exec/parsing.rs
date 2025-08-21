/// Extract workflow from wrapper object if it has tool_name: execute_sequence
pub fn extract_workflow_from_wrapper(
    value: &serde_json::Value,
) -> anyhow::Result<Option<serde_json::Value>> {
    if let Some(tool_name) = value.get("tool_name") {
        if tool_name == "execute_sequence" {
            if let Some(arguments) = value.get("arguments") {
                return Ok(Some(arguments.clone()));
            } else {
                return Err(anyhow::anyhow!("Tool call missing 'arguments' field"));
            }
        }
    }
    Ok(None)
}

/// Parse workflow content using robust parsing strategies from gist_executor.rs
pub fn parse_workflow_content(content: &str) -> anyhow::Result<serde_json::Value> {
    // Strategy 1: Try direct JSON workflow
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
        // Check if it's a valid workflow (has steps field)
        if val.get("steps").is_some() {
            return Ok(val);
        }

        // Check if it's a wrapper object
        if let Some(extracted) = extract_workflow_from_wrapper(&val)? {
            return Ok(extracted);
        }
    }

    // Strategy 2: Try direct YAML workflow
    if let Ok(val) = serde_yaml::from_str::<serde_json::Value>(content) {
        // Check if it's a valid workflow (has steps field)
        if val.get("steps").is_some() {
            return Ok(val);
        }

        // Check if it's a wrapper object
        if let Some(extracted) = extract_workflow_from_wrapper(&val)? {
            return Ok(extracted);
        }
    }

    // Strategy 3: Try parsing as JSON wrapper first, then extract
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(extracted) = extract_workflow_from_wrapper(&val)? {
            return Ok(extracted);
        }
    }

    // Strategy 4: Try parsing as YAML wrapper first, then extract
    if let Ok(val) = serde_yaml::from_str::<serde_json::Value>(content) {
        if let Some(extracted) = extract_workflow_from_wrapper(&val)? {
            return Ok(extracted);
        }
    }

    Err(anyhow::anyhow!(
        "Unable to parse content as JSON or YAML workflow or wrapper object. Content must either be:\n\
        1. A workflow with 'steps' field\n\
        2. A wrapper object with tool_name='execute_sequence' and 'arguments' field\n\
        3. Valid JSON or YAML format"
    ))
}
