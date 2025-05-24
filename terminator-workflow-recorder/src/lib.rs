//! Workflow Recorder crate for Windows
//!
//! This crate provides functionality to record user interactions with the Windows UI,
//! including mouse clicks, keyboard input, and window focus changes.
//! The recorded workflow can be saved as a JSON file for later playback or analysis.

#![cfg_attr(not(target_os = "windows"), allow(unused))]

pub mod events;
pub mod recorder;
pub mod error;

pub use events::*;
pub use recorder::*;
pub use error::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_copy_trait() {
        let pos1 = Position { x: 100, y: 200 };
        let pos2 = pos1; // This should work now that Position implements Copy
        assert_eq!(pos1.x, pos2.x);
        assert_eq!(pos1.y, pos2.y);
    }

    #[test]
    fn test_rect_creation() {
        let rect = Rect {
            x: 10,
            y: 20,
            width: 800,
            height: 600,
        };
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 800);
        assert_eq!(rect.height, 600);
    }

    #[test]
    fn test_mouse_button_equality() {
        assert_eq!(MouseButton::Left, MouseButton::Left);
        assert_ne!(MouseButton::Left, MouseButton::Right);
        assert_ne!(MouseButton::Right, MouseButton::Middle);
    }

    #[test]
    fn test_keyboard_event_creation() {
        let kb_event = KeyboardEvent {
            key_code: 65, // 'A' key
            is_key_down: true,
            ctrl_pressed: true,
            alt_pressed: false,
            shift_pressed: false,
            win_pressed: false,
            character: Some('A'),
            scan_code: None,
        };
        
        assert_eq!(kb_event.key_code, 65);
        assert!(kb_event.is_key_down);
        assert!(kb_event.ctrl_pressed);
        assert!(!kb_event.alt_pressed);
        assert_eq!(kb_event.character, Some('A'));
    }

    #[test]
    fn test_mouse_event_creation() {
        let ui_element = UiElement {
            name: Some("Test Button".to_string()),
            automation_id: Some("btn_test".to_string()),
            class_name: Some("Button".to_string()),
            control_type: Some("Button".to_string()),
            process_id: Some(1234),
            application_name: Some("TestApp".to_string()),
            window_title: Some("Test Window".to_string()),
            bounding_rect: Some(Rect { x: 0, y: 0, width: 100, height: 30 }),
            is_enabled: Some(true),
            has_keyboard_focus: Some(false),
            hierarchy_path: Some("Window/Panel/Button".to_string()),
            value: Some("Click me".to_string()),
        };

        let mouse_event = MouseEvent {
            event_type: MouseEventType::Click,
            button: MouseButton::Left,
            position: Position { x: 50, y: 15 },
            ui_element: Some(ui_element.clone()),
            scroll_delta: None,
            drag_start: None,
        };

        assert_eq!(mouse_event.event_type, MouseEventType::Click);
        assert_eq!(mouse_event.button, MouseButton::Left);
        assert_eq!(mouse_event.position.x, 50);
        assert_eq!(mouse_event.position.y, 15);
        assert!(mouse_event.ui_element.is_some());
        
        let ui_elem = mouse_event.ui_element.unwrap();
        assert_eq!(ui_elem.name, Some("Test Button".to_string()));
        assert_eq!(ui_elem.application_name, Some("TestApp".to_string()));
    }

    #[test]
    fn test_clipboard_event_creation() {
        let clipboard_event = ClipboardEvent {
            action: ClipboardAction::Copy,
            content: Some("Hello World".to_string()),
            content_size: Some(11),
            format: Some("text/plain".to_string()),
            source_application: Some("Notepad".to_string()),
            truncated: false,
        };

        assert_eq!(clipboard_event.action, ClipboardAction::Copy);
        assert_eq!(clipboard_event.content, Some("Hello World".to_string()));
        assert_eq!(clipboard_event.content_size, Some(11));
        assert!(!clipboard_event.truncated);
    }

    #[test]
    fn test_text_selection_event() {
        let selection_event = TextSelectionEvent {
            selected_text: "Selected text content".to_string(),
            start_position: Position { x: 100, y: 200 },
            end_position: Position { x: 300, y: 200 },
            target_element: None,
            selection_method: SelectionMethod::MouseDrag,
            selection_length: 21,
            is_partial_selection: false,
            application: Some("TextEditor".to_string()),
        };

        assert_eq!(selection_event.selected_text, "Selected text content");
        assert_eq!(selection_event.start_position.x, 100);
        assert_eq!(selection_event.end_position.x, 300);
        assert_eq!(selection_event.selection_length, 21);
        assert!(!selection_event.is_partial_selection);
    }

    #[test]
    fn test_window_event_creation() {
        let window_event = WindowEvent {
            action: WindowAction::FocusGained,
            title: Some("Test Application".to_string()),
            class_name: Some("ApplicationFrameWindow".to_string()),
            process_id: Some(5678),
            application_name: Some("TestApp.exe".to_string()),
            bounds: Some(Rect { x: 100, y: 100, width: 800, height: 600 }),
            previous_bounds: None,
            handle: Some("0x12345678".to_string()),
            parent_handle: None,
            state: Some("normal".to_string()),
        };

        assert_eq!(window_event.action, WindowAction::FocusGained);
        assert_eq!(window_event.title, Some("Test Application".to_string()));
        assert_eq!(window_event.process_id, Some(5678));
    }

    #[test]
    fn test_workflow_recorder_config_default() {
        let config = WorkflowRecorderConfig::default();
        
        assert!(config.record_mouse);
        assert!(config.record_keyboard);
        assert!(config.record_window);
        assert!(config.capture_ui_elements);
        assert!(config.record_clipboard);
        assert!(config.record_text_input);
        assert!(config.record_text_selection);
        assert!(config.record_applications);
        assert!(!config.record_file_operations); // Should be false by default
        assert!(config.record_menu_interactions);
        assert!(config.record_dialog_interactions);
        assert!(config.record_scroll);
        assert!(!config.record_system_events); // Should be false by default
        assert!(config.record_drag_drop);
        assert!(config.record_hotkeys);
        
        assert_eq!(config.max_clipboard_content_length, 1024);
        assert_eq!(config.max_text_selection_length, 512);
        assert_eq!(config.mouse_move_throttle_ms, 50);
        assert_eq!(config.min_drag_distance, 5.0);
    }

    #[test]
    fn test_workflow_recorder_config_custom() {
        let config = WorkflowRecorderConfig {
            record_mouse: true,
            record_keyboard: false,
            record_window: true,
            capture_ui_elements: false,
            record_clipboard: true,
            record_text_input: true,
            record_text_selection: true,
            record_applications: false,
            record_file_operations: true,
            record_menu_interactions: false,
            record_dialog_interactions: true,
            record_scroll: false,
            record_system_events: true,
            record_drag_drop: true,
            record_hotkeys: false,
            max_clipboard_content_length: 2048,
            max_text_selection_length: 1024,
            record_window_geometry: true,
            track_modifier_states: true,
            detailed_scroll_tracking: false,
            monitor_file_system: true,
            file_system_watch_paths: vec!["C:\\test".to_string()],
            record_network_events: false,
            record_multimedia_events: false,
            mouse_move_throttle_ms: 100,
            min_drag_distance: 10.0,
        };

        assert!(config.record_mouse);
        assert!(!config.record_keyboard);
        assert!(config.record_file_operations);
        assert!(config.record_system_events);
        assert_eq!(config.max_clipboard_content_length, 2048);
        assert_eq!(config.max_text_selection_length, 1024);
        assert_eq!(config.mouse_move_throttle_ms, 100);
        assert_eq!(config.min_drag_distance, 10.0);
        assert_eq!(config.file_system_watch_paths, vec!["C:\\test".to_string()]);
    }

    #[test]
    fn test_recorded_workflow_creation() {
        let workflow = RecordedWorkflow::new("Test Workflow".to_string());
        
        assert_eq!(workflow.name, "Test Workflow");
        assert!(workflow.start_time > 0);
        assert!(workflow.end_time.is_none());
        assert!(workflow.events.is_empty());
    }

    #[test]
    fn test_recorded_workflow_add_event() {
        let mut workflow = RecordedWorkflow::new("Test Workflow".to_string());
        
        let mouse_event = MouseEvent {
            event_type: MouseEventType::Click,
            button: MouseButton::Left,
            position: Position { x: 100, y: 200 },
            ui_element: None,
            scroll_delta: None,
            drag_start: None,
        };
        
        workflow.add_event(WorkflowEvent::Mouse(mouse_event));
        
        assert_eq!(workflow.events.len(), 1);
        
        if let WorkflowEvent::Mouse(recorded_mouse_event) = &workflow.events[0].event {
            assert_eq!(recorded_mouse_event.event_type, MouseEventType::Click);
            assert_eq!(recorded_mouse_event.position.x, 100);
        } else {
            panic!("Expected mouse event");
        }
    }

    #[test]
    fn test_recorded_workflow_finish() {
        let mut workflow = RecordedWorkflow::new("Test Workflow".to_string());
        
        assert!(workflow.end_time.is_none());
        
        workflow.finish();
        
        assert!(workflow.end_time.is_some());
        assert!(workflow.end_time.unwrap() >= workflow.start_time);
    }

    #[test]
    fn test_drag_drop_event() {
        let drag_drop_event = DragDropEvent {
            start_position: Position { x: 100, y: 100 },
            end_position: Position { x: 200, y: 200 },
            source_element: None,
            target_element: None,
            data_type: Some("text/plain".to_string()),
            content: Some("Dragged content".to_string()),
            success: true,
        };

        assert_eq!(drag_drop_event.start_position.x, 100);
        assert_eq!(drag_drop_event.end_position.x, 200);
        assert_eq!(drag_drop_event.content, Some("Dragged content".to_string()));
        assert!(drag_drop_event.success);
    }

    #[test]
    fn test_hotkey_event() {
        let hotkey_event = HotkeyEvent {
            combination: "Ctrl+C".to_string(),
            action: Some("Copy".to_string()),
            application: Some("Notepad".to_string()),
            is_global: true,
        };

        assert_eq!(hotkey_event.combination, "Ctrl+C");
        assert_eq!(hotkey_event.action, Some("Copy".to_string()));
        assert!(hotkey_event.is_global);
    }

    #[test]
    fn test_scroll_event() {
        let scroll_event = ScrollEvent {
            delta: (0, -120),
            position: Position { x: 400, y: 300 },
            target_element: None,
            direction: ScrollDirection::Vertical,
        };

        assert_eq!(scroll_event.delta, (0, -120));
        assert_eq!(scroll_event.position.x, 400);
        assert_eq!(scroll_event.direction, ScrollDirection::Vertical);
    }

    #[test]
    fn test_workflow_event_serialization() {
        let mouse_event = MouseEvent {
            event_type: MouseEventType::Click,
            button: MouseButton::Left,
            position: Position { x: 100, y: 200 },
            ui_element: None,
            scroll_delta: None,
            drag_start: None,
        };
        
        let workflow_event = WorkflowEvent::Mouse(mouse_event);
        
        // Test that the event can be serialized to JSON
        let json_result = serde_json::to_string(&workflow_event);
        assert!(json_result.is_ok());
        
        let json = json_result.unwrap();
        assert!(json.contains("Mouse"));
        assert!(json.contains("Click"));
        
        // Test that it can be deserialized back
        let deserialized_result: serde_json::Result<WorkflowEvent> = serde_json::from_str(&json);
        assert!(deserialized_result.is_ok());
    }

    #[test]
    fn test_selection_method_variants() {
        let methods = vec![
            SelectionMethod::MouseDrag,
            SelectionMethod::DoubleClick,
            SelectionMethod::TripleClick,
            SelectionMethod::KeyboardShortcut,
            SelectionMethod::ContextMenu,
        ];

        // Test that all methods can be created and are different
        assert_eq!(methods.len(), 5);
        
        // Test serialization
        for method in methods {
            let json_result = serde_json::to_string(&method);
            assert!(json_result.is_ok());
        }
    }

    #[test]
    fn test_error_types() {
        use crate::error::WorkflowRecorderError;
        
        let init_error = WorkflowRecorderError::InitializationError("Test error".to_string());
        let recording_error = WorkflowRecorderError::RecordingError("Recording failed".to_string());
        let save_error = WorkflowRecorderError::SaveError("Save failed".to_string());
        
        assert!(format!("{}", init_error).contains("Test error"));
        assert!(format!("{}", recording_error).contains("Recording failed"));
        assert!(format!("{}", save_error).contains("Save failed"));
    }
} 