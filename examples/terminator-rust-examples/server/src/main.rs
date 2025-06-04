use axum::{
    Router,
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env};
use std::sync::Arc;
use std::time::{Duration, Instant};
use terminator::{AutomationError, Desktop, Locator, Selector, UIElement};
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tracing::{error, info, instrument, debug};
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

// Cache entry with timestamp for expiration
struct CacheEntry {
    element: UIElement,
    last_accessed: Instant,
}

// Shared application state
struct AppState {
    desktop: Arc<Desktop>,
    element_cache: Arc<tokio::sync::RwLock<HashMap<String, CacheEntry>>>,
    cache_ttl: Duration,
}

// Add DetailLevel enum
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum DetailLevel {
    Minimal,
    Standard,
    Full,
}

impl Default for DetailLevel {
    fn default() -> Self {
        DetailLevel::Full
    }
}

impl AppState {
    fn new(desktop: Arc<Desktop>) -> Self {
        Self {
            desktop,
            element_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(300), // 300 second TTL by default
        }
    }

    // Helper to get cache key from selector chain, now returns Option<String>
    // Only use the last element if it's an ID selector
    fn get_cache_key(selector_chain: &[String]) -> Option<String> {
        if let Some(last_selector) = selector_chain.last() {
            if last_selector.starts_with('#') {
                debug!(last_selector = %last_selector, "Valid cache key (ID selector): {}", last_selector);
                return Some(last_selector.clone());
            } else {
                debug!(original_chain = ?selector_chain, last_selector = %last_selector, "Invalid cache key (last element is not an ID selector). Caching/retrieval will be skipped for this chain.");
                return None;
            }
        }
        debug!(original_chain = ?selector_chain, "Empty selector chain or invalid structure. Caching/retrieval will be skipped.");
        None
    }

    // Helper to get element from cache
    async fn get_cached_element(&self, selector_chain: &[String]) -> Option<UIElement> {
        if let Some(cache_key) = Self::get_cache_key(selector_chain) {
            let mut cache = self.element_cache.write().await;
            debug!(cache_key = %cache_key, "Attempting to retrieve element from cache using derived key for chain: {:?}", selector_chain);

            if let Some(entry) = cache.get_mut(&cache_key) {
                if entry.last_accessed.elapsed() < self.cache_ttl {
                    entry.last_accessed = Instant::now();
                    debug!(cache_key = %cache_key, "Cache hit: Element found and not expired for chain: {:?}", selector_chain);
                    return Some(entry.element.clone());
                } else {
                    debug!(cache_key = %cache_key, "Cache miss: Element found but expired for chain: {:?}. Removing from cache.", selector_chain);
                    cache.remove(&cache_key);
                }
            } else {
                debug!(cache_key = %cache_key, "Cache miss: Element not found in cache for chain: {:?}", selector_chain);
            }
        } else {
            // Reason for skipping is logged by get_cache_key
            debug!(original_chain = ?selector_chain, "Cache lookup skipped for this chain as it does not qualify for caching by ID selector rule.");
        }
        None
    }

    // Helper to store element in cache
    async fn cache_element(&self, selector_chain: &[String], element: UIElement) {
        if let Some(cache_key) = Self::get_cache_key(selector_chain) {
            let mut cache = self.element_cache.write().await;
            
            let element_id = element.id().unwrap_or_else(|| "N/A".to_string());
            let element_role = element.attributes().role;
            debug!(cache_key = %cache_key, element_id = %element_id, element_role = %element_role, "Caching element using derived key for chain: {:?}", selector_chain);
            
            cache.insert(cache_key.clone(), CacheEntry {
                element,
                last_accessed: Instant::now(),
            });
            debug!("Current cache size: {}. Item cached with key: {} for chain: {:?}", cache.len(), cache_key, selector_chain);
            // Optionally, log all keys if verbose logging is desired
            // for (key, entry) in cache.iter() {
            //     debug!(cached_item_key = %key, last_accessed = ?entry.last_accessed.elapsed(), "Item in cache");
            // }
        } else {
            // Reason for skipping is logged by get_cache_key
            debug!(original_chain = ?selector_chain, "Element caching skipped for this chain as it does not qualify for caching by ID selector rule.");
        }
    }
}

