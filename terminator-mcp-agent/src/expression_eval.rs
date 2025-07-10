use serde_json::Value;
use tracing::warn;

// Helper to get a value from the variables JSON using a dot-separated path.
fn get_value<'a>(path: &str, variables: &'a Value) -> Option<&'a Value> {
    variables.pointer(&format!("/{}", path.replacen('.', "/", 1)))
}

// Main evaluation function.
pub fn evaluate(expression: &str, variables: &Value) -> bool {
    evaluate_with_context(expression, variables, false)
}

// Main evaluation function with context about whether the workflow has been cancelled
pub fn evaluate_with_context(expression: &str, variables: &Value, is_cancelled: bool) -> bool {
    // Trim whitespace
    let expr = expression.trim();

    // Check for always() function first - it always returns true
    if expr == "always()" || expr == "${{ always() }}" {
        return true;
    }

    // Check for !cancelled() expression
    if expr == "!cancelled()" || expr == "${{ !cancelled() }}" {
        return !is_cancelled;
    }

    // Check for cancelled() expression
    if expr == "cancelled()" || expr == "${{ cancelled() }}" {
        return is_cancelled;
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
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 3 {
        return None;
    }

    let (var_path, op, raw_rhs) = (parts[0], parts[1], parts[2]);

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
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_evaluate_binary_expressions() {
        let vars = json!({
            "policy": {
                "use_max_budget": false,
                "coverage_type": "Graded"
            }
        });

        assert!(evaluate("policy.use_max_budget == false", &vars));
        assert!(!evaluate("policy.use_max_budget == true", &vars));
        assert!(evaluate("policy.coverage_type == 'Graded'", &vars));
        assert!(evaluate("policy.coverage_type != 'Standard'", &vars));
    }

    #[test]
    fn test_evaluate_contains() {
        let vars = json!({
            "policy": {
                "product_types": ["FEX", "Term"],
                "description": "Final Expense"
            }
        });

        assert!(evaluate("contains(policy.product_types, 'FEX')", &vars));
        assert!(!evaluate("contains(policy.product_types, 'MedSup')", &vars));
        assert!(evaluate("contains(policy.description, 'Expense')", &vars));
    }

    #[test]
    fn test_evaluate_starts_with() {
        let vars = json!({ "applicant": { "name": "John Doe" } });
        assert!(evaluate("startsWith(applicant.name, 'John')", &vars));
        assert!(!evaluate("startsWith(applicant.name, 'Doe')", &vars));
    }

    #[test]
    fn test_evaluate_ends_with() {
        let vars = json!({ "applicant": { "name": "John Doe" } });
        assert!(evaluate("endsWith(applicant.name, 'Doe')", &vars));
        assert!(!evaluate("endsWith(applicant.name, 'John')", &vars));
    }

    #[test]
    fn test_invalid_expressions() {
        let vars = json!({});
        assert!(!evaluate("invalid expression", &vars)); // Invalid format
        assert!(!evaluate("unsupported(a, b)", &vars)); // Unsupported function
        assert!(!evaluate("var.not.found == true", &vars)); // Variable not found
    }

    #[test]
    fn test_always_function() {
        let vars = json!({});
        assert!(evaluate("always()", &vars));
        assert!(evaluate("${{ always() }}", &vars));
        assert!(evaluate_with_context("always()", &vars, true)); // Always true even when cancelled
        assert!(evaluate_with_context("always()", &vars, false)); // Always true when not cancelled
    }

    #[test]
    fn test_cancelled_function() {
        let vars = json!({});
        assert!(!evaluate("cancelled()", &vars)); // Default is not cancelled
        assert!(evaluate("!cancelled()", &vars)); // Not cancelled is true by default
        assert!(evaluate_with_context("cancelled()", &vars, true)); // True when cancelled
        assert!(!evaluate_with_context("cancelled()", &vars, false)); // False when not cancelled
        assert!(!evaluate_with_context("!cancelled()", &vars, true)); // False when cancelled
        assert!(evaluate_with_context("!cancelled()", &vars, false)); // True when not cancelled
    }
}
