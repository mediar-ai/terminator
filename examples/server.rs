use axum::{
    Router,
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use terminator::{AutomationError, Desktop, Locator, Selector, UIElement};
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tracing::{Level, error, info}; // Import env module

// Shared application state
struct AppState {
    desktop: Arc<Desktop>,
}

// Base request structure with selector chain
#[derive(Deserialize)]
struct ChainedRequest {
    selector_chain: Vec<String>,
    timeout_ms: Option<u64>, // Added timeout
}

// Request structure for typing text (with chain)
#[derive(Deserialize)]
struct TypeTextRequest {
    selector_chain: Vec<String>,
    text: String,
    timeout_ms: Option<u64>, // Added timeout
}

// Request structure for getting text (with chain)
#[derive(Deserialize)]
struct GetTextRequest {
    selector_chain: Vec<String>,
    max_depth: Option<usize>,
    timeout_ms: Option<u64>, // Added timeout
}

// Request structure for pressing a key (with chain)
#[derive(Deserialize)]
struct PressKeyRequest {
    selector_chain: Vec<String>,
    key: String,
    timeout_ms: Option<u64>, // Added timeout
}

// Request structure for opening an application
#[derive(Deserialize)]
struct OpenApplicationRequest {
    app_name: String,
}

// Request structure for opening a URL
#[derive(Deserialize)]
struct OpenUrlRequest {
    url: String,
    browser: Option<String>,
}

// Request structure for opening a file
#[derive(Deserialize)]
struct OpenFileRequest {
    file_path: String,
}

// Request structure for running a command
#[derive(Deserialize)]
struct RunCommandRequest {
    windows_command: Option<String>,
    unix_command: Option<String>,
}

// Request structure for capturing a specific monitor
#[derive(Deserialize)]
struct CaptureMonitorRequest {
    monitor_name: String,
}

// Request structure for OCR on an image path
#[derive(Deserialize)]
struct OcrImagePathRequest {
    image_path: String,
}

// Request structure for OCR on raw screenshot data (base64 encoded)
#[derive(Deserialize)]
struct OcrScreenshotRequest {
    image_base64: String,
    width: u32,
    height: u32,
}

// Request structure for expectations (can often reuse ChainedRequest)
// Add optional timeout
#[derive(Deserialize)]
struct ExpectRequest {
    selector_chain: Vec<String>,
    timeout_ms: Option<u64>,
}

// Specific request for expecting text
#[derive(Deserialize)]
struct ExpectTextRequest {
    selector_chain: Vec<String>,
    expected_text: String,
    max_depth: Option<usize>, // Needed for element.text() call within expect_text_equals
    timeout_ms: Option<u64>,
}

// Add this new request struct
#[derive(Deserialize)]
struct ActivateApplicationRequest {
    app_name: String,
}

// Add this request struct
#[derive(Deserialize)]
struct ActivateBrowserWindowRequest {
    title: String,
}

// Request for finding a window
#[derive(Deserialize, Debug)]
struct FindWindowRequest {
    title_contains: Option<String>,
    timeout_ms: Option<u64>, // Optional timeout
}

// Request for exploring an element's children
#[derive(Deserialize)]
struct ExploreRequest {
    selector_chain: Option<Vec<String>>, // Make selector chain optional (already was)
    timeout_ms: Option<u64>,             // Added timeout
}

// Basic response structure
#[derive(Serialize)]
struct BasicResponse {
    message: String,
}

// Response structure for element details
#[derive(Serialize, Clone)]
struct ElementResponse {
    role: String,
    label: Option<String>,
    id: Option<String>,
    text: Option<String>,
    bounds: Option<(f64, f64, f64, f64)>,
    visible: Option<bool>,
    enabled: Option<bool>,
    focused: Option<bool>,
}

impl ElementResponse {
    fn from_element(element: &UIElement) -> Self {
        let attrs = element.attributes();
        Self {
            role: attrs.role,
            label: attrs.label,
            id: element.id(),
            text: Some(element.text(1).unwrap_or_default()),
            bounds: Some(element.bounds().unwrap_or_default()),
            visible: Some(element.is_visible().unwrap_or_default()),
            enabled: Some(element.is_enabled().unwrap_or_default()),
            focused: Some(element.is_focused().unwrap_or_default()),
        }
    }
}

// Response structure for click action
#[derive(Serialize)]
struct ClickResponse {
    method: String,
    coordinates: Option<(f64, f64)>,
    details: String,
}

impl From<terminator::ClickResult> for ClickResponse {
    fn from(result: terminator::ClickResult) -> Self {
        Self {
            method: result.method,
            coordinates: result.coordinates,
            details: result.details,
        }
    }
}

// Response structure for text content
#[derive(Serialize)]
struct TextResponse {
    text: String,
}

// Response structure for multiple elements
#[derive(Serialize)]
struct ElementsResponse {
    elements: Vec<ElementResponse>,
}

// Response structure for element attributes
#[derive(Serialize)]
struct AttributesResponse {
    role: String,
    label: Option<String>,
    value: Option<String>,
    description: Option<String>,
    properties: HashMap<String, Option<serde_json::Value>>,
    id: Option<String>,
}

// Response structure for element bounds
#[derive(Serialize)]
struct BoundsResponse {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

// Response structure for boolean results
#[derive(Serialize)]
struct BooleanResponse {
    result: bool,
}

// Response structure for command output
#[derive(Serialize)]
struct CommandOutputResponse {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
}

impl From<terminator::CommandOutput> for CommandOutputResponse {
    fn from(output: terminator::CommandOutput) -> Self {
        Self {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.exit_status,
        }
    }
}

// Response structure for screenshot result (with base64 encoded image)
#[derive(Serialize)]
struct ScreenshotResponse {
    image_base64: String,
    width: u32,
    height: u32,
}

impl TryFrom<terminator::ScreenshotResult> for ScreenshotResponse {
    type Error = ApiError; // Or a more specific error

    fn try_from(result: terminator::ScreenshotResult) -> Result<Self, Self::Error> {
        // Encode image data to base64
        let base64_image = BASE64_STANDARD.encode(&result.image_data);
        Ok(Self {
            image_base64: base64_image,
            width: result.width,
            height: result.height,
        })
    }
}

// Response structure for OCR result
#[derive(Serialize)]
struct OcrResponse {
    text: String,
}

// Response structure for exploration result
#[derive(Serialize)]
struct ExploredElementDetail {
    role: String,
    name: Option<String>, // Use 'name' consistently for the primary label/text
    id: Option<String>,
    bounds: Option<BoundsResponse>, // Include bounds for spatial context
    // Add other potentially useful attributes
    value: Option<String>,
    description: Option<String>,
    text: Option<String>,
    parent_id: Option<String>,
    children_ids: Vec<String>,
    // Maybe a suggested selector string? e.g., "role:button name:'Submit'"
    suggested_selector: String,
}

#[derive(Serialize)]
struct ExploreResponse {
    parent: ElementResponse, // Details of the parent element explored
    children: Vec<ExploredElementDetail>, // List of direct children details
}

// Custom error type for API responses
#[derive(Debug)]
enum ApiError {
    Automation(AutomationError),
    BadRequest(String),
}

// Implement the From trait to allow automatic conversion
impl From<AutomationError> for ApiError {
    fn from(err: AutomationError) -> Self {
        // Enhance ElementNotFound errors with more context if possible
        if let AutomationError::ElementNotFound(msg) = &err {
            // Check if the message already contains context hints
            if !msg.contains("within parent") && !msg.contains("Found windows:") {
                // Attempt to provide default context (this is basic, ideally context is added at source)
                return ApiError::Automation(AutomationError::ElementNotFound(format!(
                    "{} (context: Root)",
                    msg
                )));
            }
        }
        ApiError::Automation(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::Automation(err) => {
                tracing::error!("Automation error: {:?}", err);
                let code = match err {
                    AutomationError::ElementNotFound(_) => StatusCode::NOT_FOUND, // 404
                    AutomationError::Timeout(_) => StatusCode::REQUEST_TIMEOUT,   // 408
                    AutomationError::UnsupportedOperation(_) => StatusCode::NOT_IMPLEMENTED, // 501
                    AutomationError::InvalidArgument(_) => StatusCode::BAD_REQUEST, // 400
                    _ => StatusCode::INTERNAL_SERVER_ERROR,                       // 500 for others
                };
                (code, format!("Automation error: {}", err))
            }
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        (
            status,
            Json(BasicResponse {
                message: error_message,
            }),
        )
            .into_response()
    }
}

async fn root() -> &'static str {
    "Terminator API Server Ready"
}

// Helper to get timeout duration from optional ms
fn get_timeout(timeout_ms: Option<u64>) -> Option<Duration> {
    timeout_ms.map(Duration::from_millis)
}

// Helper function to create a locator from the full chain
fn create_locator_for_chain(
    state: &Arc<AppState>,
    selector_chain: &[String],
    // Add optional base element if needed later for within() functionality directly here
    // base_element: Option<&UIElement>
) -> Result<Locator, ApiError> {
    if selector_chain.is_empty() {
        return Err(ApiError::BadRequest(
            "selector_chain cannot be empty".to_string(),
        ));
    }

    let selectors: Vec<Selector> = selector_chain.iter().map(|s| s.as_str().into()).collect();

    // Determine the starting point: Desktop root or a specified base element
    // For now, always start from desktop root as SDK manages 'within' via chaining
    let mut locator = state.desktop.locator(selectors[0].clone());

    // Chain subsequent locators
    for selector in selectors.iter().skip(1) {
        locator = locator.locator(selector.clone());
    }

    Ok(locator)
}

// Handler for finding an element
async fn first(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChainedRequest>,
) -> Result<Json<ElementResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to find element (first)");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms); // Convert Option<u64> to Option<Duration>

    // Pass the timeout to the locator's wait method (requires core library update)
    match locator.wait(timeout).await {
        Ok(element) => {
            info!(element_id = ?element.id(), role = element.role(), "Element found (first)");
            Ok(Json(ElementResponse::from_element(&element)))
        }
        Err(e) => {
            error!("Failed finding element (first): {}", e); // Use error! macro
            Err(e.into()) // Convert AutomationError to ApiError
        }
    }
}

// Handler for clicking an element
async fn click_element(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChainedRequest>,
) -> Result<Json<ClickResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to click element");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    // Pass timeout to click (requires core library update)
    match locator.click(timeout).await {
        Ok(result) => {
            info!("Element clicked successfully");
            Ok(Json(result.into()))
        }
        Err(e) => {
            error!("Failed to click element: {}", e);
            Err(e.into())
        }
    }
}