// Base request structure with selector chain
#[derive(Deserialize)]
struct ChainedRequest {
    selector_chain: Vec<String>,
    timeout_ms: Option<u64>, 
    depth: Option<usize>,
    detail_level: Option<DetailLevel>, // Added detail_level
}

// Request structure for typing text (with chain)
#[derive(Deserialize)]
struct TypeTextRequest {
    selector_chain: Vec<String>,
    text: String,
    timeout_ms: Option<u64>,     // Added timeout
    use_clipboard: Option<bool>, // Optional flag for fast typing using clipboard
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

// Add at the top with other request structs
#[derive(Deserialize)]
struct MouseDragRequest {
    selector_chain: Vec<String>,
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
    timeout_ms: Option<u64>,
}

// Request structure for scrolling
#[derive(Deserialize)]
struct ScrollRequest {
    selector_chain: Vec<String>,
    direction: String, // "up", "down", "left", "right"
    amount: f64,       // Number of scroll increments
    timeout_ms: Option<u64>,
}

// Request structure for mouse_click_and_hold
#[derive(Deserialize)]
struct MouseClickAndHoldRequest {
    selector_chain: Vec<String>,
    x: f64,
    y: f64,
    timeout_ms: Option<u64>,
}

// Request structure for mouse_move
#[derive(Deserialize)]
struct MouseMoveRequest {
    selector_chain: Vec<String>,
    x: f64,
    y: f64,
    timeout_ms: Option<u64>,
}

// Request structure for mouse_release
#[derive(Deserialize)]
struct MouseReleaseRequest {
    selector_chain: Vec<String>,
    timeout_ms: Option<u64>,
}

// Basic response structure
#[derive(Serialize)]
struct BasicResponse {
    message: String,
}

// Response structure for element details
#[derive(Serialize)]
struct ElementResponse {
    id: Option<String>,
    role: String,
    label: Option<String>,
    name: Option<String>,
    text: Option<String>,
    visible: Option<bool>,
    enabled: Option<bool>,
    focused: Option<bool>,
    is_keyboard_focusable: Option<bool>,
    bounds: Option<(f64, f64, f64, f64)>,
}

impl ElementResponse {
    fn from_element(element: &UIElement, detail_level: DetailLevel) -> Self {
        let mut response = Self {
            id: element.id(),
            role: element.role(),
            label: None,
            name: None,
            text: None,
            visible: None,
            enabled: None,
            focused: None,
            is_keyboard_focusable: None,
            bounds: None,
        };

        if detail_level == DetailLevel::Minimal {
            return response;
        }

        let attrs = element.attributes();
        response.name = attrs.name;
        response.label = attrs.label;
        response.bounds = element.bounds().ok();

        if detail_level == DetailLevel::Standard {
            return response;
        }

        response.text = element.text(1).ok();
        response.visible = element.is_visible().ok();
        response.enabled = element.is_enabled().ok();
        response.focused = element.is_focused().ok();
        response.is_keyboard_focusable = element.is_keyboard_focusable().ok();

        response
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
    is_keyboard_focusable: Option<bool>,
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
        // if let AutomationError::ElementNotFound(msg) = &err {
        //     // Check if the message already contains context hints
        //     if !msg.contains("within parent") && !msg.contains("Found windows:") {
        //         // Attempt to provide default context (this is basic, ideally context is added at source)
        //         return ApiError::Automation(AutomationError::ElementNotFound(format!(
        //             "{} (context: Root)",
        //             msg
        //         )));
        //     }
        // }
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

// Helper function to get timeout duration
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
#[instrument(skip(state, payload), fields(selector_chain = ?payload.selector_chain))]
async fn first(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChainedRequest>,
) -> Result<Json<ElementResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to find first element");
    
    // Try to get from cache first
    if let Some(element) = state.get_cached_element(&payload.selector_chain).await {
        info!("Found element in cache");
        return Ok(Json(ElementResponse::from_element(&element, payload.detail_level.unwrap_or_default())));
    }

    // If not in cache, find the element
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);
    
