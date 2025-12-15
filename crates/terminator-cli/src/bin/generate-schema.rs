//! Schema generation for Terminator MCP Workflow
use std::{fs, path::Path};
use schemars::schema_for;
use serde_json::{json, Value, to_value};
use std::collections::BTreeMap;
use terminator_mcp_agent::utils::*;

fn main() {
    let output = std::process::Command::new(env!("CARGO"))
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .unwrap()
        .stdout;

    let work_space_toml = Path::new(std::str::from_utf8(&output).unwrap().trim());

    let schema_dir = work_space_toml.parent().unwrap().join("schema");
    if !schema_dir.try_exists().unwrap() {
        fs::create_dir_all(&schema_dir).unwrap();
    }
    let schema_path = schema_dir.join("workflow-schema.yml");
    let schema = workflow_schema();
    fs::write(schema_path, serde_yaml::to_string(&schema).unwrap()).unwrap();
}

fn workflow_schema() -> Value {
    /* 
        map all `tool_name` to their args, 
        tool names are hardcoded at this time
    */
    let tools: BTreeMap<&str, Value> = BTreeMap::from([
        ("activate_element", to_value(schema_for!(ActivateElementArgs)).unwrap()),
        ("capture_element_screenshot", to_value(schema_for!(CaptureElementScreenshotArgs)).unwrap()),
        ("click_element", to_value(schema_for!(ClickElementArgs)).unwrap()),
        ("close_element", to_value(schema_for!(CloseElementArgs)).unwrap()),
        ("delay", to_value(schema_for!(DelayArgs)).unwrap()),
        ("execute_browser_script", to_value(schema_for!(ExecuteBrowserScriptArgs)).unwrap()),
        ("execute_sequence", to_value(schema_for!(ExecuteSequenceArgs)).unwrap()),
        ("get_applications_and_windows_list", to_value(schema_for!(GetApplicationsArgs)).unwrap()),
        ("get_window_tree", to_value(schema_for!(GetWindowTreeArgs)).unwrap()),
        ("get_range_value", to_value(schema_for!(LocatorArgs)).unwrap()),
        ("highlight_element", to_value(schema_for!(HighlightElementArgs)).unwrap()),
        ("invoke_element", to_value(schema_for!(LocatorArgs)).unwrap()),
        ("is_selected", to_value(schema_for!(LocatorArgs)).unwrap()),
        ("is_toggled", to_value(schema_for!(LocatorArgs)).unwrap()),
        ("list_options", to_value(schema_for!(LocatorArgs)).unwrap()),
        ("maximize_window", to_value(schema_for!(MaximizeWindowArgs)).unwrap()),
        ("minimize_window", to_value(schema_for!(MinimizeWindowArgs)).unwrap()),
        ("mouse_drag", to_value(schema_for!(MouseDragArgs)).unwrap()),
        ("navigate_browser", to_value(schema_for!(NavigateBrowserArgs)).unwrap()),
        ("open_application", to_value(schema_for!(OpenApplicationArgs)).unwrap()),
        ("press_key", to_value(schema_for!(PressKeyArgs)).unwrap()),
        ("press_key_global", to_value(schema_for!(GlobalKeyArgs)).unwrap()),
        ("run_command", to_value(schema_for!(RunCommandArgs)).unwrap()),
        ("scroll_element", to_value(schema_for!(ScrollElementArgs)).unwrap()),
        ("select_option", to_value(schema_for!(SelectOptionArgs)).unwrap()),
        ("set_range_value", to_value(schema_for!(SetRangeValueArgs)).unwrap()),
        ("set_selected", to_value(schema_for!(SetSelectedArgs)).unwrap()),
        ("set_toggled", to_value(schema_for!(SetToggledArgs)).unwrap()),
        ("set_value", to_value(schema_for!(SetValueArgs)).unwrap()),
        ("set_zoom", to_value(schema_for!(SetZoomArgs)).unwrap()),
        ("stop_highlighting", to_value(schema_for!(StopHighlightingArgs)).unwrap()),
        ("type_into_element", to_value(schema_for!(TypeIntoElementArgs)).unwrap()),
        ("validate_element", to_value(schema_for!(ValidateElementArgs)).unwrap()),
        ("wait_for_element", to_value(schema_for!(WaitForElementArgs)).unwrap()),
    ]);

    // schema base
    let mut combined = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "Terminator Workflow Schema",
        "description": "YAML workflow schema for Terminator Automation Engine.",
        "type": "object",
        "anyOf": [
            { "$ref": "#/definitions/DirectWorkflow" },
            { "$ref": "#/definitions/ExecuteSequenceWrapper" }
        ],
        "definitions": {
            "DirectWorkflow": {
                "type": "object",
                "properties": {
                    "variables": { "$ref": "#/definitions/Variables" },
                    "inputs": {
                        "type": "object",
                        "description": "A key-value map of the actual input values for the variables."
                    },
                    "selectors": {
                        "type": "object",
                        "description": "A key-value map of static UI element selectors.",
                        "additionalProperties": { "type": "string" }
                    },
                    "steps": {
                        "type": "array",
                        "description": "The steps of the workflow to execute in order.",
                        "minItems": 1,
                        "items": { "$ref": "#/definitions/Step" }
                    }
                },
                "required": ["steps"]
            },
            "ExecuteSequenceWrapper": {
                "type": "object",
                "required": ["tool_name", "arguments"],
                "properties": {
                    "tool_name": { "const": "execute_sequence" },
                    "arguments": { "$ref": "#/definitions/DirectWorkflow" }
                }
            },
            "Step": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Optional unique identifier for this step." },
                    "name": { "type": "string", "description": "A human-readable name for this step (for logging)." },
                    "delay_ms": { "type": "integer", "description": "Delay in milliseconds after this step." },
                    "continue_on_error": { "type": "boolean", "description": "Continue sequence even if this step fails." },
                    "tool_name": {
                        "type": "string",
                        "description": "The tool to execute.",
                    },
                    "group_name": { "type": "string", "description": "Name for a group of steps." },
                    "arguments": { "type": "object", "description": "Arguments for the tool." }
                },
                "oneOf": [
                    { "required": ["tool_name"], "not": { "required": ["group_name"] } },
                    { "required": ["group_name"], "not": { "required": ["tool_name"] } }
                ],
                "allOf": []
            },
            "Variables": {
                "type": "object",
                "patternProperties": {
                    "^[^\\s]+$": {
                        "type": "object",
                        "required": ["label"],
                        "properties": {
                            "label": { "type": "string", "minLength": 1 },
                            "type": { "type": "string" },
                            "required": { "type": "boolean", "default": true },
                            "default": {}
                        },
                        "allOf": [
                            {
                                "if": { "properties": { "required": { "const": true } } },
                                "then": {
                                    "anyOf": [
                                        { "required": ["default"] },
                                        { "description": "If required and no default, must be provided via inputs" }
                                    ]
                                }
                            }
                        ]
                    }
                }
            }
        }
    });


    let mut step_all_of = Vec::new(); 
    let tool_names: Vec<Value> = tools.keys().map(|k| Value::String(k.to_string())).collect();
    {
        let definitions = combined["definitions"].as_object_mut().unwrap();
        for (tool_name, mut schema_val) in tools {
            let tool_def_name = format!("{}Args", tool_name);
            // hoist definitions
            let mut sub_defs_to_add = Vec::new();
            if let Some(schema_obj) = schema_val.as_object_mut() {
                if let Some(defs_val) = schema_obj.remove("definitions") {
                    if let Value::Object(defs_map) = defs_val {
                        sub_defs_to_add.extend(defs_map.into_iter());
                    }
                }
                if let Some(defs_val) = schema_obj.remove("$defs") {
                    if let Value::Object(defs_map) = defs_val {
                        sub_defs_to_add.extend(defs_map.into_iter());
                    }
                }
            }
            for (def_name, mut def_val) in sub_defs_to_add {
                refs(&mut def_val);
                if !definitions.contains_key(&def_name) {
                    definitions.insert(def_name, def_val);
                }
            }

            refs(&mut schema_val);
            definitions.insert(tool_def_name.clone(), schema_val);

            // handle if-then block
            step_all_of.push(json!({
                "if": {
                    "properties": { "tool_name": { "const": tool_name } }
                },
                "then": {
                    "properties": { "arguments": { "$ref": format!("#/definitions/{}", tool_def_name) } }
                }
            }));
        }
    }

    combined["definitions"]["Step"]["allOf"] = Value::Array(step_all_of);

    if let Some(props) = combined["definitions"]["Step"]
        .get_mut("properties")
        .and_then(|p| p.as_object_mut())
    {
        props.get_mut("tool_name").unwrap()["enum"] = Value::Array(tool_names);
    }

    refs(&mut combined);
    combined
}


fn refs(value: &mut Value) {
    /* 
        recursively correct the references
    */
    match value {
        Value::Object(map) => {
            if let Some(ref_val) = map.get_mut("$ref") {
                if let Some(ref_str) = ref_val.as_str().map(|s| s.to_string()) {
                    let mut new_ref = ref_str;
                    if new_ref.contains("/$defs/") {
                        new_ref = new_ref.replace("/$defs/", "/definitions/");
                    }
                    if new_ref.starts_with("/definitions/") {
                        new_ref = format!("#{}", new_ref);
                    }
                    *ref_val = Value::String(new_ref);
                }
            }
            for v in map.values_mut() {
                refs(v);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                refs(v);
            }
        }
        _ => {}
    }
}