// Handler for typing text into an element
async fn type_text_into_element(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TypeTextRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, text = %payload.text, timeout = ?payload.timeout_ms, "Attempting to type text");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    // Pass timeout to type_text (requires core library update)
    match locator.type_text(&payload.text, timeout).await {
        Ok(_) => {
            info!("Text typed successfully");
            Ok(Json(BasicResponse {
                message: "Text typed successfully".to_string(),
            }))
        }
        Err(e) => {
            error!("Failed to type text into element: {}", e);
            Err(e.into())
        }
    }
}

// Handler for getting text from an element
async fn get_element_text(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GetTextRequest>,
) -> Result<Json<TextResponse>, ApiError> {
    let max_depth = payload.max_depth.unwrap_or(5);
    info!(chain = ?payload.selector_chain, max_depth, timeout = ?payload.timeout_ms, "Attempting to get text");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    // Pass timeout to text (requires core library update)
    match locator.text(max_depth, timeout).await {
        Ok(text) => {
            info!("Text retrieved successfully (length: {})", text.len());
            Ok(Json(TextResponse { text }))
        }
        Err(e) => {
            error!("Failed to get text from element: {}", e);
            Err(e.into())
        }
    }
}

// Handler for finding multiple elements
async fn all(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChainedRequest>,
) -> Result<Json<ElementsResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to find all elements");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    // Pass timeout to all (requires core library update)
    // Note: 'all' might not inherently wait like 'first', timeout might apply
    // differently (e.g., timeout for finding the parent context).
    match locator.all(timeout).await {
        Ok(elements) => {
            info!("Found {} elements matching chain", elements.len());
            let response_elements = elements.iter().map(ElementResponse::from_element).collect();
            Ok(Json(ElementsResponse {
                elements: response_elements,
            }))
        }
        Err(e) => {
            error!("Failed to find all elements: {}", e);
            Err(e.into())
        }
    }
}

