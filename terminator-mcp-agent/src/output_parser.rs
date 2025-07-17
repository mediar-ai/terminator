use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

// Crate name uses an underscore when referenced from Rust code.
use jsonpath_rust::select;

/// Definition for the new JSONPath-driven parser.
#[derive(Debug, Deserialize)]
pub struct JsonPathParserDefinition {
    /// JSONPath expression that selects every item container in the UI tree.
    pub container_selector: String,
    /// Mapping of field name → JSONPath expression evaluated **relative to the container**.
    pub field_mappings: HashMap<String, String>,
    /// (Optional) Step id whose output contains the ui_tree to parse.
    #[serde(default)]
    pub ui_tree_source_step_id: Option<String>,
}

/// Public entry-point used by the MCP runtime. Signature is **unchanged** so
/// existing call-sites keep compiling.
pub fn run_output_parser(parser_def_val: &Value, tool_output: &Value) -> Result<Option<Value>> {
    // 1. Deserialize the user-supplied definition
    let parser_def: JsonPathParserDefinition = serde_json::from_value(parser_def_val.clone())
        .map_err(|e| anyhow!("Invalid parser definition: {e}"))?;

    // 2. Locate the ui_tree inside `tool_output`
    let ui_tree = find_ui_tree_in_results(tool_output, parser_def.ui_tree_source_step_id.as_deref())?
        .ok_or_else(|| anyhow!("No UI tree found in the tool output"))?;

    // 3. Identify item containers using JSONPath
    let containers = select(&ui_tree, &parser_def.container_selector).map_err(|e| {
        anyhow!(
            "Invalid container_selector JSONPath '{}': {e}",
            parser_def.container_selector
        )
    })?;

    // 4. For each container extract the requested fields
    let mut extracted_items = Vec::new();
    for container in containers {
        let mut item = serde_json::Map::new();

        for (field_name, field_path) in &parser_def.field_mappings {
            match select(container, field_path) {
                Ok(values) if !values.is_empty() => {
                    item.insert(field_name.clone(), values[0].clone());
                }
                Ok(_) => { /* no match – skip */ }
                Err(e) => return Err(anyhow!("Invalid JSONPath '{}': {e}", field_path)),
            }
        }

        if !item.is_empty() {
            extracted_items.push(Value::Object(item));
        }
    }

    Ok(Some(json!(extracted_items)))
}

// -----------------------------------------------------------------------------
// Helper: robust search logic to locate a ui_tree anywhere inside tool output.
// (Copied from the original implementation without changes)
// -----------------------------------------------------------------------------

fn find_ui_tree_in_results(tool_output: &Value, step_id: Option<&str>) -> Result<Option<Value>> {
    // Strategy 0: If step_id is specified, look for that specific step first
    if let Some(target_step_id) = step_id {
        if let Some(results) = tool_output.get("results") {
            if let Some(results_array) = results.as_array() {
                fn search_for_step_id(results: &[Value], target_step_id: &str) -> Option<Value> {
                    for result in results {
                        if let Some(result_step_id) = result.get("step_id").and_then(|v| v.as_str()) {
                            if result_step_id == target_step_id {
                                if let Some(ui_tree) = result.get("ui_tree") {
                                    return Some(ui_tree.clone());
                                }
                                if let Some(result_obj) = result.get("result") {
                                    if let Some(ui_tree) = result_obj.get("ui_tree") {
                                        return Some(ui_tree.clone());
                                    }
                                    if let Some(content) = result_obj.get("content") {
                                        if let Some(content_array) = content.as_array() {
                                            for content_item in content_array {
                                                if let Some(text) = content_item.get("text") {
                                                    if let Some(text_str) = text.as_str() {
                                                        if let Ok(parsed_json) = serde_json::from_str::<Value>(text_str) {
                                                            if let Some(ui_tree) = parsed_json.get("ui_tree") {
                                                                return Some(ui_tree.clone());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                return None;
                            }
                        }
                        if let Some(group_results) = result.get("results") {
                            if let Some(group_results_array) = group_results.as_array() {
                                if let Some(found) = search_for_step_id(group_results_array, target_step_id) {
                                    return Some(found);
                                }
                            }
                        }
                    }
                    None
                }

                if let Some(ui_tree) = search_for_step_id(results_array, target_step_id) {
                    return Ok(Some(ui_tree));
                }
                bail!("Step with ID '{}' not found in results", target_step_id);
            }
        }
        bail!("Step ID '{}' specified but no results array found", target_step_id);
    }

    // Strategy 1: direct ui_tree field
    if let Some(ui_tree) = tool_output.get("ui_tree") {
        return Ok(Some(ui_tree.clone()));
    }

    // Strategy 2: search through results array
    if let Some(results) = tool_output.get("results") {
        if let Some(results_array) = results.as_array() {
            for result in results_array.iter().rev() {
                if let Some(ui_tree) = result.get("ui_tree") {
                    return Ok(Some(ui_tree.clone()));
                }
                if let Some(result_obj) = result.get("result") {
                    if let Some(ui_tree) = result_obj.get("ui_tree") {
                        return Ok(Some(ui_tree.clone()));
                    }
                    if let Some(content) = result_obj.get("content") {
                        if let Some(content_array) = content.as_array() {
                            for content_item in content_array.iter().rev() {
                                if let Some(text) = content_item.get("text") {
                                    if let Some(text_str) = text.as_str() {
                                        if let Ok(parsed_json) = serde_json::from_str::<Value>(text_str) {
                                            if let Some(ui_tree) = parsed_json.get("ui_tree") {
                                                return Ok(Some(ui_tree.clone()));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}
