use crate::{AutomationError, Desktop};
use crate::tests::init_tracing;
use std::fs;


#[tokio::test]
#[ignore]
async fn test_get_firefox_window_tree() -> Result<(), AutomationError> {
    init_tracing();
    let desktop = Desktop::new(false, true)?;

    // Try to find the Firefox window by title. 
    // This might need adjustment based on the actual window title.
    let firefox_window_title_contains = "Best"; 
    

    // Now get the tree for the found/active Firefox window.
    // We'll use a common part of Firefox window titles. This might need to be made more robust.
    let window_tree = desktop.get_window_tree_by_title(firefox_window_title_contains)?;
    
    // Write the JSON to a file
    let json_output = serde_json::to_string_pretty(&window_tree).unwrap();
    fs::write("firefox_window_tree.json", json_output).expect("Failed to write JSON to file");
    println!("Window tree written to firefox_window_tree.json");

    assert!(!window_tree.children.is_empty(), "Window tree should have children.");

    Ok(())
} 