// Handler for getting element attributes
async fn get_element_attributes(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChainedRequest>,
) -> Result<Json<AttributesResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to get attributes");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    // First, wait for the element to get its handle
    let element = locator.wait(timeout).await?;

    // Now get the ID from the element handle
    let element_id = element.id();

    // Then get the attributes
    let attrs = element.attributes(); // This call is synchronous and doesn't need await/timeout here

    info!("Attributes retrieved successfully for element ID: {:?}", element_id);

    // Construct and return the response
    Ok(Json(AttributesResponse {
        role: attrs.role,
        label: attrs.label,
        value: attrs.value,
        description: attrs.description,
        properties: attrs.properties,
        id: element_id, // Use the ID obtained from the element
    }))
    // Note: The original 'match locator.attributes(timeout).await' logic was incorrect
    // because locator didn't have .attributes() and element.attributes() is sync.
}

// Handler for getting element bounds
async fn get_element_bounds(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChainedRequest>,
) -> Result<Json<BoundsResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to get bounds");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    // Pass timeout to bounds (requires core library update)
    match locator.bounds(timeout).await {
        Ok((x, y, width, height)) => {
            info!("Bounds retrieved successfully");
            Ok(Json(BoundsResponse {
                x,
                y,
                width,
                height,
            }))
        }
        Err(e) => {
            error!("Failed to get bounds: {}", e);
            Err(e.into())
        }
    }
}

