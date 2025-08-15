use serde_json::Value;
use tracing::warn;

// Helper to get a value from the variables JSON.
fn get_value<'a>(path: &str, variables: &'a Value) -> Option<&'a Value> {
    // Only access top-level keys. No more dot notation for nested objects.
    variables.get(path)
}

// Main evaluation function.
pub fn evaluate(expression: &str, variables: &Value) -> bool {
    // Trim whitespace
    let expr = expression.trim();

    // Handle negation operator
    if let Some(inner_expr) = expr.strip_prefix('!') {
        let inner_expr = inner_expr.trim();
        return !evaluate(inner_expr, variables);
    }

    // Try parsing function-based expressions first, e.g., contains(vars, 'value')
    if let Some(result) = parse_and_evaluate_function(expr, variables) {
        return result;
    }

    // Fallback to simple binary expressions, e.g., vars == 'value'
    if let Some(result) = parse_and_evaluate_binary_expression(expr, variables) {
        return result;
    }

    warn!(
        "Could not parse expression: '{}'. Defaulting to false.",
        expression
    );
    false
}

// Parses expressions like "contains(policy.product_types, 'FEX')"
fn parse_and_evaluate_function(expr: &str, variables: &Value) -> Option<bool> {
    let (func_name, args_str) = expr.split_once('(')?;
    if !args_str.ends_with(')') {
        return None;
    }
    let args_str = &args_str[..args_str.len() - 1]; // Remove trailing ')'

    match func_name.trim() {
        "always" => {
            // always() function takes no arguments and always returns true
            if args_str.trim().is_empty() {
                Some(true)
            } else {
                None // always() should not have arguments
            }
        }
        _ => {
            // For other functions, we need exactly 2 arguments
            let args: Vec<&str> = args_str.split(',').map(|s| s.trim()).collect();
            if args.len() != 2 {
                return None;
            }

            let val1 = get_value(args[0], variables)?;
            let val2_str = args[1].trim_matches('\''); // Remove single quotes

            match func_name.trim() {
                "contains" => Some(evaluate_contains(val1, val2_str)),
                "startsWith" => Some(val1.as_str()?.starts_with(val2_str)),
                "endsWith" => Some(val1.as_str()?.ends_with(val2_str)),
                _ => None,
            }
        }
    }
}

// Evaluates the 'contains' function for different types.
fn evaluate_contains(collection: &Value, item: &str) -> bool {
    match collection {
        Value::Array(arr) => arr.iter().any(|v| v.as_str() == Some(item)),
        Value::String(s) => s.contains(item),
        _ => false,
    }
}