    match locator.first(timeout).await {
        Ok(element) => {
            // Cache the element
            if let Some(id) = element.id() {
                let cache_key = vec![format!("#{}", id)];
                state.cache_element(&cache_key, element.clone()).await;
            }
            
            info!("Element found successfully");
            Ok(Json(ElementResponse::from_element(&element, payload.detail_level.unwrap_or_default())))
        }
        Err(e) => {
            error!("Failed to find element: {}", e);
            Err(e.into())
        }
    }
}

// Handler for clicking an element
#[instrument(skip(state, payload), fields(selector_chain = ?payload.selector_chain))]
async fn click_element(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChainedRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to click element");
    
    // Try to get from cache first
    if let Some(element) = state.get_cached_element(&payload.selector_chain).await {
        info!("Found element in cache for clicking");
        match element.click() {
            Ok(_) => {
                info!("Element clicked successfully using cached element");
                Ok(Json(BasicResponse {
                    message: "Element clicked successfully".to_string(),
                }))
            }
            Err(e) => {
                error!("Failed to click cached element: {}", e);
                Err(e.into())
            }
        }
    } else {
        // If not in cache, find the element
        let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
        let timeout = get_timeout(payload.timeout_ms);
        
        match locator.first(timeout).await {
            Ok(element) => {
                // Cache the element using its ID as the cache key
                if let Some(id) = element.id() {
                    let cache_key = vec![format!("#{}", id)];
                    state.cache_element(&cache_key, element.clone()).await;
                }
                
                match element.click() {
                    Ok(_) => {
                        info!("Element clicked successfully");
                        Ok(Json(BasicResponse {
                            message: "Element clicked successfully".to_string(),
                        }))
                    }
                    Err(e) => {
                        error!("Failed to click element: {}", e);
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                error!("Failed to find element for clicking: {}", e);
                Err(e.into())
            }
        }
    }
}

// Handler for typing text into an element
#[instrument(skip(state, payload), fields(selector_chain = ?payload.selector_chain, text_length = payload.text.len()))]
async fn type_text_into_element(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TypeTextRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to type text");
    
    // Try to get from cache first
    if let Some(element) = state.get_cached_element(&payload.selector_chain).await {
        info!("Found element in cache for typing");
        match element.type_text(&payload.text, payload.use_clipboard.unwrap_or(false)) {
            Ok(_) => {
                info!("Text typed successfully using cached element");
                Ok(Json(BasicResponse {
                    message: "Text typed successfully".to_string(),
                }))
            }
            Err(e) => {
                error!("Failed to type text using cached element: {}", e);
                Err(e.into())
            }
        }
    } else {
        // If not in cache, find the element
        let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
        let timeout = get_timeout(payload.timeout_ms);
        
        match locator.first(timeout).await {
            Ok(element) => {
                // Cache the element using its ID as the cache key
                if let Some(id) = element.id() {
                    let cache_key = vec![format!("#{}", id)];
                    state.cache_element(&cache_key, element.clone()).await;
                }
                
                match element.type_text(&payload.text, payload.use_clipboard.unwrap_or(false)) {
                    Ok(_) => {
                        info!("Text typed successfully");
                        Ok(Json(BasicResponse {
                            message: "Text typed successfully".to_string(),
                        }))
                    }
                    Err(e) => {
                        error!("Failed to type text: {}", e);
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                error!("Failed to find element for typing: {}", e);
                Err(e.into())
            }
        }
    }
}

// Handler for getting text from an element
#[instrument(skip(state, payload), fields(selector_chain = ?payload.selector_chain))]
async fn get_element_text(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GetTextRequest>,
) -> Result<Json<TextResponse>, ApiError> {
    let start = Instant::now();
    info!("Starting text extraction operation");

    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    let element = locator.first(timeout).await?;
    let text = element.text(payload.max_depth.unwrap_or(1))?;

    let duration = start.elapsed();
    info!(
        duration_ms = duration.as_millis(),
        text_length = text.len(),
        "Completed text extraction operation"
    );

    Ok(Json(TextResponse { text }))
}

// Handler for finding multiple elements
async fn all(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChainedRequest>,
) -> Result<Json<ElementsResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to find all elements");

    // For 'all' operations, we don't use the cache for the initial find,
    // but we will populate it afterwards.
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    match locator.all(timeout, payload.depth).await {
        Ok(elements) => {
            info!(count = elements.len(), "Elements found successfully. Preparing response.");

            // Prepare the response to be sent to the client immediately
            let response = Json(ElementsResponse {
                elements: elements
                    .iter() // Iterate over a reference to avoid moving
                    .map(|element| ElementResponse::from_element(element, payload.detail_level.unwrap_or_default()))
                    .collect(),
            });

            // Clone necessary data for the background caching task
            let state_clone_for_caching = Arc::clone(&state);
            let elements_clone_for_caching = elements.clone(); // Clone elements for the background task
            let selector_chain_for_caching = payload.selector_chain.clone(); // Clone for logging context if needed

            // Spawn a background task to cache the elements
            tokio::spawn(async move {
                info!(count = elements_clone_for_caching.len(), chain = ?selector_chain_for_caching, "Starting background caching for elements.");
                for element in &elements_clone_for_caching {
                    if let Some(id) = element.id() {
                        // Use the element's own ID for caching, not the full chain
                        let cache_key_by_id = vec![format!("#{}", id)];
                        // It's important to use element.id() for the cache_key to ensure cache_element's logic is consistent
                        // AppState::get_cache_key expects a selector chain, so providing the ID selector is correct.
                        state_clone_for_caching.cache_element(&cache_key_by_id, element.clone()).await;
                    }
                }
                info!(count = elements_clone_for_caching.len(), chain = ?selector_chain_for_caching, "Background caching completed for elements.");
            });

            // Return the prepared response to the client
            Ok(response)
        }
        Err(e) => {
            error!("Failed to find elements: {}", e);
            Err(e.into())
        }
    }
}

async fn get_full_tree(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChainedRequest>,
) -> Result<Json<ElementsResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to get full tree");

    // Define the recursive helper function
    fn get_children_recursive(element: &UIElement) -> Result<Vec<ElementResponse>, ApiError> {
        let direct_children_elements = element.children()?;
        let mut all_descendants: Vec<ElementResponse> = Vec::new();

        for child_element in &direct_children_elements {
            // 1. Add the direct child itself (converted to ElementResponse)
            all_descendants.push(ElementResponse::from_element(child_element, DetailLevel::Full));

            // 2. Recursively call for this child_element's descendants
            match get_children_recursive(child_element) {
                // Recursive call with &UIElement
                Ok(deeper_responses) => {
                    // Extend the list with the descendants returned from the recursive call
                    all_descendants.extend(deeper_responses);
                }
                Err(e) => {
                    // Log error and decide how to proceed (e.g., skip this branch, return error)
                    error!(
                        "Error getting children recursively for element {:?}: {:?}",
                        child_element.id(),
                        e
                    );
                    // Optionally: return Err(e); // Propagate the error up
                    // Continue processing other children for now.
                }
            }
        }

        Ok(all_descendants) // Return the accumulated list
    }

    // Handle empty selector chain case
    if payload.selector_chain.is_empty() {
        info!("Empty selector chain, getting full tree of all applications");
        let mut all_elements: Vec<ElementResponse> = Vec::new();

        // Get all applications
        let applications = state.desktop.applications()?;

        // Process each application and its children
        for app in applications {
            // Add the application itself
            all_elements.push(ElementResponse::from_element(&app, DetailLevel::Full));

            // Get all descendants of this application
            match get_children_recursive(&app) {
                Ok(descendants) => {
                    all_elements.extend(descendants);
                }
                Err(e) => {
                    error!(
                        "Error getting descendants for application {:?}: {:?}",
                        app.id(),
                        e
                    );
                    // Continue with other applications
                }
            }
        }

        return Ok(Json(ElementsResponse {
            elements: all_elements,
        }));
    }

    // Original logic for non-empty selector chain
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    // Find the root element for the subtree
    let root_element = locator.wait(timeout).await?;

    // Call the recursive function starting from the found root element
    let elements_in_subtree = get_children_recursive(&root_element)?;

    Ok(Json(ElementsResponse {
        elements: elements_in_subtree,
    }))
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

    info!(
        "Attributes retrieved successfully for element ID: {:?}",
        element_id
    );

    // Construct and return the response
    Ok(Json(AttributesResponse {
        role: attrs.role,
        label: attrs.label,
        value: attrs.value,
        description: attrs.description,
        properties: attrs.properties,
        id: element_id, // Use the ID obtained from the element
        is_keyboard_focusable: attrs.is_keyboard_focusable,
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
            Ok(Json(ElementResponse::from_element(&window_element, DetailLevel::Full)))
        }
        Err(e) => {
            error!("Failed to find window with criteria {:?}: {}", payload, e);
            Err(e.into())
        }
    }
}

// Handler for exploring an element's children
#[instrument(skip(state, payload), fields(selector_chain = ?payload.selector_chain))]
async fn explore_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExploreRequest>,
) -> Result<Json<ExploreResponse>, ApiError> {
    let start = Instant::now();
    info!("Starting element exploration");

    let locator = if let Some(chain) = &payload.selector_chain {
        create_locator_for_chain(&state, chain)?
    } else {
        state.desktop.locator(Selector::Role {
            role: "window".to_string(),
            name: None,
        })
    };

    let timeout = get_timeout(payload.timeout_ms);
    let element = locator.first(timeout).await?;

    let parent = ElementResponse::from_element(&element, DetailLevel::Full);
    let mut children = Vec::new();

    let children_start = Instant::now();
    info!("Starting children exploration");

    for child in element.children()? {
        let child_id = child.id().unwrap_or_default();
        let child_bounds = child.bounds().ok();
        let child_bounds_response = child_bounds.map(|(x, y, width, height)| BoundsResponse {
            x,
            y,
            width,
            height,
        });

        let child_attrs = child.attributes();
        let child_text = child.text(1).ok();

        let suggested_selector = format!(
            "#{}",
            child_id
        );

        children.push(ExploredElementDetail {
            role: child_attrs.role,
            name: child_attrs.name,
            id: Some(child_id),
            bounds: child_bounds_response,
            value: child_attrs.value,
            description: child_attrs.description,
            text: child_text,
            parent_id: parent.id.clone(),
            children_ids: Vec::new(),
            suggested_selector,
        });
    }

    let children_duration = children_start.elapsed();
    info!(
        children_count = children.len(),
        children_duration_ms = children_duration.as_millis(),
        "Completed children exploration"
    );

    let duration = start.elapsed();
    info!(
        duration_ms = duration.as_millis(),
        "Completed element exploration"
    );

    Ok(Json(ExploreResponse { parent, children }))
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
async fn capture_screen(State(state): State<Arc<AppState>>) -> Result<Json<OcrResponse>, ApiError> {
    info!("Attempting to capture primary screen and perform OCR");
    // 1. Capture screen
    let screenshot_result = state
        .desktop
        .capture_screen()
        .await
        .map_err(ApiError::from)?;
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
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Expecting element to be visible");
    
    // Try to get from cache first
    if let Some(element) = state.get_cached_element(&payload.selector_chain).await {
        info!("Found element in cache for visibility check");
        if element.is_visible().unwrap_or(false) {
            info!("Cached element is visible");
            return Ok(Json(ElementResponse::from_element(&element, DetailLevel::Full)));
        }
    }

    // If not in cache or not visible, find the element
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);
    
    match locator.first(timeout).await {
        Ok(element) => {
            // Cache the element
            state.cache_element(&payload.selector_chain, element.clone()).await;
            
            if element.is_visible().unwrap_or(false) {
                info!("Element is visible");
                Ok(Json(ElementResponse::from_element(&element, DetailLevel::Full)))
            } else {
                error!("Element is not visible");
                Err(AutomationError::ElementNotFound(format!(
                    "Element {:?} is not visible",
                    payload.selector_chain
                )).into())
            }
        }
        Err(e) => {
            error!("Failed to find element for visibility check: {}", e);
            Err(e.into())
        }
    }
}

async fn expect_element_enabled(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExpectRequest>,
) -> Result<Json<ElementResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Expecting element to be enabled");
    
    // Try to get from cache first
    if let Some(element) = state.get_cached_element(&payload.selector_chain).await {
        info!("Found element in cache for enabled check");
        if element.is_enabled().unwrap_or(false) {
            info!("Cached element is enabled");
            return Ok(Json(ElementResponse::from_element(&element, DetailLevel::Full)));
        }
    }

    // If not in cache or not enabled, find the element
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);
    
    match locator.first(timeout).await {
        Ok(element) => {
            // Cache the element
            state.cache_element(&payload.selector_chain, element.clone()).await;
            
            if element.is_enabled().unwrap_or(false) {
                info!("Element is enabled");
                Ok(Json(ElementResponse::from_element(&element, DetailLevel::Full)))
            } else {
                error!("Element is not enabled");
                Err(AutomationError::ElementNotFound(format!(
                    "Element {:?} is not enabled",
                    payload.selector_chain
                )).into())
            }
        }
        Err(e) => {
            error!("Failed to find element for enabled check: {}", e);
            Err(e.into())
        }
    }
}

async fn expect_element_text_equals(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExpectTextRequest>,
) -> Result<Json<ElementResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Expecting element text to equal");
    
