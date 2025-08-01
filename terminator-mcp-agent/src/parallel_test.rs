// Test file for parallel execution features

use serde_json::json;
use crate::utils::{SequenceStep, ExecutionStrategy, create_execution_plan};

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sequential_execution_strategy() {
        let steps = vec![
            SequenceStep {
                tool_name: Some("tool1".to_string()),
                id: Some("step1".to_string()),
                parallelizable: Some(false),
                ..Default::default()
            },
            SequenceStep {
                tool_name: Some("tool2".to_string()),
                id: Some("step2".to_string()),
                parallelizable: Some(false),
                ..Default::default()
            },
        ];
        
        let plan = create_execution_plan(&steps, ExecutionStrategy::Sequential, 4);
        
        assert_eq!(plan.sequential_groups.len(), 1);
        assert_eq!(plan.parallel_groups.len(), 0);
        assert_eq!(plan.sequential_groups[0], vec![0, 1]);
    }
    
    #[test]
    fn test_parallel_execution_strategy() {
        let steps = vec![
            SequenceStep {
                tool_name: Some("tool1".to_string()),
                id: Some("step1".to_string()),
                parallelizable: Some(true),
                ..Default::default()
            },
            SequenceStep {
                tool_name: Some("tool2".to_string()),
                id: Some("step2".to_string()),
                parallelizable: Some(true),
                ..Default::default()
            },
        ];
        
        let plan = create_execution_plan(&steps, ExecutionStrategy::Parallel, 4);
        
        // Should have one parallel group with both steps
        assert_eq!(plan.parallel_groups.len(), 1);
        assert_eq!(plan.parallel_groups[0].steps, vec![0, 1]);
        assert_eq!(plan.parallel_groups[0].max_parallel, Some(4));
    }
    
    #[test]
    fn test_mixed_execution_strategy() {
        let steps = vec![
            SequenceStep {
                tool_name: Some("tool1".to_string()),
                id: Some("step1".to_string()),
                parallel_group_id: Some("group1".to_string()),
                max_parallel: Some(2),
                ..Default::default()
            },
            SequenceStep {
                tool_name: Some("tool2".to_string()),
                id: Some("step2".to_string()),
                parallel_group_id: Some("group1".to_string()),
                ..Default::default()
            },
            SequenceStep {
                tool_name: Some("tool3".to_string()),
                id: Some("step3".to_string()),
                // No parallel group - should be sequential
                ..Default::default()
            },
        ];
        
        let plan = create_execution_plan(&steps, ExecutionStrategy::Mixed, 4);
        
        // Should have one parallel group and one sequential step
        assert_eq!(plan.parallel_groups.len(), 1);
        assert_eq!(plan.sequential_groups.len(), 1);
        
        // Parallel group should have steps 0 and 1
        assert_eq!(plan.parallel_groups[0].steps, vec![0, 1]);
        assert_eq!(plan.parallel_groups[0].group_id, "group1");
        assert_eq!(plan.parallel_groups[0].max_parallel, Some(2));
        
        // Sequential group should have step 2
        assert_eq!(plan.sequential_groups[0], vec![2]);
    }
    
    #[test]
    fn test_execution_strategy_from_string() {
        assert_eq!(ExecutionStrategy::from(Some("parallel".to_string())), ExecutionStrategy::Parallel);
        assert_eq!(ExecutionStrategy::from(Some("mixed".to_string())), ExecutionStrategy::Mixed);
        assert_eq!(ExecutionStrategy::from(Some("sequential".to_string())), ExecutionStrategy::Sequential);
        assert_eq!(ExecutionStrategy::from(None), ExecutionStrategy::Sequential);
        assert_eq!(ExecutionStrategy::from(Some("invalid".to_string())), ExecutionStrategy::Sequential);
    }
    
    #[test]
    fn test_dependency_handling() {
        let steps = vec![
            SequenceStep {
                tool_name: Some("tool1".to_string()),
                id: Some("step1".to_string()),
                parallelizable: Some(true),
                ..Default::default()
            },
            SequenceStep {
                tool_name: Some("tool2".to_string()),
                id: Some("step2".to_string()),
                parallelizable: Some(true),
                depends_on: Some(vec!["step1".to_string()]),
                ..Default::default()
            },
        ];
        
        let plan = create_execution_plan(&steps, ExecutionStrategy::Parallel, 4);
        
        // Should handle dependencies correctly
        assert!(plan.dependency_graph.contains_key("step2"));
        assert_eq!(plan.dependency_graph["step2"], vec!["step1"]);
    }
}