// Parses simple expressions like "variable == 'value'" or "variable == true"
fn parse_and_evaluate_binary_expression(expr: &str, variables: &Value) -> Option<bool> {
    let (var_path, op, raw_rhs) = if let Some(pos) = expr.find("==") {
        (&expr[..pos], "==", &expr[pos + 2..])
    } else if let Some(pos) = expr.find("!=") {
        (&expr[..pos], "!=", &expr[pos + 2..])
    } else {
        return None;
    };

    let var_path = var_path.trim();
    let raw_rhs = raw_rhs.trim();

    let lhs = get_value(var_path, variables)?;

    let are_equal = match raw_rhs {
        "true" => lhs.as_bool() == Some(true),
        "false" => lhs.as_bool() == Some(false),
        _ if raw_rhs.starts_with('\'') && raw_rhs.ends_with('\'') => {
            let rhs_str = raw_rhs.trim_matches('\'');
            lhs.as_str() == Some(rhs_str)
        }
        _ => return None, // Invalid RHS
    };

    match op {
        "==" => Some(are_equal),
        "!=" => Some(!are_equal),
        _ => None, // Should be unreachable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_evaluate_binary_expressions() {
        let vars = json!({
            "use_max_budget": false,
            "coverage_type": "Graded"
        });

        assert!(evaluate("use_max_budget == false", &vars));
        assert!(!evaluate("use_max_budget == true", &vars));
        assert!(evaluate("coverage_type == 'Graded'", &vars));
        assert!(evaluate("coverage_type != 'Standard'", &vars));
    }

    #[test]
    fn test_evaluate_contains() {
        let vars = json!({
            "product_types": ["FEX", "Term"],
            "description": "Final Expense"
        });

        assert!(evaluate("contains(product_types, 'FEX')", &vars));
        assert!(!evaluate("contains(product_types, 'MedSup')", &vars));
        assert!(evaluate("contains(description, 'Expense')", &vars));
    }

    #[test]
    fn test_evaluate_starts_with() {
        let vars = json!({ "name": "John Doe" });
        assert!(evaluate("startsWith(name, 'John')", &vars));
        assert!(!evaluate("startsWith(name, 'Doe')", &vars));
    }

    #[test]
    fn test_evaluate_ends_with() {
        let vars = json!({ "name": "John Doe" });
        assert!(evaluate("endsWith(name, 'Doe')", &vars));
        assert!(!evaluate("endsWith(name, 'John')", &vars));
    }

    #[test]
    fn test_string_with_spaces() {
        let vars = json!({
            "quote_type": "Face Amount"
        });

        assert!(evaluate("quote_type == 'Face Amount'", &vars));
        assert!(!evaluate("quote_type == 'Monthly Amount'", &vars));
    }

    #[test]
    fn test_invalid_expressions() {
        let vars = json!({});
        assert!(!evaluate("invalid expression", &vars)); // Invalid format
        assert!(!evaluate("unsupported(a, b)", &vars)); // Unsupported function
        assert!(!evaluate("var.not.found == true", &vars)); // Variable not found
    }

    // New tests for negation operator
    #[test]
    fn test_negation_contains() {
        let vars = json!({
            "product_types": ["FEX", "Term"],
            "description": "Final Expense"
        });

        // Test !contains with arrays
        assert!(!evaluate("!contains(product_types, 'FEX')", &vars)); // FEX is in array, so !contains is false
        assert!(evaluate("!contains(product_types, 'MedSup')", &vars)); // MedSup not in array, so !contains is true

        // Test !contains with strings
        assert!(!evaluate("!contains(description, 'Expense')", &vars)); // "Expense" is in string, so !contains is false
        assert!(evaluate("!contains(description, 'Medical')", &vars)); // "Medical" not in string, so !contains is true
    }

    #[test]
    fn test_negation_starts_with() {
        let vars = json!({ "name": "John Doe" });

        assert!(!evaluate("!startsWith(name, 'John')", &vars)); // Starts with John, so !startsWith is false
        assert!(evaluate("!startsWith(name, 'Doe')", &vars)); // Doesn't start with Doe, so !startsWith is true
    }

    #[test]
    fn test_negation_ends_with() {
        let vars = json!({ "name": "John Doe" });

        assert!(!evaluate("!endsWith(name, 'Doe')", &vars)); // Ends with Doe, so !endsWith is false
        assert!(evaluate("!endsWith(name, 'John')", &vars)); // Doesn't end with John, so !endsWith is true
    }

    #[test]
    fn test_negation_binary_expressions() {
        let vars = json!({
            "use_max_budget": false,
            "coverage_type": "Graded",
            "enabled": true
        });

        // Test negation of equality
        assert!(!evaluate("!use_max_budget == false", &vars)); // use_max_budget is false, so !(false == false) = !true = false
        assert!(evaluate("!use_max_budget == true", &vars)); // use_max_budget is false, so !(false == true) = !false = true

        // Test negation of string equality
        assert!(!evaluate("!coverage_type == 'Graded'", &vars)); // coverage_type is Graded, so !(Graded == Graded) = !true = false
        assert!(evaluate("!coverage_type == 'Standard'", &vars)); // coverage_type is Graded, so !(Graded == Standard) = !false = true

        // Test negation of inequality
        assert!(evaluate("!coverage_type != 'Graded'", &vars)); // coverage_type is Graded, so !(Graded != Graded) = !false = true
        assert!(!evaluate("!coverage_type != 'Standard'", &vars)); // coverage_type is Graded, so !(Graded != Standard) = !true = false

        // Test negation of boolean values
        assert!(!evaluate("!enabled == true", &vars)); // enabled is true, so !(true == true) = !true = false
        assert!(evaluate("!enabled == false", &vars)); // enabled is true, so !(true == false) = !false = true
    }

    #[test]
    fn test_negation_with_whitespace() {
        let vars = json!({
            "product_types": ["FEX", "Term"]
        });

        // Test various whitespace combinations
        assert!(evaluate("! contains(product_types, 'MedSup')", &vars));
        assert!(evaluate("!  contains(product_types, 'MedSup')", &vars));
        assert!(evaluate("  !contains(product_types, 'MedSup')", &vars));
        assert!(evaluate("  ! contains(product_types, 'MedSup')", &vars));
        assert!(evaluate("  !  contains(product_types, 'MedSup')  ", &vars));
    }

    #[test]
    fn test_double_negation() {
        let vars = json!({
            "product_types": ["FEX", "Term"]
        });

        // Double negation should cancel out
        assert!(evaluate("!!contains(product_types, 'FEX')", &vars)); // !!true = true
        assert!(!evaluate("!!contains(product_types, 'MedSup')", &vars)); // !!false = false
    }

    #[test]
    fn test_triple_negation() {
        let vars = json!({
            "product_types": ["FEX", "Term"]
        });

        // Triple negation should be equivalent to single negation
        assert!(!evaluate("!!!contains(product_types, 'FEX')", &vars)); // !!!true = !true = false
        assert!(evaluate("!!!contains(product_types, 'MedSup')", &vars)); // !!!false = !false = true
    }

    #[test]
    fn test_negation_with_missing_variables() {
        let vars = json!({});

        // Negation of expressions with missing variables should still default to false, then be negated
        assert!(evaluate("!contains(missing_var, 'value')", &vars)); // !false = true
        assert!(evaluate("!missing_var == 'value'", &vars)); // !false = true
        assert!(evaluate("!startsWith(missing_var, 'test')", &vars)); // !false = true
    }

    #[test]
    fn test_negation_edge_cases() {
        let vars = json!({
            "empty_array": [],
            "empty_string": "",
            "null_value": null
        });

        // Test negation with empty/null values
        assert!(evaluate("!contains(empty_array, 'anything')", &vars)); // !false = true
        assert!(evaluate("!contains(empty_string, 'anything')", &vars)); // !false = true
        assert!(evaluate("!startsWith(empty_string, 'test')", &vars)); // !false = true
        assert!(evaluate("!endsWith(empty_string, 'test')", &vars)); // !false = true

        // Test with null values (should default to false, then be negated)
        assert!(evaluate("!null_value == 'test'", &vars)); // !false = true
    }

    #[test]
    fn test_complex_negation_scenarios() {
        let vars = json!({
            "product_types": ["FEX", "Term", "MedSup"],
            "quote_type": "Face Amount",
            "enabled": true,
            "user_name": "John Smith"
        });

        // Test realistic workflow scenarios
        assert!(!evaluate("!contains(product_types, 'FEX')", &vars)); // FEX is selected, so we don't want to uncheck it
        assert!(evaluate("!contains(product_types, 'Preneed')", &vars)); // Preneed is not selected, so we might want to uncheck it

        assert!(!evaluate("!quote_type == 'Face Amount'", &vars)); // Quote type is Face Amount, so condition is false
        assert!(evaluate("!quote_type == 'Monthly Amount'", &vars)); // Quote type is not Monthly Amount, so condition is true

        assert!(evaluate("!startsWith(user_name, 'Jane')", &vars)); // User name doesn't start with Jane
        assert!(!evaluate("!endsWith(user_name, 'Smith')", &vars)); // User name does end with Smith
    }

    #[test]
    fn test_negation_preserves_original_functionality() {
        let vars = json!({
            "product_types": ["FEX", "Term"],
            "quote_type": "Face Amount",
            "enabled": true
        });

        // Ensure original functionality still works
        assert!(evaluate("contains(product_types, 'FEX')", &vars));
        assert!(!evaluate("contains(product_types, 'MedSup')", &vars));
        assert!(evaluate("quote_type == 'Face Amount'", &vars));
        assert!(!evaluate("quote_type == 'Monthly Amount'", &vars));
        assert!(evaluate("enabled == true", &vars));
        assert!(!evaluate("enabled == false", &vars));

        // And that negation works correctly
        assert!(!evaluate("!contains(product_types, 'FEX')", &vars));
        assert!(evaluate("!contains(product_types, 'MedSup')", &vars));
        assert!(!evaluate("!quote_type == 'Face Amount'", &vars));
        assert!(evaluate("!quote_type == 'Monthly Amount'", &vars));
        assert!(!evaluate("!enabled == true", &vars));
        assert!(evaluate("!enabled == false", &vars));
    }

    // Tests for always() function
    #[test]
    fn test_always_function() {
        let vars = json!({
            "some_var": "some_value"
        });

        // always() should always return true regardless of variables
        assert!(evaluate("always()", &vars));

        // Test with empty variables
        let empty_vars = json!({});
        assert!(evaluate("always()", &empty_vars));

        // Test with null variables
        let null_vars = json!(null);
        assert!(evaluate("always()", &null_vars));
    }

    #[test]
    fn test_always_function_with_whitespace() {
        let vars = json!({});

        // Test various whitespace combinations
        assert!(evaluate("always()", &vars));
        assert!(evaluate("always( )", &vars));
        assert!(evaluate("always(  )", &vars));
        assert!(evaluate(" always() ", &vars));
        assert!(evaluate("  always()  ", &vars));
    }

    #[test]
    fn test_always_function_with_arguments_should_fail() {
        let vars = json!({});

        // always() should not accept arguments
        assert!(!evaluate("always(arg)", &vars));
        assert!(!evaluate("always('test')", &vars));
        assert!(!evaluate("always(var1, var2)", &vars));
    }

    #[test]
    fn test_negation_of_always() {
        let vars = json!({});

        // !always() should always be false
        assert!(!evaluate("!always()", &vars));
        assert!(!evaluate("! always()", &vars));
        assert!(!evaluate("  !always()  ", &vars));

        // Double negation should be true
        assert!(evaluate("!!always()", &vars));
    }
}