    // Try to get from cache first
    if let Some(element) = state.get_cached_element(&payload.selector_chain).await {
        info!("Found element in cache for text check");
        if element.text(1).unwrap_or_default() == payload.expected_text {
            info!("Cached element text matches");
            return Ok(Json(ElementResponse::from_element(&element, DetailLevel::Full)));
        }
    }

    // If not in cache or text doesn't match, find the element
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);
    
    match locator.first(timeout).await {
        Ok(element) => {
            // Cache the element
            state.cache_element(&payload.selector_chain, element.clone()).await;
            
            if element.text(1).unwrap_or_default() == payload.expected_text {
                info!("Element text matches");
                Ok(Json(ElementResponse::from_element(&element, DetailLevel::Full)))
            } else {
                error!("Element text does not match");
                Err(AutomationError::ElementNotFound(format!(
                    "Element {:?} text does not match expected text",
                    payload.selector_chain
                )).into())
            }
        }
        Err(e) => {
            error!("Failed to find element for text check: {}", e);
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
    let element = locator.wait(timeout).await.map_err(|e| {
        error!("Failed to find element for activation: {}", e);
        ApiError::from(e)
    })?;

    // 2. Get the containing application/window from the element (needs core implementation)
    // Example: let app_element = element.containing_application()?; // Needs method on UIElement/Impl

    // 3. Activate the application (using existing desktop method or a new one)
    // Example: state.desktop.activate_application_by_element(&app_element)?;
    // Or maybe: element.activate_window()?; (using existing trait method)
    match element.activate_window() {
        // Now `element` is defined
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

// --- NEW HANDLER for current browser window --- //
async fn get_current_browser_window_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ElementResponse>, ApiError> {
    info!("Attempting to get current browser window");
    match state.desktop.get_current_browser_window().await {
        Ok(element) => {
            info!(
                "Current browser window found: role={}, label={:?}, id={:?}",
                element.role(),
                element.attributes().label,
                element.id()
            );
            Ok(Json(ElementResponse::from_element(&element, DetailLevel::Full)))
        }
        Err(e) => {
            error!("Failed to get current browser window: {}", e);
            Err(e.into())
        }
    }
}

// Handler for mouse_drag
async fn mouse_drag_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MouseDragRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, start_x = payload.start_x, start_y = payload.start_y, end_x = payload.end_x, end_y = payload.end_y, timeout = ?payload.timeout_ms, "Attempting to mouse_drag element");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);
    // Wait for the element (reuse locator.wait)
    let element = locator.wait(timeout).await.map_err(|e| {
        error!("Failed to find element for mouse_drag: {}", e);
        ApiError::from(e)
    })?;
    // Call mouse_drag on the element
    element
        .mouse_drag(
            payload.start_x,
            payload.start_y,
            payload.end_x,
            payload.end_y,
        )
        .map_err(|e| {
            error!("Failed to mouse_drag element: {}", e);
            ApiError::from(e)
        })?;
    Ok(Json(BasicResponse {
        message: "Mouse drag performed successfully".to_string(),
    }))
}

