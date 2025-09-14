/// Validate workflow structure to provide early error detection
pub fn validate_workflow(workflow: &serde_json::Value) -> anyhow::Result<()> {
    // Check that it's an object
    let obj = workflow
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Workflow must be a JSON object"))?;

    // Check that steps exists and is an array
    let steps = obj
        .get("steps")
        .ok_or_else(|| anyhow::anyhow!("Workflow must contain a 'steps' field"))?;

    let steps_array = steps
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("'steps' field must be an array"))?;

    if steps_array.is_empty() {
        return Err(anyhow::anyhow!("Workflow must contain at least one step"));
    }

    // Validate each step
    for (i, step) in steps_array.iter().enumerate() {
        let step_obj = step
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Step {} must be an object", i))?;

        let has_tool_name = step_obj.contains_key("tool_name");
        let has_group_name = step_obj.contains_key("group_name");

        if !has_tool_name && !has_group_name {
            return Err(anyhow::anyhow!(
                "Step {} must have either 'tool_name' or 'group_name'",
                i
            ));
        }

        if has_tool_name && has_group_name {
            return Err(anyhow::anyhow!(
                "Step {} cannot have both 'tool_name' and 'group_name'",
                i
            ));
        }
    }

    // Validate variables if present
    if let Some(variables) = obj.get("variables") {
        if let Some(vars_obj) = variables.as_object() {
            for (name, def) in vars_obj {
                if name.is_empty() {
                    return Err(anyhow::anyhow!("Variable name cannot be empty"));
                }

                if let Some(def_obj) = def.as_object() {
                    // Ensure label exists and is non-empty
                    if let Some(label) = def_obj.get("label") {
                        if let Some(label_str) = label.as_str() {
                            if label_str.is_empty() {
                                return Err(anyhow::anyhow!(
                                    "Variable '{}' must have a non-empty label",
                                    name
                                ));
                            }
                        }
                    } else {
                        return Err(anyhow::anyhow!(
                            "Variable '{}' must have a 'label' field",
                            name
                        ));
                    }

                    // --------------------- NEW VALIDATION ---------------------
                    // Enforce `required` property logic
                    let is_required = def_obj
                        .get("required")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);

                    if is_required {
                        // Check for default value in definition
                        let has_default = def_obj.contains_key("default");

                        // Check if inputs provide a value for this variable
                        let input_has_value = obj
                            .get("inputs")
                            .and_then(|v| v.as_object())
                            .map(|inputs_obj| inputs_obj.contains_key(name))
                            .unwrap_or(false);

                        if !has_default && !input_has_value {
                            return Err(anyhow::anyhow!(
                                "Required variable '{}' is missing and has no default value",
                                name
                            ));
                        }
                    }
                    // ----------------------------------------------------------------
                }
            }
        }
    }

    Ok(())
}
