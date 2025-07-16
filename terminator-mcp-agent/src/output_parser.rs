use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OutputParserDefinition {
    /// Defines how to identify a single item's container within the UI tree.
    pub item_container_definition: ItemContainerDefinition,
    /// Defines which fields to extract from each identified container.
    pub fields_to_extract: std::collections::HashMap<String, FieldExtractor>,
    /// Optional: Specify which step ID contains the UI tree to parse.
    /// If provided, will look for ui_tree in the result of the step with this ID.
    /// If not provided, falls back to searching for the last UI tree in results.
    #[serde(default)]
    pub ui_tree_source_step_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ItemContainerDefinition {
    /// Conditions the container node itself must match.
    pub node_conditions: Vec<PropertyCondition>,
    /// Conditions that must be met by the container's direct children.
    pub child_conditions: LogicalCondition,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FieldExtractor {
    #[serde(default)]
    pub from_child: Option<FromChild>,
    #[serde(default)]
    pub from_children: Option<FromChildren>,
    #[serde(default)]
    pub from_self: Option<FromSelf>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FromChild {
    pub conditions: Vec<PropertyCondition>,
    pub extract_property: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FromChildren {
    pub conditions: Vec<PropertyCondition>,
    pub extract_property: String,
    /// If a value is provided, joins the results into a single string.
    pub join_with: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FromSelf {
    pub extract_property: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LogicalCondition {
    pub logic: Logic, // "and" or "or"
    pub conditions: Vec<ChildCondition>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Logic {
    And,
    Or,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChildCondition {
    /// Asserts that at least one child matches the given property conditions.
    pub exists_child: Option<ExistsChild>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ExistsChild {
    pub conditions: Vec<PropertyCondition>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PropertyCondition {
    pub property: String,
    #[serde(flatten)]
    pub operator: ConditionOperator,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "op", content = "value")]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Equals(String),
    StartsWith(String),
    Contains(String),
    IsOneOf(Vec<String>),
}

/// The main entry point for parsing tool output.
pub fn run_output_parser(parser_def_val: &Value, tool_output: &Value) -> Result<Option<Value>> {
    let parser_def: OutputParserDefinition = serde_json::from_value(parser_def_val.clone())?;

    let parsed_tree =
        find_ui_tree_in_results(tool_output, parser_def.ui_tree_source_step_id.as_deref())?;

    match parsed_tree {
        Some(tree) => {
            let mut extracted_items = Vec::new();
            find_items_in_node(&tree, &parser_def, &mut extracted_items);
            Ok(Some(json!(extracted_items)))
        }
        None => {
            anyhow::bail!("No UI tree found in the tool output. Make sure to include get_focused_window_tree or get_window_tree in your workflow to capture the UI state.");
        }
    }
}

/// Finds a UI tree in the tool output results.
///
/// If step_id is provided, looks for ui_tree in the result of that specific step.
/// Otherwise, searches through the tool output to find the most recent UI tree,
/// which could be in various locations:
/// 1. Direct ui_tree field in the root
/// 2. In results array, looking for ui_tree fields
/// 3. In results array, looking for parsed content that contains ui_tree
/// 4. In results array, looking for text content that can be parsed as JSON containing ui_tree
fn find_ui_tree_in_results(tool_output: &Value, step_id: Option<&str>) -> Result<Option<Value>> {
    // Strategy 0: If step_id is specified, look for that specific step first
    if let Some(target_step_id) = step_id {
        if let Some(results) = tool_output.get("results") {
            if let Some(results_array) = results.as_array() {
                // Recursive function to search through results and group results
                fn search_for_step_id(results: &[Value], target_step_id: &str) -> Option<Value> {
                    for result in results {
                        // Check if this result has a matching step ID
                        if let Some(result_step_id) = result.get("step_id").and_then(|v| v.as_str()) {
                            if result_step_id == target_step_id {
                                // Found the target step, look for UI tree in it
                                if let Some(ui_tree) = result.get("ui_tree") {
                                    return Some(ui_tree.clone());
                                }
                                if let Some(result_obj) = result.get("result") {
                                    if let Some(ui_tree) = result_obj.get("ui_tree") {
                                        return Some(ui_tree.clone());
                                    }
                                    // Check for ui_tree in result.result.content[].text (parsed JSON)
                                    if let Some(content) = result_obj.get("content") {
                                        if let Some(content_array) = content.as_array() {
                                            for content_item in content_array {
                                                if let Some(text) = content_item.get("text") {
                                                    if let Some(text_str) = text.as_str() {
                                                        // Try to parse the text as JSON
                                                        if let Ok(parsed_json) =
                                                            serde_json::from_str::<Value>(text_str)
                                                        {
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
                                // If we found the step but no UI tree, return None
                                return None;
                            }
                        }
                        
                        // If this is a group result, search recursively in its results
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
                
                // Step ID was specified but not found
                anyhow::bail!("Step with ID '{}' not found in results", target_step_id);
            }
        }
        // Step ID was specified but no results array found
        anyhow::bail!(
            "Step ID '{}' specified but no results array found",
            target_step_id
        );
    }

    // Strategy 1: Check if there's a direct ui_tree field
    if let Some(ui_tree) = tool_output.get("ui_tree") {
        return Ok(Some(ui_tree.clone()));
    }

    // Strategy 2: Look through results array for UI trees (fallback behavior)
    if let Some(results) = tool_output.get("results") {
        if let Some(results_array) = results.as_array() {
            // Iterate through results in reverse order to get the last one
            for result in results_array.iter().rev() {
                // Check for direct ui_tree field in result
                if let Some(ui_tree) = result.get("ui_tree") {
                    return Ok(Some(ui_tree.clone()));
                }

                // Check for ui_tree in result.result
                if let Some(result_obj) = result.get("result") {
                    if let Some(ui_tree) = result_obj.get("ui_tree") {
                        return Ok(Some(ui_tree.clone()));
                    }

                    // Check for ui_tree in result.result.content[].text (parsed JSON)
                    if let Some(content) = result_obj.get("content") {
                        if let Some(content_array) = content.as_array() {
                            for content_item in content_array.iter().rev() {
                                if let Some(text) = content_item.get("text") {
                                    if let Some(text_str) = text.as_str() {
                                        // Try to parse the text as JSON
                                        if let Ok(parsed_json) =
                                            serde_json::from_str::<Value>(text_str)
                                        {
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

/// Recursively traverses the UI tree to find nodes that match the container definition.
fn find_items_in_node(
    node: &Value,
    parser_def: &OutputParserDefinition,
    extracted_items: &mut Vec<Value>,
) {
    if check_node_as_container(node, &parser_def.item_container_definition) {
        if let Some(item) = extract_fields_from_container(node, &parser_def.fields_to_extract) {
            extracted_items.push(item);
        }
    }

    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            find_items_in_node(child, parser_def, extracted_items);
        }
    }
}

/// Checks if a given node meets all the criteria to be an item container.
fn check_node_as_container(node: &Value, container_def: &ItemContainerDefinition) -> bool {
    // 1. Check if the node itself matches the conditions
    for condition in &container_def.node_conditions {
        if !check_property_condition(node, condition) {
            return false;
        }
    }

    // 2. Check if the node's children meet the logical conditions
    let children = match node.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => {
            return false;
        }
    };

    let logic_ok = match container_def.child_conditions.logic {
        Logic::And => container_def
            .child_conditions
            .conditions
            .iter()
            .all(|cond| check_child_condition(children, cond)),
        Logic::Or => container_def
            .child_conditions
            .conditions
            .iter()
            .any(|cond| check_child_condition(children, cond)),
    };

    if !logic_ok {
        return false;
    }

    true
}

/// Extracts the defined fields from a container node.
fn extract_fields_from_container(
    container: &Value,
    fields_to_extract: &std::collections::HashMap<String, FieldExtractor>,
) -> Option<Value> {
    let mut extracted_object = serde_json::Map::new();
    let children = container.get("children")?.as_array()?;

    for (field_name, extractor) in fields_to_extract {
        if let Some(from_child) = &extractor.from_child {
            for child in children {
                if from_child
                    .conditions
                    .iter()
                    .all(|cond| check_property_condition(child, cond))
                {
                    if let Some(value) =
                        get_property_value(child, &from_child.extract_property).cloned()
                    {
                        extracted_object.insert(field_name.clone(), value);
                        break;
                    }
                }
            }
        } else if let Some(from_children) = &extractor.from_children {
            let mut found_values: Vec<Value> = Vec::new();
            for child in children {
                if from_children
                    .conditions
                    .iter()
                    .all(|cond| check_property_condition(child, cond))
                {
                    if let Some(value) =
                        get_property_value(child, &from_children.extract_property).cloned()
                    {
                        found_values.push(value);
                    }
                }
            }
            if !found_values.is_empty() {
                if let Some(separator) = &from_children.join_with {
                    let joined = found_values
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(separator);
                    extracted_object.insert(field_name.clone(), json!(joined));
                } else {
                    extracted_object.insert(field_name.clone(), json!(found_values));
                }
            }
        } else if let Some(from_self) = &extractor.from_self {
            if let Some(value) = get_property_value(container, &from_self.extract_property).cloned()
            {
                extracted_object.insert(field_name.clone(), value);
            }
        }
    }

    if extracted_object.is_empty() {
        None
    } else {
        Some(Value::Object(extracted_object))
    }
}

/// Checks if a set of children nodes satisfies a given condition.
fn check_child_condition(children: &[Value], condition: &ChildCondition) -> bool {
    if let Some(exists_child) = &condition.exists_child {
        let result = children.iter().any(|child| {
            exists_child
                .conditions
                .iter()
                .all(|cond| check_property_condition(child, cond))
        });
        return result;
    }
    false
}

/// Gets a property from a node, checking the top level first, then a nested 'attributes' object.
fn get_property_value<'a>(node: &'a Value, property: &str) -> Option<&'a Value> {
    if let Some(value) = node.get(property) {
        return Some(value);
    }
    if let Some(attributes) = node.get("attributes") {
        if let Some(value) = attributes.get(property) {
            return Some(value);
        }
    }
    None
}

/// Checks if a node's property matches a specific condition.
fn check_property_condition(node: &Value, condition: &PropertyCondition) -> bool {
    let prop_value = match get_property_value(node, &condition.property).and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return false;
        }
    };

    let result = match &condition.operator {
        ConditionOperator::Equals(val) => prop_value == val,
        ConditionOperator::StartsWith(val) => prop_value.starts_with(val),
        ConditionOperator::Contains(val) => prop_value.contains(val),
        ConditionOperator::IsOneOf(vals) => vals.iter().any(|v| v == prop_value),
    };
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn get_test_ui_tree() -> Value {
        json!({
            "name": "Root",
            "role": "Document",
            "children": [
                {
                    "name": "Some Header",
                    "role": "Header"
                },
                {
                    "name": "Quote List Container",
                    "role": "Group",
                    "children": [
                        // Valid Quote 1
                        {
                            "name": "Quote 1 Container",
                            "role": "Group",
                            "children": [
                                {"role": "Image", "name": "prosperity primeterm-to-100 logo"},
                                {"role": "Text", "name": "Prosperity PrimeTerm to 100: 20 YEAR TERM*"},
                                {"role": "Text", "name": "$358.56"},
                                {"role": "Text", "name": "Monthly Price"},
                                {"role": "Text", "name": "Graded"},
                                {"role": "Text", "name": "Discontinued"},
                            ]
                        },
                        // Valid Quote 2
                        {
                            "name": "Quote 2 Container",
                            "role": "Group",
                            "children": [
                                {"role": "Image", "name": "acme-insurance-logo"},
                                {"role": "Text", "name": "ACME Insurance: Term Life"},
                                {"role": "Text", "name": "$120.00"},
                                {"role": "Text", "name": "Monthly Price"},
                                {"role": "Text", "name": "Standard"},
                            ]
                        },
                        // Invalid item (no price)
                        {
                            "name": "Invalid Container",
                            "role": "Group",
                            "children": [
                                {"role": "Text", "name": "Some other info"},
                                {"role": "Text", "name": "Monthly Price"},
                            ]
                        }
                    ]
                }
            ]
        })
    }

    fn get_quote_parser_definition() -> OutputParserDefinition {
        serde_json::from_value(json!({
            "itemContainerDefinition": {
                "nodeConditions": [{"property": "role", "op": "equals", "value": "Group"}],
                "childConditions": {
                    "logic": "and",
                    "conditions": [
                        {"existsChild": {"conditions": [{"property": "name", "op": "startsWith", "value": "$"}]}},
                        {"existsChild": {"conditions": [{"property": "name", "op": "equals", "value": "Monthly Price"}]}}
                    ]
                }
            },
            "fieldsToExtract": {
                "carrierProduct": {
                    "fromChild": {
                        "conditions": [{"property": "name", "op": "contains", "value": ":"}],
                        "extractProperty": "name"
                    }
                },
                "monthlyPrice": {
                    "fromChild": {
                        "conditions": [{"property": "name", "op": "startsWith", "value": "$"}],
                        "extractProperty": "name"
                    }
                },
                "status": {
                    "fromChildren": {
                        "conditions": [
                            {"property": "role", "op": "equals", "value": "Text"},
                            {"property": "name", "op": "isOneOf", "value": ["Graded", "Discontinued", "Standard"]}
                        ],
                        "extractProperty": "name",
                        "joinWith": ", "
                    }
                }
            }
        }))
        .unwrap()
    }

    #[test]
    fn test_extract_quotes() {
        let ui_tree = get_test_ui_tree();
        let parser_def = get_quote_parser_definition();

        let mut extracted_items = Vec::new();
        find_items_in_node(&ui_tree, &parser_def, &mut extracted_items);

        assert_eq!(extracted_items.len(), 2);

        // Check first quote
        let quote1 = &extracted_items[0];
        assert_eq!(
            quote1["carrierProduct"],
            "Prosperity PrimeTerm to 100: 20 YEAR TERM*"
        );
        assert_eq!(quote1["monthlyPrice"], "$358.56");
        assert_eq!(quote1["status"], "Graded, Discontinued");

        // Check second quote
        let quote2 = &extracted_items[1];
        assert_eq!(quote2["carrierProduct"], "ACME Insurance: Term Life");
        assert_eq!(quote2["monthlyPrice"], "$120.00");
        assert_eq!(quote2["status"], "Standard");
    }
}