// Handler for scrolling an element
async fn scroll_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScrollRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, direction = %payload.direction, amount = payload.amount, timeout = ?payload.timeout_ms, "Attempting to scroll element");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);

    // Wait for the element
    let element = locator.wait(timeout).await.map_err(|e| {
        error!("Failed to find element for scrolling: {}", e);
        ApiError::from(e)
    })?;

    // Call scroll on the element
    element
        .scroll(&payload.direction, payload.amount)
        .map_err(|e| {
            error!("Failed to scroll element: {}", e);
            ApiError::from(e)
        })?;

    Ok(Json(BasicResponse {
        message: format!(
            "Scrolled {} by {} increments",
            payload.direction, payload.amount
        ),
    }))
}

// Handler for mouse_click_and_hold
async fn mouse_click_and_hold_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MouseClickAndHoldRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, x = payload.x, y = payload.y, timeout = ?payload.timeout_ms, "Attempting to mouse_click_and_hold element");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);
    let element = locator.wait(timeout).await.map_err(|e| {
        error!("Failed to find element for mouse_click_and_hold: {}", e);
        ApiError::from(e)
    })?;
    element.mouse_click_and_hold(payload.x, payload.y).map_err(|e| {
        error!("Failed to mouse_click_and_hold element: {}", e);
        ApiError::from(e)
    })?;
    Ok(Json(BasicResponse {
        message: "Mouse click and hold performed successfully".to_string(),
    }))
}