// Handler for checking if an element is visible
async fn is_element_visible(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChainedRequest>,
) -> Result<Json<BooleanResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to check visibility");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    // Pass timeout to is_visible (requires core library update)
    match locator.is_visible(timeout).await {
        Ok(result) => {
            info!("Visibility check successful: {}", result);
            Ok(Json(BooleanResponse { result }))
        }
        Err(e) => {
            // Distinguish between element not found during wait vs. error calling is_visible
            if matches!(e, AutomationError::Timeout(_)) {
                info!(
                    "Element not found or timed out while checking visibility: {}",
                    e
                );
                // Return false if the element wasn't found or visible within timeout
                Ok(Json(BooleanResponse { result: false }))
            } else if matches!(e, AutomationError::ElementNotFound(_)) {
                // This case might occur if the element disappears *after* being found but *before* visibility check
                info!("Element disappeared while checking visibility: {}", e);
                Ok(Json(BooleanResponse { result: false })) // Treat disappeared as not visible
            } else {
                error!("Failed to check visibility: {}", e);
                Err(e.into())
            }
        }
    }
}

// Handler for pressing a key on an element
async fn press_key_on_element(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PressKeyRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, key = %payload.key, timeout = ?payload.timeout_ms, "Attempting to press key");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    // Pass timeout to press_key (requires core library update)
    match locator.press_key(&payload.key, timeout).await {
        Ok(_) => {
            info!("Key pressed successfully");
            Ok(Json(BasicResponse {
                message: "Key pressed successfully".to_string(),
            }))
        }
        Err(e) => {
            error!("Failed to press key: {}", e);
            Err(e.into())
        }
    }
}

// Handler for opening an application (returns basic info, no handle)
async fn open_application(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OpenApplicationRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(app_name = %payload.app_name, "Attempting to open application");
    // Desktop::open_application is sync, wrap if needed or keep as is
    match state.desktop.open_application(&payload.app_name) {
        Ok(_) => {
            info!("Application '{}' opened command issued", payload.app_name);
            // Maybe add a short delay or wait for window appearance?
            // For now, just return success message immediately.
            Ok(Json(BasicResponse {
                message: format!("Application '{}' open command issued", payload.app_name),
            }))
        }
        Err(e) => {
            error!("Failed to open application '{}': {}", payload.app_name, e);
            Err(e.into())
        }
    }
}

// Handler for opening a URL (returns basic info, no handle)
async fn open_url(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OpenUrlRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(url = %payload.url, browser = ?payload.browser, "Attempting to open URL");
    match state
        .desktop
        .open_url(&payload.url, payload.browser.as_deref())
    {
        Ok(_) => {
            info!("URL '{}' opened command issued", payload.url);
            Ok(Json(BasicResponse {
                message: format!("URL '{}' open command issued", payload.url),
            }))
        }
        Err(e) => {
            error!("Failed to open URL '{}': {}", payload.url, e);
            Err(e.into())
        }
    }
}

