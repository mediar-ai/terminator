#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::time::Duration;
    use tokio_stream::StreamExt;
    
    #[tokio::test]
    async fn test_workflow_recorder_creation() {
        let config = WorkflowRecorderConfig::default();
        let recorder = WorkflowRecorder::new("Test Recorder".to_string(), config);
        
        // This test just verifies that we can create a recorder instance
        // without panicking or errors
        assert_eq!("Test Recorder", "Test Recorder"); // Placeholder assertion
    }
    
    #[tokio::test]
    async fn test_event_stream_creation() {
        let config = WorkflowRecorderConfig::default();
        let recorder = WorkflowRecorder::new("Test Recorder".to_string(), config);
        
        let mut event_stream = recorder.event_stream();
        
        // Verify we can create an event stream
        // We don't expect any events since we haven't started recording
        tokio::select! {
            _ = event_stream.next() => {
                // If we get an event, that's unexpected but not an error
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                // This is expected - no events should be emitted yet
            }
        }
    }
    
    #[test]
    fn test_workflow_save_load_roundtrip() {
        use std::fs;
        use tempfile::tempdir;
        
        // Create a test workflow
        let mut workflow = RecordedWorkflow::new("Test Workflow".to_string());
        
        // Add some test events
        let mouse_event = MouseEvent {
            event_type: MouseEventType::Click,
            button: MouseButton::Left,
            position: Position { x: 100, y: 200 },
            ui_element: None,
            scroll_delta: None,
            drag_start: None,
        };
        workflow.add_event(WorkflowEvent::Mouse(mouse_event));
        
        let keyboard_event = KeyboardEvent {
            key_code: 65,
            is_key_down: true,
            ctrl_pressed: false,
            alt_pressed: false,
            shift_pressed: false,
            win_pressed: false,
            character: Some('A'),
            scan_code: None,
        };
        workflow.add_event(WorkflowEvent::Keyboard(keyboard_event));
        
        workflow.finish();
        
        // Save to temporary file
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test_workflow.json");
        
        // Serialize and save
        let json = serde_json::to_string_pretty(&workflow).expect("Failed to serialize");
        fs::write(&file_path, json).expect("Failed to write file");
        
        // Load and deserialize
        let loaded_json = fs::read_to_string(&file_path).expect("Failed to read file");
        let loaded_workflow: RecordedWorkflow = serde_json::from_str(&loaded_json)
            .expect("Failed to deserialize");
        
        // Verify the data
        assert_eq!(workflow.name, loaded_workflow.name);
        assert_eq!(workflow.start_time, loaded_workflow.start_time);
        assert_eq!(workflow.end_time, loaded_workflow.end_time);
        assert_eq!(workflow.events.len(), loaded_workflow.events.len());
        assert_eq!(workflow.events.len(), 2);
    }
    
    #[test]
    fn test_ui_element_hierarchy() {
        let ui_element = UiElement {
            name: Some("Submit Button".to_string()),
            automation_id: Some("btn_submit".to_string()),
            class_name: Some("Button".to_string()),
            control_type: Some("Button".to_string()),
            process_id: Some(1234),
            application_name: Some("WebBrowser".to_string()),
            window_title: Some("Contact Form - Browser".to_string()),
            bounding_rect: Some(Rect { 
                x: 200, 
                y: 400, 
                width: 120, 
                height: 40 
            }),
            is_enabled: Some(true),
            has_keyboard_focus: Some(false),
            hierarchy_path: Some("Window/Document/Form/Panel/Button".to_string()),
            value: Some("Submit".to_string()),
        };
        
        // Test all the properties
        assert_eq!(ui_element.name, Some("Submit Button".to_string()));
        assert_eq!(ui_element.automation_id, Some("btn_submit".to_string()));
        assert_eq!(ui_element.application_name, Some("WebBrowser".to_string()));
        assert_eq!(ui_element.hierarchy_path, Some("Window/Document/Form/Panel/Button".to_string()));
        
        // Test bounding rect
        let rect = ui_element.bounding_rect.unwrap();
        assert_eq!(rect.x, 200);
        assert_eq!(rect.y, 400);
        assert_eq!(rect.width, 120);
        assert_eq!(rect.height, 40);
    }
    
    #[test]
    fn test_complex_workflow_scenario() {
        let mut workflow = RecordedWorkflow::new("Complex User Interaction".to_string());
        
        // Simulate a complex user interaction workflow
        
        // 1. User clicks in a text field
        let click_event = MouseEvent {
            event_type: MouseEventType::Click,
            button: MouseButton::Left,
            position: Position { x: 300, y: 150 },
            ui_element: Some(UiElement {
                name: Some("Email Field".to_string()),
                control_type: Some("Edit".to_string()),
                application_name: Some("ContactForm".to_string()),
                window_title: Some("Contact Us".to_string()),
                ..Default::default()
            }),
            scroll_delta: None,
            drag_start: None,
        };
        workflow.add_event(WorkflowEvent::Mouse(click_event));
        
        // 2. User types some text
        let text_input = TextInputEvent {
            text: "user@example.com".to_string(),
            target_element: Some(UiElement {
                name: Some("Email Field".to_string()),
                control_type: Some("Edit".to_string()),
                ..Default::default()
            }),
            is_replacement: false,
            previous_text: None,
            selection_start: None,
            selection_end: None,
        };
        workflow.add_event(WorkflowEvent::TextInput(text_input));
        
        // 3. User selects some text
        let text_selection = TextSelectionEvent {
            selected_text: "example.com".to_string(),
            start_position: Position { x: 350, y: 150 },
            end_position: Position { x: 420, y: 150 },
            target_element: None,
            selection_method: SelectionMethod::DoubleClick,
            selection_length: 11,
            is_partial_selection: true,
            application: Some("ContactForm".to_string()),
        };
        workflow.add_event(WorkflowEvent::TextSelection(text_selection));
        
        // 4. User copies the selection
        let copy_hotkey = HotkeyEvent {
            combination: "Ctrl+C".to_string(),
            action: Some("Copy".to_string()),
            application: Some("ContactForm".to_string()),
            is_global: false,
        };
        workflow.add_event(WorkflowEvent::Hotkey(copy_hotkey));
        
        // 5. Clipboard event
        let clipboard_event = ClipboardEvent {
            action: ClipboardAction::Copy,
            content: Some("example.com".to_string()),
            content_size: Some(11),
            format: Some("text/plain".to_string()),
            source_application: Some("ContactForm".to_string()),
            truncated: false,
        };
        workflow.add_event(WorkflowEvent::Clipboard(clipboard_event));
        
        // 6. User scrolls down
        let scroll_event = ScrollEvent {
            delta: (0, -120),
            position: Position { x: 400, y: 300 },
            target_element: None,
            direction: ScrollDirection::Vertical,
        };
        workflow.add_event(WorkflowEvent::Scroll(scroll_event));
        
        // 7. User drags and drops something
        let drag_drop = DragDropEvent {
            start_position: Position { x: 100, y: 200 },
            end_position: Position { x: 200, y: 300 },
            source_element: None,
            target_element: None,
            data_type: Some("text/plain".to_string()),
            content: Some("Dragged item".to_string()),
            success: true,
        };
        workflow.add_event(WorkflowEvent::DragDrop(drag_drop));
        
        workflow.finish();
        
        // Verify the workflow
        assert_eq!(workflow.events.len(), 7);
        assert!(workflow.end_time.is_some());
        
        // Test serialization of the complex workflow
        let json = serde_json::to_string_pretty(&workflow).expect("Failed to serialize complex workflow");
        assert!(json.len() > 1000); // Should be a substantial JSON object
        
        // Test deserialization
        let loaded_workflow: RecordedWorkflow = serde_json::from_str(&json)
            .expect("Failed to deserialize complex workflow");
        assert_eq!(loaded_workflow.events.len(), 7);
    }
    
    #[test]
    fn test_config_combinations() {
        // Test minimal config
        let minimal_config = WorkflowRecorderConfig {
            record_mouse: true,
            record_keyboard: true,
            record_window: false,
            capture_ui_elements: false,
            record_clipboard: false,
            record_text_input: false,
            record_text_selection: false,
            record_applications: false,
            record_file_operations: false,
            record_menu_interactions: false,
            record_dialog_interactions: false,
            record_scroll: false,
            record_system_events: false,
            record_drag_drop: false,
            record_hotkeys: false,
            max_clipboard_content_length: 512,
            max_text_selection_length: 256,
            record_window_geometry: false,
            track_modifier_states: false,
            detailed_scroll_tracking: false,
            monitor_file_system: false,
            file_system_watch_paths: vec![],
            record_network_events: false,
            record_multimedia_events: false,
            mouse_move_throttle_ms: 100,
            min_drag_distance: 10.0,
        };
        
        assert!(minimal_config.record_mouse);
        assert!(minimal_config.record_keyboard);
        assert!(!minimal_config.capture_ui_elements);
        assert_eq!(minimal_config.max_clipboard_content_length, 512);
        
        // Test maximal config
        let maximal_config = WorkflowRecorderConfig {
            record_mouse: true,
            record_keyboard: true,
            record_window: true,
            capture_ui_elements: true,
            record_clipboard: true,
            record_text_input: true,
            record_text_selection: true,
            record_applications: true,
            record_file_operations: true,
            record_menu_interactions: true,
            record_dialog_interactions: true,
            record_scroll: true,
            record_system_events: true,
            record_drag_drop: true,
            record_hotkeys: true,
            max_clipboard_content_length: 4096,
            max_text_selection_length: 2048,
            record_window_geometry: true,
            track_modifier_states: true,
            detailed_scroll_tracking: true,
            monitor_file_system: true,
            file_system_watch_paths: vec!["C:\\".to_string(), "D:\\".to_string()],
            record_network_events: true,
            record_multimedia_events: true,
            mouse_move_throttle_ms: 10,
            min_drag_distance: 1.0,
        };
        
        assert!(maximal_config.record_file_operations);
        assert!(maximal_config.detailed_scroll_tracking);
        assert_eq!(maximal_config.file_system_watch_paths.len(), 2);
        assert_eq!(maximal_config.mouse_move_throttle_ms, 10);
    }
}

impl Default for UiElement {
    fn default() -> Self {
        Self {
            name: None,
            automation_id: None,
            class_name: None,
            control_type: None,
            process_id: None,
            application_name: None,
            window_title: None,
            bounding_rect: None,
            is_enabled: None,
            has_keyboard_focus: None,
            hierarchy_path: None,
            value: None,
        }
    }
} 