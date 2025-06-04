use terminator_workflow_recorder::*;
use std::time::Duration;
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_workflow_recorder_creation() {
    let config = WorkflowRecorderConfig::default();
    let _recorder = WorkflowRecorder::new("Test Recorder".to_string(), config);
    
    // This test just verifies that we can create a recorder instance
    // without panicking or errors
    assert!(true); // Placeholder assertion
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
        scroll_delta: None,
        drag_start: None,
        metadata: EventMetadata::empty(),
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
        metadata: EventMetadata::empty(),
    };
    workflow.add_event(WorkflowEvent::Keyboard(keyboard_event));
    
    workflow.finish();
    
    // Save to temporary file
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test_workflow.json");
    
    // Serialize and save
    let json = workflow.to_json().expect("Failed to serialize");
    fs::write(&file_path, json).expect("Failed to write file");
    
    // Load and deserialize
    let loaded_json = fs::read_to_string(&file_path).expect("Failed to read file");
    let loaded_workflow = RecordedWorkflow::from_json(&loaded_json)
        .expect("Failed to deserialize");
    
    // Verify the data
    assert_eq!(workflow.name, loaded_workflow.name);
    assert_eq!(workflow.start_time, loaded_workflow.start_time);
    assert_eq!(workflow.end_time, loaded_workflow.end_time);
    assert_eq!(workflow.events.len(), loaded_workflow.events.len());
    assert_eq!(workflow.events.len(), 2);
}

#[test]
fn test_complex_workflow_scenario() {
    let mut workflow = RecordedWorkflow::new("Complex User Interaction".to_string());
    
    // Simulate a complex user interaction workflow
    
    // 1. User clicks in a text field

    // 2. User selects some text
    let text_selection = TextSelectionEvent {
        selected_text: "example.com".to_string(),
        start_position: Position { x: 350, y: 150 },
        end_position: Position { x: 420, y: 150 },
        selection_method: SelectionMethod::DoubleClick,
        selection_length: 11,
        metadata: EventMetadata::empty(),
    };
    workflow.add_event(WorkflowEvent::TextSelection(text_selection));
    
    // 3. User copies the selection
    let copy_hotkey = HotkeyEvent {
        combination: "Ctrl+C".to_string(),
        action: Some("Copy".to_string()),
        is_global: false,
        metadata: EventMetadata::empty(),
    };
    workflow.add_event(WorkflowEvent::Hotkey(copy_hotkey));
    
    // 4. Clipboard event
    let clipboard_event = ClipboardEvent {
        action: ClipboardAction::Copy,
        content: Some("example.com".to_string()),
        content_size: Some(11),
        format: Some("text/plain".to_string()),
        truncated: false,
        metadata: EventMetadata::empty(),
    };
    workflow.add_event(WorkflowEvent::Clipboard(clipboard_event));
    
    // 5. User drags and drops something
    let drag_drop = DragDropEvent {
        start_position: Position { x: 100, y: 200 },
        end_position: Position { x: 200, y: 300 },
        source_element: None,
        data_type: Some("text/plain".to_string()),
        content: Some("Dragged item".to_string()),
        success: true,
        metadata: EventMetadata::empty(),
    };
    workflow.add_event(WorkflowEvent::DragDrop(drag_drop));
    
    workflow.finish();
    
    // Verify the workflow
    assert_eq!(workflow.events.len(), 4);
    assert!(workflow.end_time.is_some());
    
    // Test serialization of the complex workflow
    let json = workflow.to_json().expect("Failed to serialize complex workflow");
    assert!(json.len() > 500); // Should be a substantial JSON object
    
    // Test deserialization
    let loaded_workflow = RecordedWorkflow::from_json(&json)
        .expect("Failed to deserialize complex workflow");
    assert_eq!(loaded_workflow.events.len(), 4);
} 