// Handler for finding a window by criteria
async fn find_window_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<FindWindowRequest>,
) -> Result<Json<ElementResponse>, ApiError> {
    info!(criteria = ?payload, "Attempting to find window");
    let timeout = get_timeout(payload.timeout_ms); // Optional timeout

    // Call the underlying desktop method (needs implementation)
    match state
        .desktop
        .find_window_by_criteria(
            payload.title_contains.as_deref(),
            timeout, // Pass timeout
        )
        .await
    {
        // Make sure find_window_by_criteria is async
        Ok(window_element) => {
            info!(
                "Window found successfully: role={}, label={:?}",
                window_element.role(),
                window_element.attributes().label
            );
            Ok(Json(ElementResponse::from_element(&window_element)))
        }
        Err(e) => {
            error!("Failed to find window with criteria {:?}: {}", payload, e);
            Err(e.into())
        }
    }
}

// Handler for exploring an element's children
async fn explore_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExploreRequest>,
) -> Result<Json<ExploreResponse>, ApiError> {
    // Handle case where selector_chain might be null/empty for exploring root
    match &payload.selector_chain {
        Some(chain) if !chain.is_empty() => {
            // --- Existing logic for exploring a specific element --- 
            let locator = create_locator_for_chain(&state, chain)?;
            let timeout = get_timeout(payload.timeout_ms);
            info!(chain = ?payload.selector_chain, timeout = ?timeout, "Attempting to explore element");

            // Wait for the parent element first, using the timeout
            let parent_element = locator.wait(timeout).await.map_err(ApiError::from)?;
            let parent_response = ElementResponse::from_element(&parent_element);
            let children_elements = parent_element.children().map_err(ApiError::from)?;

            info!(
                "Exploration successful, found {} children for parent role={}, label={:?}",
                children_elements.len(),
                parent_response.role,
                parent_response.label
            );

            let detailed_children: Vec<ExploredElementDetail> = children_elements
                .into_iter() // Consume the vector
                .map(|child_element| {
                    let attrs = child_element.attributes();
                    let bounds_res = child_element.bounds();
                    let bounds = match bounds_res {
                        Ok((x, y, width, height)) => Some(BoundsResponse {
                            x,
                            y,
                            width,
                            height,
                        }),
                        Err(_) => None,
                    };
                    let text = child_element.text(1).unwrap_or_default();
                    let child_id = child_element.id(); // Get ID once
                    let child_role = attrs.role.clone(); // Clone role
                    let child_name = attrs.label.clone(); // Clone name

                    let suggested_selector =
                        match (child_role.as_str(), &child_id, &child_name) {
                            (_, Some(id), _) if !id.is_empty() => format!("#{}", id),
                            (_, _, Some(name)) if !name.is_empty() => {
                                format!("name:\"{}\"", name.replace('"', "\\\"")) // Correct escaping for name
                            }
                            (role, _, _) => format!("role:{}", role),
                        };

                    // Get children IDs for this child (can be empty)
                    let grandchildren_ids = child_element.children()
                        .map(|gc| gc.into_iter().filter_map(|g| g.id()).collect())
                        .unwrap_or_else(|_| Vec::new());

                    ExploredElementDetail {
                        role: child_role,
                        name: child_name,
                        id: child_id,
                        bounds,
                        value: attrs.value,
                        description: attrs.description,
                        text: Some(text),
                        parent_id: parent_element.id(), // ID of the element being explored
                        children_ids: grandchildren_ids,
                        suggested_selector,
                    }
                })
                .collect();

            Ok(Json(ExploreResponse {
                parent: parent_response,
                children: detailed_children,
            }))
            // --- End of existing logic ---
        }
        _ => {
            // --- New logic for exploring the screen (root) ---
            info!("Exploring screen (getting top-level applications)");

            // Create the dummy root parent response
            let root_parent = ElementResponse {
                role: "root".to_string(),
                label: None,
                id: None,
                text: Some("Screen Root".to_string()), // More descriptive text
                bounds: Some((0.0, 0.0, 0.0, 0.0)), // Bounds are not applicable
                visible: Some(true), // Root is conceptually always visible
                enabled: Some(true), // Root is conceptually always enabled
                focused: Some(false), // Root itself cannot be focused
            };

            // Get top-level applications/windows
            let applications = state.desktop.applications().map_err(ApiError::from)?;
            info!("Found {} top-level applications/windows", applications.len());

            let detailed_children: Vec<ExploredElementDetail> = applications
                .into_iter()
                .map(|app_element| {
                    let attrs = app_element.attributes();
                    let bounds_res = app_element.bounds();
                    let bounds = match bounds_res {
                        Ok((x, y, width, height)) => Some(BoundsResponse {
                            x,
                            y,
                            width,
                            height,
                        }),
                        Err(_) => None,
                    };
                    let text = app_element.text(1).unwrap_or_default();
                    let app_id = app_element.id();
                    let app_role = attrs.role.clone();
                    let app_name = attrs.label.clone();

                    let suggested_selector =
                        match (app_role.as_str(), &app_id, &app_name) {
                            (_, Some(id), _) if !id.is_empty() => format!("#{}", id),
                            (_, _, Some(name)) if !name.is_empty() => {
                                format!("name:\"{}\"", name.replace('"', "\\\""))
                            }
                            (role, _, _) => format!("role:{}", role),
                        };
                    
                    // Get children IDs for this app window (can be empty)
                    let children_ids = app_element.children()
                        .map(|gc| gc.into_iter().filter_map(|g| g.id()).collect())
                        .unwrap_or_else(|_| Vec::new());

                    ExploredElementDetail {
                        role: app_role,
                        name: app_name,
                        id: app_id,
                        bounds,
                        value: attrs.value,
                        description: attrs.description,
                        text: Some(text),
                        parent_id: None, // Top-level windows have no UI parent in this context
                        children_ids, // IDs of direct children of the window
                        suggested_selector,
                    }
                })
                .collect();

            Ok(Json(ExploreResponse {
                parent: root_parent,
                children: detailed_children,
            }))
            // --- End of new logic ---
        }
    }
}

