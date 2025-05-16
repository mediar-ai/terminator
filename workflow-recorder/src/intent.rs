use crate::{RecordedEvent, RecordedWorkflow, WorkflowEvent, WindowEvent};
use std::time::Duration;

/// Represents a group of related events that form a user intent
#[derive(Debug, Clone)]
pub struct IntentGroup {
    /// The name of the intent group
    pub name: String,
    
    /// The events in this group
    pub events: Vec<RecordedEvent>,
    
    /// The start time of the group
    pub start_time: u64,
    
    /// The end time of the group
    pub end_time: u64,
    
    /// The application context of this group
    pub application_context: Option<ApplicationContext>,
}

/// Represents the application context of an intent group
#[derive(Debug, Clone)]
pub struct ApplicationContext {
    /// The application name
    pub application_name: Option<String>,
    
    /// The window title
    pub window_title: Option<String>,
    
    /// The process ID
    pub process_id: Option<u32>,
}

/// Configuration for intent grouping
#[derive(Debug, Clone)]
pub struct IntentGroupingConfig {
    /// The maximum time gap between events in the same group (milliseconds)
    pub max_time_gap: u64,
    
    /// Whether to create new groups on window focus changes
    pub split_on_focus_change: bool,
    
    /// Whether to create new groups on significant pauses
    pub split_on_pause: bool,
    
    /// The minimum pause duration to trigger a new group (milliseconds)
    pub min_pause_duration: u64,
}

impl Default for IntentGroupingConfig {
    fn default() -> Self {
        Self {
            max_time_gap: 2000, // 2 seconds
            split_on_focus_change: true,
            split_on_pause: true,
            min_pause_duration: 3000, // 3 seconds
        }
    }
}

/// Groups events in a workflow into intent groups
pub fn group_events(workflow: &RecordedWorkflow, config: &IntentGroupingConfig) -> Vec<IntentGroup> {
    let mut groups = Vec::new();
    let mut current_group_events = Vec::new();
    let mut current_app_context: Option<ApplicationContext> = None;
    let mut last_event_time: Option<u64> = None;
    
    // Helper function to finalize a group
    let mut finalize_group = |events: &mut Vec<RecordedEvent>, app_context: &mut Option<ApplicationContext>| {
        if !events.is_empty() {
            let start_time = events.first().unwrap().timestamp;
            let end_time = events.last().unwrap().timestamp;
            
            // Determine group name based on context and events
            let name = if let Some(context) = app_context.as_ref() {
                if let Some(app_name) = &context.application_name {
                    format!("Interaction with {}", app_name)
                } else if let Some(title) = &context.window_title {
                    format!("Interaction with window: {}", title)
                } else {
                    "Unknown interaction".to_string()
                }
            } else {
                "Unknown interaction".to_string()
            };
            
            groups.push(IntentGroup {
                name,
                events: events.clone(),
                start_time,
                end_time,
                application_context: app_context.clone(),
            });
            
            events.clear();
        }
    };
    
    // Process each event
    for event in &workflow.events {
        // Update application context based on window events
        if let WorkflowEvent::WindowFocusChanged(window_event) = &event.event {
            update_app_context(&mut current_app_context, window_event);
            
            // Split on focus change if configured
            if config.split_on_focus_change && !current_group_events.is_empty() {
                finalize_group(&mut current_group_events, &mut current_app_context);
            }
        }
        
        // Check for time gap
        if let Some(last_time) = last_event_time {
            let time_gap = event.timestamp - last_time;
            
            // Split on significant pause
            if config.split_on_pause && time_gap > config.min_pause_duration && !current_group_events.is_empty() {
                finalize_group(&mut current_group_events, &mut current_app_context);
            }
            // Split on max time gap
            else if time_gap > config.max_time_gap && !current_group_events.is_empty() {
                finalize_group(&mut current_group_events, &mut current_app_context);
            }
        }
        
        // Add event to current group
        current_group_events.push(event.clone());
        last_event_time = Some(event.timestamp);
    }
    
    // Finalize the last group
    if !current_group_events.is_empty() {
        finalize_group(&mut current_group_events, &mut current_app_context);
    }
    
    groups
}

/// Updates the application context based on a window event
fn update_app_context(context: &mut Option<ApplicationContext>, window_event: &WindowEvent) {
    *context = Some(ApplicationContext {
        application_name: window_event.application_name.clone(),
        window_title: window_event.title.clone(),
        process_id: window_event.process_id,
    });
}

/// Analyzes an intent group to determine a more descriptive name
pub fn analyze_intent_group(group: &IntentGroup) -> String {
    // This is a placeholder for more sophisticated analysis
    // In a real implementation, this would use heuristics or ML to determine the intent
    
    if let Some(context) = &group.application_context {
        if let Some(app_name) = &context.application_name {
            // Count different event types
            let mut mouse_clicks = 0;
            let mut key_presses = 0;
            
            for event in &group.events {
                match &event.event {
                    WorkflowEvent::Mouse(mouse_event) => {
                        if matches!(mouse_event.event_type, crate::MouseEventType::Click | crate::MouseEventType::Down) {
                            mouse_clicks += 1;
                        }
                    }
                    WorkflowEvent::Keyboard(keyboard_event) => {
                        if keyboard_event.is_key_down {
                            key_presses += 1;
                        }
                    }
                    _ => {}
                }
            }
            
            // Determine intent based on event counts
            if key_presses > 10 && mouse_clicks < 3 {
                format!("Text input in {}", app_name)
            } else if mouse_clicks > 5 && key_presses < 3 {
                format!("Navigation in {}", app_name)
            } else if mouse_clicks > 0 && key_presses > 0 {
                format!("Form interaction in {}", app_name)
            } else {
                format!("Interaction with {}", app_name)
            }
        } else if let Some(title) = &context.window_title {
            format!("Interaction with window: {}", title)
        } else {
            "Unknown interaction".to_string()
        }
    } else {
        "Unknown interaction".to_string()
    }
}

/// Extracts intent groups from a workflow
pub fn extract_intent_groups(workflow: &RecordedWorkflow) -> Vec<IntentGroup> {
    let config = IntentGroupingConfig::default();
    let groups = group_events(workflow, &config);
    
    // Analyze each group to determine a better name
    groups.into_iter().map(|mut group| {
        group.name = analyze_intent_group(&group);
        group
    }).collect()
} 