// Handler for mouse_move
async fn mouse_move_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MouseMoveRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, x = payload.x, y = payload.y, timeout = ?payload.timeout_ms, "Attempting to mouse_move element");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);
    let element = locator.wait(timeout).await.map_err(|e| {
        error!("Failed to find element for mouse_move: {}", e);
        ApiError::from(e)
    })?;
    element.mouse_move(payload.x, payload.y).map_err(|e| {
        error!("Failed to mouse_move element: {}", e);
        ApiError::from(e)
    })?;
    Ok(Json(BasicResponse {
        message: "Mouse move performed successfully".to_string(),
    }))
}

// Handler for mouse_release
async fn mouse_release_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MouseReleaseRequest>,
) -> Result<Json<BasicResponse>, ApiError> {
    info!(chain = ?payload.selector_chain, timeout = ?payload.timeout_ms, "Attempting to mouse_release element");
    let locator = create_locator_for_chain(&state, &payload.selector_chain)?;
    let timeout = get_timeout(payload.timeout_ms);
    let element = locator.wait(timeout).await.map_err(|e| {
        error!("Failed to find element for mouse_release: {}", e);
        ApiError::from(e)
    })?;
    element.mouse_release().map_err(|e| {
        error!("Failed to mouse_release element: {}", e);
        ApiError::from(e)
    })?;
    Ok(Json(BasicResponse {
        message: "Mouse release performed successfully".to_string(),
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with timestamps and more detailed formatting
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_timer(tracing_subscriber::fmt::time::time())
        .with_span_events(FmtSpan::CLOSE)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting Terminator server");
    let start = Instant::now();

    let desktop = Arc::new(Desktop::new(false, false)?);
    let app_state = Arc::new(AppState::new(desktop));

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
        .route("/get_full_tree", post(get_full_tree))
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
        // New endpoint for current browser window
        .route(
            "/current_browser_window",
            get(get_current_browser_window_handler),
        )
        // State and Layers
        .route("/mouse_drag", post(mouse_drag_handler))
        .route("/mouse_click_and_hold", post(mouse_click_and_hold_handler))
        .route("/mouse_move", post(mouse_move_handler))
        .route("/mouse_release", post(mouse_release_handler))
        .route("/scroll", post(scroll_handler))
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024)) // Reduced to 10MB
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(app_state);

    // Determine port
    let default_port = 9375;
    let port: u16 = env::var("PORT")
        .ok() // Convert Result to Option
        .and_then(|s| s.parse().ok()) // Try parsing the string to u16
        .unwrap_or(default_port); // Use default if env var not set or invalid

    let addr = format!("127.0.0.1:{}", port);
    info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    let duration = start.elapsed();
    info!(
        duration_ms = duration.as_millis(),
        "Server shutdown complete"
    );

    Ok(())
}