// Handler for opening a file
async fn open_file(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OpenFileRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(file_path = %payload.file_path, "Attempting to open file");
    match state.desktop.open_file(&payload.file_path) {
        Ok(_) => {
            info!("File '{}' open command issued", payload.file_path);
            Ok(Json(BasicResponse {
                message: format!("File '{}' open command issued", payload.file_path),
            }))
        }
        Err(e) => {
            error!("Failed to open file '{}': {}", payload.file_path, e);
            Err(e.into())
        }
    }
}

// Handler for running a command
async fn run_command(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RunCommandRequest>,
) -> Result<Json<CommandOutputResponse>, ApiError> {
    info!(windows = ?payload.windows_command, unix = ?payload.unix_command, "Attempting to run command");
    // Ensure at least one command is provided based on OS or handle error?
    // Current implementation relies on the core library to handle None correctly.
    match state
        .desktop
        .run_command(
            payload.windows_command.as_deref(),
            payload.unix_command.as_deref(),
        )
        .await
    {
        // Make async
        Ok(output) => {
            info!(
                "Command executed successfully, exit_code={:?}",
                output.exit_status
            );
            Ok(Json(output.into()))
        }
        Err(e) => {
            error!("Failed to run command: {}", e);
            Err(e.into())
        }
    }
}

// Handler for capturing the primary screen and performing OCR
async fn capture_screen(
    State(state): State<Arc<AppState>>,
) -> Result<Json<OcrResponse>, ApiError> {
    info!("Attempting to capture primary screen and perform OCR");
    // 1. Capture screen
    let screenshot_result = state.desktop.capture_screen().await.map_err(ApiError::from)?;
    info!("Screen captured successfully, performing OCR...");

    // 2. Perform OCR directly
    match state.desktop.ocr_screenshot(&screenshot_result).await {
        Ok(text) => {
            info!("OCR performed successfully.");
            Ok(Json(OcrResponse { text })) // Return OCR text directly
        }
        Err(e) => {
            error!("Failed to perform OCR after capture: {}", e);
            Err(e.into())
        }
    }
}

// Handler for capturing a specific monitor
async fn capture_monitor(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CaptureMonitorRequest>,
) -> Result<Json<ScreenshotResponse>, ApiError> {
    info!(monitor_name = %payload.monitor_name, "Attempting to capture monitor");
    match state
        .desktop
        .capture_monitor_by_name(&payload.monitor_name)
        .await
    {
        // Make async
        Ok(screenshot_result) => {
            info!("Monitor captured successfully");
            ScreenshotResponse::try_from(screenshot_result)
                .map(Json)
                .map_err(|e| {
                    error!("Failed to encode screenshot: {:?}", e);
                    ApiError::BadRequest("Failed to encode screenshot data".to_string())
                })
        }
        Err(e) => {
            error!(
                "Failed to capture monitor '{}': {}",
                payload.monitor_name, e
            );
            Err(e.into())
        }
    }
}

