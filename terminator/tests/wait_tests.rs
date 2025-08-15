use std::{sync::Arc, thread};

fn start_test_server() -> (String, Arc<tiny_http::Server>) {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let server_arc = Arc::new(server);
    let server_clone = server_arc.clone();

    thread::spawn(move || {
        for request in server_clone.incoming_requests() {
            let html_content = include_str!("wait_test_page.html");
            let header: tiny_http::Header = "Content-Type: text/html".parse().unwrap();
            let response = tiny_http::Response::from_string(html_content).with_header(header);
            request.respond(response).unwrap();
        }
    });

    (format!("http://127.0.0.1:{port}"), server_arc)
}

#[tokio::test]
#[cfg(target_os = "windows")]
async fn test_wait_for_element_on_webpage() {
    use std::time::Duration;

    use terminator::{Desktop, Selector};

    let (server_url, _server) = start_test_server();
    let desktop = Desktop::new(false, false).unwrap();

    // Open the local test page
    let browser_window = desktop.open_url(&server_url, None).unwrap();

    // 1. Test that waiting for a delayed element succeeds
    let locator = browser_window
        .locator(Selector::Role {
            role: "Text".to_string(),
            name: Some("I have arrived!".to_string()),
        })
        .unwrap();

    // Wait for up to 5 seconds. The element should appear after 2.
    let wait_result = locator.wait(Some(Duration::from_secs(5))).await;
    assert!(
        wait_result.is_ok(),
        "wait() should succeed for an element that appears after a delay. Error: {:?}",
        wait_result.err()
    );
    let found_element = wait_result.unwrap();
    assert_eq!(found_element.name().unwrap(), "I have arrived!");

    // 2. Test that waiting for a non-existent element times out
    let locator_non_existent = browser_window
        .locator(Selector::Role {
            role: "Text".to_string(),
            name: Some("non-existent-element".to_string()),
        })
        .unwrap();

    let wait_result_timeout = locator_non_existent
        .wait(Some(Duration::from_secs(1)))
        .await;
    assert!(
        wait_result_timeout.is_err(),
        "wait() should fail for an element that never appears"
    );

    // Verify it's a Timeout error specifically
    match wait_result_timeout.err().unwrap() {
        terminator::AutomationError::Timeout(_) => {
            // This is the expected outcome
        }
        e => panic!("Expected a Timeout error, but got {e:?}"),
    }

    browser_window.close().unwrap();
}
