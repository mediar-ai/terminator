// Test file for simple parallel execution features

use serde_json::json;
use crate::utils::{SequenceStep, create_simple_execution_plan};

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_sequential_only() {
        let steps = vec![
            SequenceStep {
                tool_name: Some("tool1".to_string()),
                id: Some("step1".to_string()),
                parallel: Some(false),
                ..Default::default()
            },
            SequenceStep {
                tool_name: Some("tool2".to_string()),
                id: Some("step2".to_string()),
                parallel: None, // defaults to false
                ..Default::default()
            },
        ];
        
        let (sequential_steps, parallel_groups) = create_simple_execution_plan(&steps);
        
        assert_eq!(sequential_steps, vec![0, 1]);
        assert_eq!(parallel_groups.len(), 0);
    }
    
    #[test]
    fn test_simple_parallel_group() {
        let steps = vec![
            SequenceStep {
                tool_name: Some("tool1".to_string()),
                id: Some("step1".to_string()),
                parallel: Some(true),
                ..Default::default()
            },
            SequenceStep {
                tool_name: Some("tool2".to_string()),
                id: Some("step2".to_string()),
                parallel: Some(true),
                ..Default::default()
            },
        ];
        
        let (sequential_steps, parallel_groups) = create_simple_execution_plan(&steps);
        
        assert_eq!(sequential_steps.len(), 0);
        assert_eq!(parallel_groups.len(), 1);
        assert_eq!(parallel_groups[0], vec![0, 1]);
    }
    
    #[test]
    fn test_mixed_execution() {
        let steps = vec![
            SequenceStep {
                tool_name: Some("tool1".to_string()),
                id: Some("step1".to_string()),
                parallel: Some(false),
                ..Default::default()
            },
            SequenceStep {
                tool_name: Some("tool2".to_string()),
                id: Some("step2".to_string()),
                parallel: Some(true),
                ..Default::default()
            },
            SequenceStep {
                tool_name: Some("tool3".to_string()),
                id: Some("step3".to_string()),
                parallel: Some(true),
                ..Default::default()
            },
            SequenceStep {
                tool_name: Some("tool4".to_string()),
                id: Some("step4".to_string()),
                parallel: Some(false),
                ..Default::default()
            },
        ];
        
        let (sequential_steps, parallel_groups) = create_simple_execution_plan(&steps);
        
        // Step 0 is sequential, steps 1-2 are parallel, step 3 is sequential
        assert_eq!(sequential_steps, vec![0, 3]);
        assert_eq!(parallel_groups.len(), 1);
        assert_eq!(parallel_groups[0], vec![1, 2]);
    }
}