// Handler for performing OCR on an image path
async fn ocr_image_path(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OcrImagePathRequest>,
) -> Result<Json<OcrResponse>, ApiError> {
    info!(image_path = %payload.image_path, "Attempting OCR on image path");
    match state.desktop.ocr_image_path(&payload.image_path).await {
        Ok(text) => {
            info!(
                "OCR performed successfully on path '{}'",
                payload.image_path
            );
            Ok(Json(OcrResponse { text }))
        }
        Err(e) => {
            error!(
                "Failed to perform OCR on path '{}': {}",
                payload.image_path, e
            );
            Err(e.into())
        }
    }
}

// Handler for activating an application by name (top-level)
async fn activate_application_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ActivateApplicationRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(app_name = %payload.app_name, "Attempting to activate application by name");
    // activate_application is sync
    match state.desktop.activate_application(&payload.app_name) {
        Ok(_) => {
            info!(
                "Application '{}' activated successfully by name",
                payload.app_name
            );
            Ok(Json(BasicResponse {
                message: format!("Application '{}' activated by name", payload.app_name),
            }))
        }
        Err(e) => {
            error!(
                "Failed to activate application '{}' by name: {}",
                payload.app_name, e
            );
            Err(e.into())
        }
    }
}

// Handler for activating a browser window by title (top-level)
async fn activate_browser_window_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ActivateBrowserWindowRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(title = %payload.title, "Attempting to activate browser window by title");
    // activate_browser_window_by_title is sync
    match state
        .desktop
        .activate_browser_window_by_title(&payload.title)
    {
        Ok(_) => {
            info!(
                "Browser window containing title '{}' activated",
                payload.title
            );
            Ok(Json(BasicResponse {
                message: format!(
                    "Browser window containing title '{}' activated",
                    payload.title
                ),
            }))
        }
        Err(e) => {
            error!(
                "Failed to activate browser window by title '{}': {}",
                payload.title, e
            );
            Err(e.into())
        }
    }
}

// --- NEW EXPECT HANDLERS --- // Restored

async fn expect_element_visible(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExpectRequest>,
) -> Result<Json<ElementResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to expect element visible");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms); // Uses Locator's default if None

    match locator.expect_visible(timeout).await {
        Ok(element) => {
            info!("Element found and is visible");
            Ok(Json(ElementResponse::from_element(&element)))
        }
        Err(e) => {
            error!("Expect visible failed: {}", e);
            Err(e.into()) // Includes Timeout error
        }
    }
}

async fn expect_element_enabled(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExpectRequest>,
) -> Result<Json<ElementResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to expect element enabled");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    match locator.expect_enabled(timeout).await {
        Ok(element) => {
            info!("Element found and is enabled");
            Ok(Json(ElementResponse::from_element(&element)))
        }
        Err(e) => {
            error!("Expect enabled failed: {}", e);
            Err(e.into())
        }
    }
}

async fn expect_element_text_equals(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExpectTextRequest>,
) -> Result<Json<ElementResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, text = %payload.expected_text, depth = ?payload.max_depth, timeout = ?payload.timeout_ms, "Attempting to expect text equals");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let max_depth = payload.max_depth.unwrap_or(5);
    let timeout = get_timeout(payload.timeout_ms);

    match locator
        .expect_text_equals(&payload.expected_text, max_depth, timeout)
        .await
    {
        Ok(element) => {
            info!("Element found and text matches");
            let attrs = element.attributes();
            Ok(Json(ElementResponse {
                role: attrs.role,
                label: attrs.label,
                id: element.id(),
                text: Some(payload.expected_text.to_string()),
                bounds: Some(element.bounds().unwrap_or_default()),
                visible: Some(element.is_visible().unwrap_or_default()),
                enabled: Some(element.is_enabled().unwrap_or_default()),
                focused: Some(element.is_focused().unwrap_or_default()),
            }))
        }
        Err(e) => {
            error!("Expect text equals failed: {}", e);
            Err(e.into())
        }
    }
}

