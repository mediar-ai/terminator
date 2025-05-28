use crate::{AutomationError, Desktop};
use crate::tests::init_tracing;


#[tokio::test]
#[ignore]
async fn test_get_firefox_window_tree() -> Result<(), AutomationError> {
    init_tracing();
    let desktop = Desktop::new(false, true).await?;

    // Try to find the Firefox window by title. 
    // This might need adjustment based on the actual window title.
    let firefox_window_title_contains = "Air"; 
    

    // Now get the tree for the found/active Firefox window.
    // We'll use a common part of Firefox window titles. This might need to be made more robust.
    let window_tree = desktop.get_window_tree_by_title(firefox_window_title_contains)?;
    println!("Window tree: {:?}", window_tree);

    assert!(!window_tree.children.is_empty(), "Window tree should have children.");
    assert!(window_tree.attributes.name.as_ref().map_or(false, |name| name.contains("Firefox")), "Window name should contain 'Firefox'");

    Ok(())
} 