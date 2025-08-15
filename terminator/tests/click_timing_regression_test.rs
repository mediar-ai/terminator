use std::{sync::Arc, time::Duration};

fn start_test_server() -> (String, Arc<tiny_http::Server>) {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let server_arc = Arc::new(server);
    let server_clone = server_arc.clone();

    std::thread::spawn(move || {
        for request in server_clone.incoming_requests() {
            let html_content = include_str!("click_timing_test_page.html");
            let header: tiny_http::Header = "Content-Type: text/html".parse().unwrap();
            let response = tiny_http::Response::from_string(html_content).with_header(header);
            let _ = request.respond(response);
        }
    });

    (format!("http://127.0.0.1:{port}"), server_arc)
}

#[tokio::test]
#[cfg(target_os = "windows")]
#[ignore]
async fn test_click_hover_layout_shift_flaky() {
    use terminator::{Desktop, Selector};

    let (server_url, _server) = start_test_server();
    let desktop = Desktop::new(false, false).expect("Failed to create Desktop");

    // Open the local test page that triggers a hover-based layout shift
    let browser_window = desktop
        .open_url(&server_url, None)
        .expect("Failed to open test page");

    // Find the target element (a div with role=button and name "Click Area")
    let target = browser_window
        .locator(Selector::Role {
            role: "Button".to_string(),
            name: Some("Click Area".to_string()),
        })
        .expect("Failed to create locator")
        .first(Some(Duration::from_secs(5)))
        .await
        .expect("Failed to find target element");

    // Attempt a single click. On some Windows builds, the mouse move can trigger a hover
    // layout shift before the down event, making the clickable point stale and missing the target.
    let _ = target.click();

    // Give the page a brief moment to process events
    tokio::time::sleep(Duration::from_millis(200)).await;

    // If the click missed, the status will remain "not clicked"
    let clicked_probe = browser_window
        .locator(Selector::Role {
            role: "Text".to_string(),
            name: Some("clicked".to_string()),
        })
        .expect("Failed to create clicked probe locator")
        .first(Some(Duration::from_millis(500)))
        .await;

    // This assertion intentionally expects a failure to reproduce the issue.
    // If it doesn't fail on your system, the bug may not reproduce deterministically.
    assert!(
        clicked_probe.is_err(),
        "Expected first click to miss due to hover-induced layout shift; adjust the page if this passes."
    );
}