// --- HANDLER for activating app window via element --- // Restored

// Add the placeholder /activate_app handler if needed, or remove from SDK
async fn activate_app_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChainedRequest>, // Reuse ChainedRequest with timeout
) -> Result<Json<BasicResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to activate app via element locator");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    // 1. Wait for the element
    let element = locator.wait(timeout).await?; // Assign the result of wait to element

    // 2. Get the containing application/window from the element (needs core implementation)
    // Example: let app_element = element.containing_application()?; // Needs method on UIElement/Impl

    // 3. Activate the application (using existing desktop method or a new one)
    // Example: state.desktop.activate_application_by_element(&app_element)?;
    // Or maybe: element.activate_window()?; (using existing trait method)
    match element.activate_window() { // Now `element` is defined
        // Assuming activate_window brings app to front
        Ok(_) => {
            info!("Application window containing element activated successfully");
            Ok(Json(BasicResponse {
                message: "Application window activated successfully".to_string(),
            }))
        }
        Err(e) => {
            error!("Failed to activate application window: {}", e);
            Err(ApiError::from(e)) // Convert AutomationError to ApiError
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use tracing subscriber with settings appropriate for environment
    tracing_subscriber::fmt() // Use fmt subscriber
        // Consider .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()) for RUST_LOG control
        .with_max_level(Level::INFO) // Default to INFO, can override with RUST_LOG=debug
        .init();

    info!("Initializing Terminator server...");

    // Initialize the Desktop instance
    // Make Desktop::new async if it performs async operations internally
    let desktop = Desktop::new(false, true)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let shared_state = Arc::new(AppState {
        desktop: Arc::new(desktop),
    });
    info!("Desktop automation backend initialized.");

    // Define a permissive CORS policy
    let cors = CorsLayer::new()
        .allow_origin(Any) // Allows any origin
        .allow_methods(Any) // Allows any method (GET, POST, etc.)
        .allow_headers(Any); // Allows any header

    // Define request body limit (e.g., 50MB)
    const BODY_LIMIT: usize = 500000 * 1024 * 1024; 

    // Build our application with routes and layers
    let app = Router::new()
        .route("/", get(root))
        // Core Locator Actions
        .route("/first", post(first))
        .route("/all", post(all))
        .route("/click", post(click_element))
        .route("/type_text", post(type_text_into_element))
        .route("/get_text", post(get_element_text))
        .route("/get_attributes", post(get_element_attributes))
        .route("/get_bounds", post(get_element_bounds))
        .route("/is_visible", post(is_element_visible))
        .route("/press_key", post(press_key_on_element))
        // New Exploration/Finding Routes
        .route("/find_window", post(find_window_handler)) // New
        .route("/explore", post(explore_handler)) // New
        // Top-Level Desktop Actions
        .route("/open_application", post(open_application))
        .route("/open_url", post(open_url))
        .route("/open_file", post(open_file))
        .route("/run_command", post(run_command))
        .route("/capture_screen", post(capture_screen))
        .route("/capture_monitor", post(capture_monitor))
        // Activation Actions
        .route("/activate_application", post(activate_application_handler)) // Activate by name
        .route(
            "/activate_browser_window",
            post(activate_browser_window_handler),
        ) // Activate browser by title
        .route("/activate_app", post(activate_app_handler)) // Added route for activate via element
        // Expectation Actions
        .route("/expect_visible", post(expect_element_visible))
        .route("/expect_enabled", post(expect_element_enabled))
        .route("/expect_text_equals", post(expect_element_text_equals))
        // OCR Actions
        .route("/ocr_image_path", post(ocr_image_path))
        // State and Layers
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT))
        .layer(cors)
        .with_state(shared_state);

    // Determine port
    let default_port = 9375;
    let port: u16 = env::var("PORT")
        .ok() // Convert Result to Option
        .and_then(|s| s.parse().ok()) // Try parsing the string to u16
        .unwrap_or(default_port); // Use default if env var not set or invalid

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("Server listening on http://{} with {}MB body limit", addr, BODY_LIMIT / (1024 * 1024));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    info!("Server shutting down."); // Added shutdown message

    Ok(())
}
