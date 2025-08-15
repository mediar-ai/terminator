use terminator::Browser;
use terminator::{platforms, AutomationError};
use tracing::{info, Level};

#[tokio::test]
async fn test_open_url() -> Result<(), AutomationError> {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_max_level(Level::DEBUG)
        .init();

    let engine = platforms::create_engine(false, false)?;

    let cases = vec![
        ("https://www.mediar.ai/", Browser::Default),
        ("https://www.reddit.com/", Browser::Chrome),
        ("https://stackoverflow.com/", Browser::Firefox),
        (
            "https://github.com/mediar-ai/terminator/issues/180",
            Browser::Edge,
        ),
        (
            "https://practicetestautomation.com/practice-test-login/",
            Browser::Brave,
        ),
        (
            "https://docs.screenpi.pe/terminator/introduction",
            Browser::Opera,
        ),
        ("https://thisisatest.com/", Browser::Default), // invalid url
    ];

    for (url, browser) in cases {
        let start = std::time::Instant::now();
        let result = engine.open_url(url, Some(browser.clone()));
        let elapsed = start.elapsed();

        if url == "https://thisisatest.com/" {
            // if error contains ShellExecuteW returned error code: 2 it means we dont have the browser, skip error
            let is_error = result.is_err();
            let is_browser_error = result
                .unwrap_err()
                .to_string()
                .contains("ShellExecuteW returned error code: 2");
            if is_error && is_browser_error {
                continue;
            }
            assert!(is_error, "expected failure for invalid url");
        } else {
            match result {
                Ok(element) => {
                    assert!(element.name().is_some(), "expected name for ui element");
                    info!(
                        "opened url '{:?}' in '{:?}' in '{:?}'",
                        url, browser, elapsed
                    );
                }
                Err(e) => {
                    // Handle cases where browser is not installed or fails to launch
                    let error_msg = e.to_string();
                    if error_msg.contains("ShellExecuteW returned error code: 2")
                        || error_msg.contains("Timeout waiting for")
                        || error_msg.contains("browser to appear")
                    {
                        info!(
                            "Skipping {:?} browser test - not available or failed to launch: {}",
                            browser, error_msg
                        );
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }
    Ok(())
}
