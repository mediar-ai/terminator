use crate::element::UIElementImpl;
use crate::platforms::AccessibilityEngine;
use crate::{AutomationError, Locator, Selector, UIElement, UIElementAttributes};
use crate::{ClickResult, CommandOutput, ScreenshotResult};
use atspi::{State, StateSet};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Debug;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;
use std::sync::{mpsc, OnceLock};
use std::thread;
use std::time::Duration;
use tracing::debug;

use atspi::{
    connection::set_session_accessibility,
    proxy::accessible::{AccessibleProxy, ObjectRefExt},
    zbus::{proxy::CacheProperties, Connection},
    AccessibilityConnection, Role,
};
use atspi_common::{
    object_match::{MatchType, ObjectMatchRule, SortOrder},
    state, CoordType,
};
use atspi_proxies::{
    action::ActionProxy,
    collection::CollectionProxy,
    component::ComponentProxy,
    device_event_controller::{DeviceEventControllerProxy, KeySynthType},
    text::TextProxy,
};
use futures::future::join_all;
use image::{DynamicImage, ImageBuffer, Rgba};
use uni_ocr::{OcrEngine, OcrProvider};
use zbus::fdo::DBusProxy;

// Copied from atspi-common/src/role.rs (not public)
const ROLE_NAMES: &[&str] = &[
    "invalid",
    "accelerator label",
    "alert",
    "animation",
    "arrow",
    "calendar",
    "canvas",
    "check box",
    "check menu item",
    "color chooser",
    "column header",
    "combo box",
    "date editor",
    "desktop icon",
    "desktop frame",
    "dial",
    "dialog",
    "directory pane",
    "drawing area",
    "file chooser",
    "filler",
    "focus traversable",
    "font chooser",
    "frame",
    "glass pane",
    "html container",
    "icon",
    "image",
    "internal frame",
    "label",
    "layered pane",
    "list",
    "list item",
    "menu",
    "menu bar",
    "menu item",
    "option pane",
    "page tab",
    "page tab list",
    "panel",
    "password text",
    "popup menu",
    "progress bar",
    "button",
    "radio button",
    "radio menu item",
    "root pane",
    "row header",
    "scroll bar",
    "scroll pane",
    "separator",
    "slider",
    "spin button",
    "split pane",
    "status bar",
    "table",
    "table cell",
    "table column header",
    "table row header",
    "tearoff menu item",
    "terminal",
    "text",
    "toggle button",
    "tool bar",
    "tool tip",
    "tree",
    "tree table",
    "unknown",
    "viewport",
    "window",
    "extended",
    "header",
    "footer",
    "paragraph",
    "ruler",
    "application",
    "autocomplete",
    "editbar",
    "embedded",
    "entry",
    "chart",
    "caption",
    "document frame",
    "heading",
    "page",
    "section",
    "redundant object",
    "form",
    "link",
    "input method window",
    "table row",
    "tree item",
    "document spreadsheet",
    "document presentation",
    "document text",
    "document web",
    "document email",
    "comment",
    "list box",
    "grouping",
    "image map",
    "notification",
    "info bar",
    "level bar",
    "title bar",
    "block quote",
    "audio",
    "video",
    "definition",
    "article",
    "landmark",
    "log",
    "marquee",
    "math",
    "rating",
    "timer",
    "static",
    "math fraction",
    "math root",
    "subscript",
    "superscript",
    "description list",
    "description term",
    "description value",
    "footnote",
    "content deletion",
    "content insertion",
    "mark",
    "suggestion",
    "push button menu",
];

// Linux-specific error handling
impl From<zbus::Error> for AutomationError {
    fn from(error: zbus::Error) -> Self {
        AutomationError::PlatformError(error.to_string())
    }
}

impl From<atspi_proxies::AtspiError> for AutomationError {
    fn from(error: atspi_proxies::AtspiError) -> Self {
        AutomationError::PlatformError(error.to_string())
    }
}

const REGISTRY_DEST: &str = "org.a11y.atspi.Registry";
const REGISTRY_PATH: &str = "/org/a11y/atspi/accessible/root";
const ACCESSIBLE_INTERFACE: &str = "org.a11y.atspi.Accessible";

// Thread-safe wrapper for AccessibleProxy
#[derive(Debug, Clone)]
pub struct ThreadSafeLinuxUIElement(pub Arc<AccessibleProxy<'static>>);
unsafe impl Send for ThreadSafeLinuxUIElement {}
unsafe impl Sync for ThreadSafeLinuxUIElement {}

// Linux Engine struct
#[derive(Clone)]
pub struct LinuxEngine {
    connection: Arc<Connection>,
    root: ThreadSafeLinuxUIElement,
}

#[derive(Debug, Clone)]
pub struct LinuxUIElement {
    connection: Arc<Connection>,
    destination: String,
    path: String,
}

// --- Background Worker for Async AT-SPI Calls ---
// Type aliases for complex types to reduce clippy::type_complexity warnings

type StringChannel = (
    std::sync::mpsc::Sender<Result<String, AutomationError>>,
    std::sync::mpsc::Receiver<Result<String, AutomationError>>,
);
type OptionStringChannel = (
    std::sync::mpsc::Sender<Result<Option<String>, AutomationError>>,
    std::sync::mpsc::Receiver<Result<Option<String>, AutomationError>>,
);
type OptionUIElementChannel = (
    std::sync::mpsc::Sender<Result<Option<UIElement>, AutomationError>>,
    std::sync::mpsc::Receiver<Result<Option<UIElement>, AutomationError>>,
);
type BoundsChannel = (
    std::sync::mpsc::Sender<Result<(f64, f64, f64, f64), AutomationError>>,
    std::sync::mpsc::Receiver<Result<(f64, f64, f64, f64), AutomationError>>,
);
type ClickResultChannel = (
    std::sync::mpsc::Sender<Result<ClickResult, AutomationError>>,
    std::sync::mpsc::Receiver<Result<ClickResult, AutomationError>>,
);
type UnitChannel = (
    std::sync::mpsc::Sender<Result<(), AutomationError>>,
    std::sync::mpsc::Receiver<Result<(), AutomationError>>,
);
type BoolChannel = (
    std::sync::mpsc::Sender<Result<bool, AutomationError>>,
    std::sync::mpsc::Receiver<Result<bool, AutomationError>>,
);
type U32Channel = (
    std::sync::mpsc::Sender<Result<u32, AutomationError>>,
    std::sync::mpsc::Receiver<Result<u32, AutomationError>>,
);

type Request = Box<
    dyn FnOnce() -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<Vec<UIElement>, AutomationError>> + Send>,
        > + Send
        + 'static,
>;
type Response = Result<Vec<UIElement>, AutomationError>;

type UsizeRequest = Box<
    dyn FnOnce() -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<usize, AutomationError>> + Send>,
        > + Send
        + 'static,
>;
type UsizeResponse = Result<usize, AutomationError>;

static WORKER: OnceLock<std::sync::mpsc::Sender<(Request, std::sync::mpsc::Sender<Response>)>> =
    OnceLock::new();
static USIZE_WORKER: OnceLock<
    std::sync::mpsc::Sender<(UsizeRequest, std::sync::mpsc::Sender<UsizeResponse>)>,
> = OnceLock::new();
type AtSpiInitWorkerSender = std::sync::mpsc::Sender<(
    (),
    std::sync::mpsc::Sender<Result<LinuxEngine, AutomationError>>,
)>;
static AT_SPI_INIT_WORKER: OnceLock<AtSpiInitWorkerSender> = OnceLock::new();

fn get_worker() -> &'static std::sync::mpsc::Sender<(Request, std::sync::mpsc::Sender<Response>)> {
    WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(Request, mpsc::Sender<Response>)>();
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            for (req, resp_tx) in rx {
                let fut = req();
                let result = rt.block_on(fut);
                let _ = resp_tx.send(result);
            }
        });
        tx
    })
}

fn get_usize_worker(
) -> &'static std::sync::mpsc::Sender<(UsizeRequest, std::sync::mpsc::Sender<UsizeResponse>)> {
    USIZE_WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(UsizeRequest, mpsc::Sender<UsizeResponse>)>();
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            for (req, resp_tx) in rx {
                let fut = req();
                let result = rt.block_on(fut);
                let _ = resp_tx.send(result);
            }
        });
        tx
    })
}

// Helper to erase the type of the async block to a trait object
fn erase_future<F>(
    fut: F,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Vec<UIElement>, AutomationError>> + Send>,
>
where
    F: std::future::Future<Output = Result<Vec<UIElement>, AutomationError>> + Send + 'static,
{
    Box::pin(fut)
}

// Helper: Convert string to Role using ROLE_NAMES (case-insensitive)
fn role_from_str(role: &str) -> Option<Role> {
    let role_lc = role.to_lowercase();
    for (idx, &name) in ROLE_NAMES.iter().enumerate() {
        if name == role_lc {
            return Role::try_from(idx as u32).ok();
        }
    }
    None
}

/// Recursive fallback: Traverse all descendants with filters if CollectionProxy is not available
async fn traverse_descendants_with_filters<'a>(
    proxy: &'a AccessibleProxy<'a>,
    states: Option<Vec<State>>,
    roles: Option<Vec<Role>>,
    name_substring: Option<&'a str>,
    visited: &mut HashSet<String>,
    stop_after_first: bool,
) -> Result<Vec<atspi::ObjectRef>, AutomationError> {
    let mut matches = Vec::new();
    let path = proxy.inner().path().to_string();
    if !visited.insert(path.clone()) {
        // Already visited
        return Ok(matches);
    }
    // Apply filters to this node
    let mut is_match = true;
    if let Some(ref expected_states) = states {
        is_match &= proxy
            .get_state()
            .await
            .ok()
            .is_some_and(|s| expected_states.iter().all(|st| s.contains(*st)));
    }
    if let Some(ref expected_roles) = roles {
        is_match &= proxy
            .get_role()
            .await
            .ok()
            .is_some_and(|r| expected_roles.contains(&r));
    }
    if let Some(ref substr) = name_substring {
        is_match &= proxy.name().await.ok().is_some_and(|n| n.contains(substr));
    }
    if is_match {
        let role = proxy.get_role().await.ok();
        let name = proxy.name().await.ok();
        let path = proxy.inner().path().to_string();
        let destination = proxy.inner().destination().to_string();
        debug!(
            "[fallback traversal] Matched descendant: role={:?}, name={:?}, path={}, destination={}",
            role, name, path, destination
        );
        let object_ref = atspi_common::ObjectRef::try_from(proxy).map_err(|e| {
            AutomationError::PlatformError(format!("Failed to convert proxy to ObjectRef: {e}"))
        })?;
        matches.push(object_ref);
        if stop_after_first {
            return Ok(matches);
        }
    }
    // Recurse into children in parallel
    let children = proxy.get_children().await.unwrap_or_default();
    let connection = proxy.inner().connection().clone();
    let futures = children.into_iter().map(|child_ref| {
        let connection = connection.clone();
        let states = states.clone();
        let roles = roles.clone();
        let name_substring = name_substring;
        let mut visited = visited.clone();
        async move {
            if let Ok(child_proxy) = child_ref.clone().into_accessible_proxy(&connection).await {
                traverse_descendants_with_filters(
                    &child_proxy,
                    states,
                    roles,
                    name_substring,
                    &mut visited,
                    stop_after_first,
                )
                .await
                .unwrap_or_default()
            } else {
                Vec::new()
            }
        }
    });
    let children_results = join_all(futures).await;
    for mut child_matches in children_results {
        if !child_matches.is_empty() {
            matches.append(&mut child_matches);
            if stop_after_first {
                return Ok(matches);
            }
        }
    }
    Ok(matches)
}

// Helper: Recursively collect all elements from a root up to a given depth (breadth-first)
pub async fn get_all_elements_from_root(
    root: &UIElement,
) -> Result<Vec<UIElement>, AutomationError> {
    let mut all_elements = Vec::new();
    let linux_elem = root
        .as_any()
        .downcast_ref::<LinuxUIElement>()
        .ok_or_else(|| AutomationError::PlatformError("Invalid root element type".to_string()))?;

    // Get the role of the root element
    let root_role = linux_elem.role();
    debug!("Root element role: {}", root_role);

    if root_role == "desktop frame" {
        // Get direct children of desktop frame
        let root_proxy = AccessibleProxy::builder(&linux_elem.connection)
            .destination(linux_elem.destination.as_str())
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?
            .path(linux_elem.path.as_str())
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?
            .interface(ACCESSIBLE_INTERFACE)?
            .cache_properties(CacheProperties::No)
            .build()
            .await
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

        let root_children = root_proxy
            .get_children()
            .await
            .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;

        // Filter children with child_count > 0
        let mut children_with_descendants = Vec::new();
        for child in &root_children {
            let child_proxy = child
                .clone()
                .into_accessible_proxy(&linux_elem.connection)
                .await
                .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
            let child_count = child_proxy
                .child_count()
                .await
                .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
            if child_count > 0 {
                children_with_descendants.push(UIElement::new(Box::new(LinuxUIElement {
                    connection: Arc::clone(&linux_elem.connection),
                    destination: child_proxy.inner().destination().to_string(),
                    path: child_proxy.inner().path().to_string(),
                })));
            }
        }

        // Add root element
        all_elements.push(root.clone());
        // Add direct children
        all_elements.extend(children_with_descendants.iter().cloned());

        // Use CollectionProxy with ObjectMatchRule for each child with descendants
        let mut futures = Vec::new();
        for child in &children_with_descendants {
            let child_clone = child.clone();
            let future = async move {
                let linux_child = child_clone
                    .as_any()
                    .downcast_ref::<LinuxUIElement>()
                    .ok_or_else(|| {
                        AutomationError::PlatformError("Invalid child element type".to_string())
                    })?;

                let collection_proxy_result = CollectionProxy::builder(&linux_child.connection)
                    .destination(linux_child.destination.as_str())
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                    .path(linux_child.path.as_str())
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                    .build()
                    .await;
                let collection_matches = if let Ok(collection_proxy) = collection_proxy_result {
                    let match_rule = ObjectMatchRule::builder()
                        .states(StateSet::new(State::Showing), MatchType::All)
                        .build();
                    match collection_proxy
                        .get_matches_from(
                            collection_proxy.inner().path(),
                            match_rule,
                            SortOrder::Canonical,
                            atspi::TreeTraversalType::Inorder,
                            0,
                            true,
                        )
                        .await
                    {
                        Ok(matches) => matches.into_iter().collect::<Vec<_>>(),
                        Err(_) => {
                            // Fallback: manual traversal with filter State::Showing
                            let root_proxy = AccessibleProxy::builder(&linux_child.connection)
                                .destination(linux_child.destination.as_str())
                                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                                .path(linux_child.path.as_str())
                                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                                .interface(ACCESSIBLE_INTERFACE)?
                                .cache_properties(CacheProperties::No)
                                .build()
                                .await
                                .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
                            let mut visited = HashSet::new();
                            let matches = traverse_descendants_with_filters(
                                &root_proxy,
                                Some(vec![State::Showing]),
                                None,
                                None,
                                &mut visited,
                                false,
                            )
                            .await?;
                            matches.into_iter().collect::<Vec<_>>()
                        }
                    }
                } else {
                    // Fallback: manual traversal with filter State::Showing
                    let root_proxy = AccessibleProxy::builder(&linux_child.connection)
                        .destination(linux_child.destination.as_str())
                        .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                        .path(linux_child.path.as_str())
                        .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                        .interface(ACCESSIBLE_INTERFACE)?
                        .cache_properties(CacheProperties::No)
                        .build()
                        .await
                        .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
                    let mut visited = HashSet::new();
                    let matches = traverse_descendants_with_filters(
                        &root_proxy,
                        Some(vec![State::Showing]),
                        None,
                        None,
                        &mut visited,
                        false,
                    )
                    .await?;
                    matches.into_iter().collect::<Vec<_>>()
                };
                let mut child_elements = Vec::new();
                for m in collection_matches {
                    let proxy = m
                        .into_accessible_proxy(&linux_child.connection)
                        .await
                        .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
                    child_elements.push(UIElement::new(Box::new(LinuxUIElement {
                        connection: Arc::clone(&linux_child.connection),
                        destination: proxy.inner().destination().to_string(),
                        path: proxy.inner().path().to_string(),
                    })));
                }
                Ok::<Vec<UIElement>, AutomationError>(child_elements)
            };
            futures.push(future);
        }
        // Join all futures to get descendants
        let results = futures::future::try_join_all(futures).await?;
        for child_elements in results {
            all_elements.extend(child_elements);
        }
    } else {
        // For non-desktop frame root, directly use CollectionProxy
        let root_proxy = AccessibleProxy::builder(&linux_elem.connection)
            .destination(linux_elem.destination.as_str())
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?
            .path(linux_elem.path.as_str())
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?
            .interface(ACCESSIBLE_INTERFACE)?
            .cache_properties(CacheProperties::No)
            .build()
            .await
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;

        let destination = root_proxy.inner().destination().to_string();
        let path = root_proxy.inner().path().to_string();
        let collection_proxy_result = CollectionProxy::builder(&linux_elem.connection)
            .destination(destination.as_str())
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?
            .path(path.as_str())
            .map_err(|e| AutomationError::PlatformError(e.to_string()))?
            .build()
            .await;

        let collection_matches = if let Ok(collection_proxy) = collection_proxy_result {
            let match_rule = ObjectMatchRule::builder()
                .states(StateSet::new(State::Showing), MatchType::All)
                .build();
            match collection_proxy
                .get_matches_from(
                    collection_proxy.inner().path(),
                    match_rule,
                    SortOrder::Canonical,
                    atspi::TreeTraversalType::Inorder,
                    0,
                    true,
                )
                .await
            {
                Ok(matches) => matches.into_iter().collect::<Vec<_>>(),
                Err(_) => {
                    // Fallback: manual traversal with filter State::Showing
                    let mut visited = HashSet::new();
                    let matches = traverse_descendants_with_filters(
                        &root_proxy,
                        Some(vec![State::Showing]),
                        None,
                        None,
                        &mut visited,
                        false,
                    )
                    .await?;
                    matches.into_iter().collect::<Vec<_>>()
                }
            }
        } else {
            // Fallback: manual traversal with filter State::Showing
            let mut visited = HashSet::new();
            let matches = traverse_descendants_with_filters(
                &root_proxy,
                Some(vec![State::Showing]),
                None,
                None,
                &mut visited,
                false,
            )
            .await?;
            matches.into_iter().collect::<Vec<_>>()
        };
        // Add root element
        all_elements.push(root.clone());
        // Add all matched elements
        for m in collection_matches {
            let proxy = m
                .into_accessible_proxy(&linux_elem.connection)
                .await
                .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
            all_elements.push(UIElement::new(Box::new(LinuxUIElement {
                connection: Arc::clone(&linux_elem.connection),
                destination: proxy.inner().destination().to_string(),
                path: proxy.inner().path().to_string(),
            })));
        }
    }
    Ok(all_elements)
}

fn find_elements_inner<'a>(
    linux_engine: &'a LinuxEngine,
    selector: &'a Selector,
    root: Option<&'a UIElement>,
    depth: Option<usize>,
) -> Pin<Box<dyn Future<Output = Result<Vec<UIElement>, AutomationError>> + Send + 'a>> {
    Box::pin(async move {
        use crate::Selector;
        match selector {
            Selector::Attributes(_) => {
                return Err(AutomationError::UnsupportedPlatform(
                    "Selector::Attributes is not implemented for Linux".to_string(),
                ));
            }
            Selector::Filter(_) => {
                return Err(AutomationError::UnsupportedPlatform(
                    "Selector::Filter is not implemented for Linux".to_string(),
                ));
            }
            Selector::Path(_) => {
                return Err(AutomationError::UnsupportedPlatform(
                    "Selector::Path is not implemented for Linux".to_string(),
                ));
            }
            Selector::LocalizedRole(_) => {
                return Err(AutomationError::UnsupportedPlatform(
                    "LocalizedRole selector is not yet supported for Linux".to_string(),
                ));
            }
            Selector::ClassName(_) => {
                return Err(AutomationError::UnsupportedPlatform(
                    "Selector::ClassName is not implemented for Linux".to_string(),
                ));
            }
            Selector::NativeId(_) => {
                return Err(AutomationError::UnsupportedPlatform(
                    "Selector::NativeId is not implemented for Linux".to_string(),
                ));
            }

            Selector::Text(_) => {
                return Err(AutomationError::UnsupportedPlatform(
                    "Selector::Text is not implemented for Linux".to_string(),
                ));
            }
            Selector::Id(target_id) => {
                // Traverse the tree from root, collect elements whose object_id matches target_id
                let root_binding = linux_engine.get_root_element();
                let root_elem = root.unwrap_or(&root_binding);
                let all_elements = get_all_elements_from_root(root_elem).await?;
                let results: Vec<UIElement> = all_elements
                    .into_iter()
                    .filter(|elem| elem.id() == Some(target_id.clone()))
                    .collect();
                if results.is_empty() {
                    return Err(AutomationError::ElementNotFound(format!(
                        "No element found with ID '{}';",
                        target_id
                    )));
                }
                return Ok(results);
            }
            Selector::Visible(_) => {
                return Err(AutomationError::UnsupportedPlatform(
                    "Selector::Visible is not implemented for Linux".to_string(),
                ));
            }
            Selector::Chain(chain) => {
                if chain.is_empty() {
                    return Err(AutomationError::InvalidArgument(
                        "Selector chain cannot be empty".to_string(),
                    ));
                }
                let mut current_elements = if let Some(r) = root {
                    vec![r.clone()]
                } else {
                    vec![linux_engine.get_root_element()]
                };
                for sel in chain {
                    let mut next_elements = Vec::new();
                    for elem in &current_elements {
                        let found =
                            find_elements_inner(linux_engine, sel, Some(elem), depth).await?;
                        next_elements.extend(found);
                    }
                    if next_elements.is_empty() {
                        return Err(AutomationError::ElementNotFound(
                            "Element not found after traversing chain".to_string(),
                        ));
                    }
                    current_elements = next_elements;
                }
                return Ok(current_elements);
            }
            Selector::Role { .. } | Selector::Name(_) => {
                // Supported - continue to processing below
            }
            Selector::Invalid(reason) => {
                return Err(AutomationError::InvalidArgument(reason.clone()));
            }
            Selector::RightOf(_)
            | Selector::LeftOf(_)
            | Selector::Above(_)
            | Selector::Below(_)
            | Selector::Near(_) => {
                return Err(AutomationError::UnsupportedPlatform(
                    "Relative selectors (RightOf/LeftOf/Above/Below/Near) are not implemented for Linux".to_string(),
                ));
            }
            Selector::Nth(_) => {
                return Err(AutomationError::UnsupportedPlatform(
                    "Selector::Nth is not implemented for Linux".to_string(),
                ));
            }
            Selector::Has(_) => {
                return Err(AutomationError::UnsupportedPlatform(
                    "Selector::Has is not implemented for Linux".to_string(),
                ));
            }
        }
        // Only Role and Name selectors are supported below
        let root_binding = linux_engine.get_root_element();
        let root_elem = root.unwrap_or(&root_binding);
        // Get all elements from the root
        let all_elements = get_all_elements_from_root(root_elem).await?;

        // Extract search criteria from selector
        let (target_role, name_contains) = match selector {
            Selector::Role { role, name } => (Some(role.to_lowercase()), name.clone()),
            Selector::Name(name) => (None, Some(name.clone())),
            _ => unreachable!(),
        };

        // Convert role string to Role enum if provided
        let role_enums: Option<Vec<Role>> = target_role.as_ref().map(|r| match r.as_str() {
            "window" => {
                let mut v = Vec::new();
                if let Some(win) = role_from_str("window") {
                    v.push(win);
                }
                if let Some(frame) = role_from_str("frame") {
                    v.push(frame);
                }
                v
            }
            "frame" => {
                let mut v = Vec::new();
                if let Some(win) = role_from_str("window") {
                    v.push(win);
                }
                if let Some(frame) = role_from_str("frame") {
                    v.push(frame);
                }
                v
            }
            _ => role_from_str(r).map(|x| vec![x]).unwrap_or_default(),
        });

        let mut results = Vec::new();
        for element in all_elements {
            let linux_elem = element
                .as_any()
                .downcast_ref::<LinuxUIElement>()
                .ok_or_else(|| {
                    AutomationError::PlatformError("Invalid element type".to_string())
                })?;
            let proxy = AccessibleProxy::builder(&linux_elem.connection)
                .destination(linux_elem.destination.as_str())
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                .path(linux_elem.path.as_str())
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                .interface(ACCESSIBLE_INTERFACE)?
                .cache_properties(CacheProperties::No)
                .build()
                .await
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
            let role = proxy
                .get_role()
                .await
                .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
            let name = proxy
                .name()
                .await
                .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
            let role_matches = role_enums
                .as_ref()
                .is_none_or(|targets| targets.contains(&role));
            let name_matches = name_contains
                .as_ref()
                .is_none_or(|target| name.contains(target));
            if role_matches && name_matches {
                results.push(element);
                if depth == Some(1) {
                    break;
                }
            }
        }
        Ok(results)
    })
}

fn find_elements_sync(
    engine: &LinuxEngine,
    selector: &Selector,
    root: Option<&UIElement>,
    timeout: Option<Duration>,
    depth: Option<usize>,
) -> Result<Vec<UIElement>, AutomationError> {
    let selector = selector.clone();
    let root = root.cloned();
    let engine = engine.clone();
    let (resp_tx, resp_rx) = mpsc::channel();
    let req = Box::new(move || {
        erase_future(
            async move { find_elements_inner(&engine, &selector, root.as_ref(), depth).await },
        )
    });
    get_worker().send((req, resp_tx)).unwrap();
    match timeout {
        Some(dur) => resp_rx
            .recv_timeout(dur)
            .unwrap_or_else(|_| Err(AutomationError::Timeout("Timeout".into()))),
        None => resp_rx.recv().unwrap(),
    }
}

// Helper: Find the first focused element in a list, with CollectionProxy fallback
async fn find_focused_element_async(elements: &[UIElement]) -> Result<UIElement, AutomationError> {
    use atspi::object_match::{MatchType, ObjectMatchRule, SortOrder};
    use atspi_common::state::{State, StateSet};
    use atspi_proxies::{accessible::AccessibleProxy, collection::CollectionProxy};
    // First, check if any element is focused
    let mut children_with_count = Vec::new();
    for element in elements {
        if element.is_focused().unwrap_or(false) {
            return Ok(element.clone());
        }
        // Save elements with child_count > 0 for fallback
        let linux_elem = element.as_any().downcast_ref::<LinuxUIElement>();
        if let Some(linux_elem) = linux_elem {
            let child_count = AccessibleProxy::builder(&linux_elem.connection)
                .destination(linux_elem.destination.as_str())
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                .path(linux_elem.path.as_str())
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                .build()
                .await
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                .child_count()
                .await
                .unwrap_or(0);
            if child_count > 0 {
                children_with_count.push(element.clone());
            }
        }
    }
    // Fallback: use CollectionProxy with StateSet matcher on each element
    for element in children_with_count {
        let linux_elem = element.as_any().downcast_ref::<LinuxUIElement>();
        if let Some(linux_elem) = linux_elem {
            let collection_proxy_result = CollectionProxy::builder(&linux_elem.connection)
                .destination(linux_elem.destination.as_str())
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                .path(linux_elem.path.as_str())
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                .build()
                .await;
            let matches = if let Ok(collection_proxy) = collection_proxy_result {
                let match_rule = ObjectMatchRule::builder()
                    .states(StateSet::new(State::Focused), MatchType::All)
                    .build();
                match collection_proxy
                    .get_matches_from(
                        collection_proxy.inner().path(),
                        match_rule,
                        SortOrder::Canonical,
                        atspi::TreeTraversalType::Inorder,
                        0,
                        true,
                    )
                    .await
                {
                    Ok(matches) => matches,
                    Err(_) => {
                        let root_proxy = AccessibleProxy::builder(&linux_elem.connection)
                            .destination(linux_elem.destination.as_str())
                            .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                            .path(linux_elem.path.as_str())
                            .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                            .interface(ACCESSIBLE_INTERFACE)?
                            .cache_properties(CacheProperties::No)
                            .build()
                            .await
                            .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
                        let mut visited = HashSet::new();
                        traverse_descendants_with_filters(
                            &root_proxy,
                            Some(vec![State::Focused]),
                            None,
                            None,
                            &mut visited,
                            false,
                        )
                        .await?
                    }
                }
            } else {
                // Fallback: manual traversal with filter State::Focused
                let root_proxy = AccessibleProxy::builder(&linux_elem.connection)
                    .destination(linux_elem.destination.as_str())
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                    .path(linux_elem.path.as_str())
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                    .interface(ACCESSIBLE_INTERFACE)?
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
                let mut visited = HashSet::new();
                traverse_descendants_with_filters(
                    &root_proxy,
                    Some(vec![State::Focused]),
                    None,
                    None,
                    &mut visited,
                    false,
                )
                .await?
            };
            if !matches.is_empty() {
                return Ok(element.clone());
            }
        }
    }
    Err(AutomationError::ElementNotFound(
        "No focused element found".to_string(),
    ))
}

/// Generate a stable element ID for a Linux UI element using accessibility properties
fn generate_element_id(
    connection: &Arc<Connection>,
    destination: &str,
    path: &str,
) -> Result<usize, AutomationError> {
    let connection = Arc::clone(connection);
    let destination = destination.to_string();
    let path = path.to_string();
    let (resp_tx, resp_rx) = mpsc::channel();
    let req = Box::new(move || {
        Box::pin(async move {
            let proxy = AccessibleProxy::builder(&connection)
                .destination(destination.as_str())
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                .path(path.as_str())
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                .build()
                .await
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
            // Get application (ObjectRef)
            let application = proxy.get_application().await.ok();
            // Get attributes
            let attributes = proxy.get_attributes().await.unwrap_or_default();
            // Sort attributes to ensure consistent ordering
            let mut sorted_attrs: Vec<(&str, &str)> = attributes
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            sorted_attrs.sort_by(|a, b| a.0.cmp(b.0));
            // Get role
            let role = proxy.get_role().await.ok();
            // Get description
            let description = proxy.description().await.ok();
            // Get name
            let name = proxy.name().await.ok();

            // Build a stable string
            let mut id_string = String::new();
            if let Some(app) = &application {
                id_string.push_str(&format!("app:{}:{};", app.name, app.path));
                tracing::trace!("ID component - app: {}:{}", app.name, app.path);
            }
            for (k, v) in &sorted_attrs {
                id_string.push_str(&format!("attr:{}={};", k, v));
                tracing::trace!("ID component - attr: {}={}", k, v);
            }
            if let Some(role) = &role {
                id_string.push_str(&format!("role:{};", role));
                tracing::trace!("ID component - role: {}", role.to_string());
            }
            if let Some(desc) = &description {
                id_string.push_str(&format!("desc:{};", desc));
                tracing::trace!("ID component - desc: {}", desc);
            }
            if let Some(name) = &name {
                id_string.push_str(&format!("name:{};", name));
                tracing::trace!("ID component - name: {}", name);
            }
            // Hash the string
            let mut hasher = DefaultHasher::new();
            id_string.hash(&mut hasher);
            tracing::trace!(
                "Generated ID string: {}, Hash: {}",
                id_string,
                hasher.finish()
            );
            Ok(hasher.finish() as usize)
        }) as Pin<Box<dyn Future<Output = Result<usize, AutomationError>> + Send>>
    });
    get_usize_worker().send((req, resp_tx)).unwrap();
    resp_rx.recv().unwrap()
}

/// Helper method to get accessible attributes using AccessibleProxy
fn get_accessible_attributes(
    element: &LinuxUIElement,
) -> Result<std::collections::HashMap<String, String>, AutomationError> {
    use std::sync::mpsc;
    let (resp_tx, resp_rx) = mpsc::channel();
    let this = element.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async move {
            let proxy = AccessibleProxy::builder(&this.connection)
                .destination(this.destination.as_str())
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                .path(this.path.as_str())
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                .build()
                .await
                .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
            let attributes = proxy
                .get_attributes()
                .await
                .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
            Ok(attributes)
        });
        let _ = resp_tx.send(result);
    });
    resp_rx.recv().unwrap()
}

fn get_at_spi_worker() -> &'static std::sync::mpsc::Sender<(
    (),
    std::sync::mpsc::Sender<Result<LinuxEngine, AutomationError>>,
)> {
    AT_SPI_INIT_WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<((), mpsc::Sender<Result<LinuxEngine, AutomationError>>)>();
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            for (_, resp_tx) in rx {
                let result = rt.block_on(async {
                    set_session_accessibility(true).await?;
                    let accessibility_connection = AccessibilityConnection::new().await?;
                    let connection = Arc::new(accessibility_connection.connection().clone());
                    let registry = AccessibleProxy::builder(&connection)
                        .destination(REGISTRY_DEST)?
                        .path(REGISTRY_PATH)?
                        .interface(ACCESSIBLE_INTERFACE)?
                        .cache_properties(CacheProperties::No)
                        .build()
                        .await?;

                    let root = ThreadSafeLinuxUIElement(Arc::new(registry));
                    Ok(LinuxEngine { connection, root })
                });
                let _ = resp_tx.send(result);
            }
        });
        tx
    })
}

impl LinuxEngine {
    pub fn new(_use_background_apps: bool, _activate_app: bool) -> Result<Self, AutomationError> {
        let (resp_tx, resp_rx) = mpsc::channel();
        get_at_spi_worker().send(((), resp_tx)).unwrap();
        resp_rx.recv().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to receive result from AT-SPI worker: {}",
                e
            ))
        })?
    }
}

#[async_trait::async_trait]
impl AccessibilityEngine for LinuxEngine {
    fn get_root_element(&self) -> UIElement {
        UIElement::new(Box::new(LinuxUIElement {
            connection: Arc::clone(&self.connection),
            destination: self.root.0.inner().destination().to_string(),
            path: self.root.0.inner().path().to_string(),
        }))
    }

    fn get_element_by_id(&self, _id: i32) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "get_element_by_id not yet implemented for Linux".to_string(),
        ))
    }

    fn get_focused_element(&self) -> Result<UIElement, AutomationError> {
        let (resp_tx, resp_rx) = std::sync::mpsc::channel();
        let this = self.clone();
        let req = Box::new(move || {
            erase_future(async move {
                let apps = this.get_applications()?;
                match find_focused_element_async(&apps).await {
                    Ok(element) => Ok(vec![element]),
                    Err(e) => Err(e),
                }
            })
        });
        get_worker().send((req, resp_tx)).unwrap();
        match resp_rx.recv().unwrap() {
            Ok(mut v) => v.pop().ok_or_else(|| {
                AutomationError::ElementNotFound("No focused element found".to_string())
            }),
            Err(e) => Err(e),
        }
    }

    fn get_applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        let (resp_tx, resp_rx) = mpsc::channel();
        let this = self.clone();
        let req = Box::new(move || {
            erase_future(async move {
                let apps = this
                    .root
                    .0
                    .get_children()
                    .await
                    .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
                let mut elements = Vec::new();
                for app in apps {
                    let proxy = app
                        .into_accessible_proxy(this.connection.as_ref())
                        .await
                        .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
                    let child_count = proxy.child_count().await?;
                    if child_count > 0 {
                        elements.push(UIElement::new(Box::new(LinuxUIElement {
                            connection: Arc::clone(&this.connection),
                            destination: proxy.inner().destination().to_string(),
                            path: proxy.inner().path().to_string(),
                        })));
                    }
                }
                Ok(elements)
            })
        });
        get_worker().send((req, resp_tx)).unwrap();
        resp_rx.recv().unwrap()
    }

    fn get_application_by_name(&self, name: &str) -> Result<UIElement, AutomationError> {
        let apps = self.get_applications()?;
        for app in apps {
            if let Some(app_name) = app.name() {
                if app_name.to_lowercase() == name.to_lowercase() {
                    return Ok(app);
                }
            }
        }
        Err(AutomationError::ElementNotFound(format!(
            "No application found with name '{}'",
            name
        )))
    }

    fn get_application_by_pid(
        &self,
        pid: i32,
        _timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError> {
        let apps = self.get_applications()?;
        for app in apps {
            if let Ok(app_pid) = app.process_id() {
                if app_pid as i32 == pid {
                    return Ok(app);
                }
            }
        }
        Err(AutomationError::ElementNotFound(format!(
            "No application found with PID {}",
            pid
        )))
    }

    fn find_element(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
        timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError> {
        let elements = find_elements_sync(self, selector, root, timeout, Some(1))?;
        elements
            .into_iter()
            .next()
            .ok_or_else(|| AutomationError::ElementNotFound("No element found".to_string()))
    }

    fn find_elements(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
        timeout: Option<Duration>,
        depth: Option<usize>,
    ) -> Result<Vec<UIElement>, AutomationError> {
        find_elements_sync(self, selector, root, timeout, depth)
    }

    fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError> {
        use std::process::{Command, Stdio};

        fn get_latest_pid(app_name: &str) -> Option<i32> {
            let output = Command::new("pgrep")
                .arg("-f")
                .arg("-n")
                .arg(app_name)
                .output()
                .ok()?;

            if output.status.success() {
                String::from_utf8(output.stdout).ok()?.trim().parse().ok()
            } else {
                None
            }
        }

        let app_name = app_name.to_string();
        let mut tried_pids = Vec::new();

        let mut output = Command::new(&app_name)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| AutomationError::PlatformError(e.to_string()));

        if output.is_err() {
            debug!("Direct launch of '{}' failed, trying gtk-launch", app_name);
            output = Command::new("gtk-launch")
                .arg(&app_name)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| AutomationError::PlatformError(format!("gtk-launch failed: {}", e)));
        }

        let output = output?;
        let mut pid = output.id() as i32;
        tried_pids.push(pid);

        // Check if original PID exists
        let process_exists = Command::new("ps")
            .arg(pid.to_string())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !process_exists {
            if let Some(new_pid) = get_latest_pid(&app_name) {
                pid = new_pid;
                tried_pids.push(pid);
            } else {
                // Fallback: try to get by name before giving up
                debug!("No process found for '{}' using pgrep, falling back to get_application_by_name", app_name);
                if let Ok(app) = self.get_application_by_name(&app_name) {
                    debug!("Found application '{}' by name fallback", app_name);
                    return Ok(app);
                }
                return Err(AutomationError::ElementNotFound(format!(
                    "No process found for '{}' using pgrep (tried pids: {:?}) and not found by name",
                    app_name, tried_pids
                )));
            }
        }

        // Try original PID for 5s
        for _ in 0..5 {
            if let Ok(app) = self.get_application_by_pid(pid, None) {
                debug!("Found application '{}' with pid {}", app_name, pid);
                return Ok(app);
            }
            std::thread::sleep(Duration::from_millis(1000));
        }

        // Then try with pgrep PID
        for _ in 0..5 {
            if let Some(new_pid) = get_latest_pid(&app_name) {
                if !tried_pids.contains(&new_pid) {
                    tried_pids.push(new_pid);
                }
                pid = new_pid;
                if let Ok(app) = self.get_application_by_pid(pid, None) {
                    debug!("Found application '{}' with pgrep pid {}", app_name, pid);
                    return Ok(app);
                }
            }
            std::thread::sleep(Duration::from_millis(1000));
        }

        // Final fallback: try to get by name before giving up
        debug!("Application '{}' not found by PID after launch, falling back to get_application_by_name", app_name);
        if let Ok(app) = self.get_application_by_name(&app_name) {
            debug!("Found application '{}' by name fallback at end", app_name);
            return Ok(app);
        }

        Err(AutomationError::ElementNotFound(format!(
            "Application '{}' not found after launch (tried pids: {:?}) and not found by name",
            app_name, tried_pids
        )))
    }

    fn activate_application(&self, app_name: &str) -> Result<(), AutomationError> {
        let (resp_tx, resp_rx) = mpsc::channel();
        let this = self.clone();
        let app_name = app_name.to_string();
        let req = Box::new(move || {
            erase_future(async move {
                let selector = Selector::Role {
                    role: "application".to_string(),
                    name: Some(app_name.clone()),
                };
                if let Ok(app) = this.find_element(&selector, None, None) {
                    // Try to find a window to activate
                    let mut windows = Vec::new();
                    for child in app.children()? {
                        if child.role() == "frame" || child.role() == "window" {
                            windows.push(child);
                        }
                    }
                    if let Some(window) = windows.first() {
                        window.focus()?;
                        return Ok(vec![]);
                    }
                    // If no window found, try focusing the application itself
                    app.focus()?;
                    Ok(vec![])
                } else {
                    Err(AutomationError::ElementNotFound(format!(
                        "Application '{}' not found",
                        app_name
                    )))
                }
            })
        });
        get_worker().send((req, resp_tx)).unwrap();
        match resp_rx.recv().unwrap() {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn open_url(
        &self,
        _url: &str,
        _browser: Option<crate::Browser>,
    ) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn open_file(&self, _file_path: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    async fn run_command(
        &self,
        _windows_command: Option<&str>,
        unix_command: Option<&str>,
    ) -> Result<CommandOutput, AutomationError> {
        let command = unix_command.ok_or_else(|| {
            AutomationError::InvalidArgument("Unix command is required for Linux".to_string())
        })?;

        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| {
                AutomationError::PlatformError(format!("Failed to execute command: {}", e))
            })?;

        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_status: Some(output.status.code().unwrap_or(-1)),
        })
    }

    async fn capture_screen(&self) -> Result<ScreenshotResult, AutomationError> {
        let monitors = xcap::Monitor::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitors: {}", e))
        })?;
        let mut primary_monitor: Option<xcap::Monitor> = None;
        for monitor in monitors {
            match monitor.is_primary() {
                Ok(true) => {
                    primary_monitor = Some(monitor);
                    break;
                }
                Ok(false) => continue,
                Err(e) => {
                    return Err(AutomationError::PlatformError(format!(
                        "Error checking monitor primary status: {}",
                        e
                    )));
                }
            }
        }
        let primary_monitor = primary_monitor.ok_or_else(|| {
            AutomationError::PlatformError("Could not find primary monitor".to_string())
        })?;

        let image = primary_monitor.capture_image().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to capture screen: {}", e))
        })?;

        Ok(ScreenshotResult {
            image_data: image.to_vec(),
            width: image.width(),
            height: image.height(),
            monitor: None,
        })
    }

    // ============== NEW MONITOR ABSTRACTIONS ==============

    async fn list_monitors(&self) -> Result<Vec<crate::Monitor>, AutomationError> {
        let monitors = xcap::Monitor::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitors: {}", e))
        })?;

        let mut result = Vec::new();
        for (index, monitor) in monitors.iter().enumerate() {
            let name = monitor.name().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor name: {}", e))
            })?;

            let is_primary = monitor.is_primary().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to check primary status: {}", e))
            })?;

            let width = monitor.width().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor width: {}", e))
            })?;

            let height = monitor.height().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor height: {}", e))
            })?;

            let x = monitor.x().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor x position: {}", e))
            })?;

            let y = monitor.y().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor y position: {}", e))
            })?;

            let scale_factor = monitor.scale_factor().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to get monitor scale factor: {}", e))
            })? as f64;

            result.push(crate::Monitor {
                id: format!("monitor_{}", index),
                name,
                is_primary,
                width,
                height,
                x,
                y,
                scale_factor,
            });
        }

        Ok(result)
    }

    async fn get_primary_monitor(&self) -> Result<crate::Monitor, AutomationError> {
        let monitors = self.list_monitors().await?;
        monitors
            .into_iter()
            .find(|m| m.is_primary)
            .ok_or_else(|| AutomationError::PlatformError("No primary monitor found".to_string()))
    }

    async fn get_active_monitor(&self) -> Result<crate::Monitor, AutomationError> {
        // Get all windows
        let windows = xcap::Window::all()
            .map_err(|e| AutomationError::PlatformError(format!("Failed to get windows: {}", e)))?;

        // Find the focused window
        let focused_window = windows
            .iter()
            .find(|w| w.is_focused().unwrap_or(false))
            .ok_or_else(|| {
                AutomationError::ElementNotFound("No focused window found".to_string())
            })?;

        // Get the monitor for the focused window
        let xcap_monitor = focused_window.current_monitor().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get current monitor: {}", e))
        })?;

        // Convert to our Monitor struct
        let name = xcap_monitor.name().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor name: {}", e))
        })?;

        let is_primary = xcap_monitor.is_primary().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to check primary status: {}", e))
        })?;

        // Find the monitor index for ID generation
        let monitors = xcap::Monitor::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitors: {}", e))
        })?;

        let monitor_index = monitors
            .iter()
            .position(|m| m.name().map(|n| n == name).unwrap_or(false))
            .unwrap_or(0);

        let width = xcap_monitor.width().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor width: {}", e))
        })?;

        let height = xcap_monitor.height().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor height: {}", e))
        })?;

        let x = xcap_monitor.x().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor x position: {}", e))
        })?;

        let y = xcap_monitor.y().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor y position: {}", e))
        })?;

        let scale_factor = xcap_monitor.scale_factor().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitor scale factor: {}", e))
        })? as f64;

        Ok(crate::Monitor {
            id: format!("monitor_{}", monitor_index),
            name,
            is_primary,
            width,
            height,
            x,
            y,
            scale_factor,
        })
    }

    async fn get_monitor_by_id(&self, id: &str) -> Result<crate::Monitor, AutomationError> {
        let monitors = self.list_monitors().await?;
        monitors.into_iter().find(|m| m.id == id).ok_or_else(|| {
            AutomationError::ElementNotFound(format!("Monitor with ID '{}' not found", id))
        })
    }

    async fn get_monitor_by_name(&self, name: &str) -> Result<crate::Monitor, AutomationError> {
        let monitors = self.list_monitors().await?;
        monitors
            .into_iter()
            .find(|m| m.name == name)
            .ok_or_else(|| {
                AutomationError::ElementNotFound(format!("Monitor '{}' not found", name))
            })
    }

    async fn capture_monitor_by_id(
        &self,
        id: &str,
    ) -> Result<crate::ScreenshotResult, AutomationError> {
        let monitor = self.get_monitor_by_id(id).await?;

        // Find the xcap monitor by name
        let monitors = xcap::Monitor::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitors: {}", e))
        })?;

        let xcap_monitor = monitors
            .into_iter()
            .find(|m| m.name().map(|n| n == monitor.name).unwrap_or(false))
            .ok_or_else(|| {
                AutomationError::ElementNotFound(format!("Monitor '{}' not found", monitor.name))
            })?;

        let image = xcap_monitor.capture_image().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to capture monitor '{}': {}",
                monitor.name, e
            ))
        })?;

        Ok(ScreenshotResult {
            image_data: image.to_vec(),
            width: image.width(),
            height: image.height(),
            monitor: Some(monitor),
        })
    }

    // ============== DEPRECATED METHODS ==============

    // Method kept for backward compatibility; not recommended for new code
    async fn capture_monitor_by_name(
        &self,
        name: &str,
    ) -> Result<ScreenshotResult, AutomationError> {
        let monitors = xcap::Monitor::all().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to get monitors: {}", e))
        })?;
        let mut target_monitor: Option<xcap::Monitor> = None;
        for monitor in monitors {
            match monitor.name() {
                Ok(monitor_name) if monitor_name == name => {
                    target_monitor = Some(monitor);
                    break;
                }
                Ok(_) => continue,
                Err(e) => {
                    return Err(AutomationError::PlatformError(format!(
                        "Error getting monitor name: {}",
                        e
                    )));
                }
            }
        }
        let target_monitor = target_monitor.ok_or_else(|| {
            AutomationError::ElementNotFound(format!("Monitor '{}' not found", name))
        })?;

        let image = target_monitor.capture_image().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to capture monitor '{}': {}", name, e))
        })?;

        Ok(ScreenshotResult {
            image_data: image.to_vec(),
            width: image.width(),
            height: image.height(),
            monitor: None,
        })
    }

    async fn ocr_image_path(&self, image_path: &str) -> Result<String, AutomationError> {
        let engine = OcrEngine::new(OcrProvider::Auto).map_err(|e| {
            AutomationError::PlatformError(format!("Failed to create OCR engine: {}", e))
        })?;

        let (text, _language, _confidence) = engine // Destructure the tuple
            .recognize_file(image_path)
            .await
            .map_err(|e| {
                AutomationError::PlatformError(format!("OCR recognition failed: {}", e))
            })?;

        Ok(text) // Return only the text
    }

    async fn ocr_screenshot(
        &self,
        screenshot: &ScreenshotResult,
    ) -> Result<String, AutomationError> {
        // Reconstruct the image buffer from raw data
        let img_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
            screenshot.width,
            screenshot.height,
            screenshot.image_data.clone(), // Clone data into the buffer
        )
        .ok_or_else(|| {
            AutomationError::InvalidArgument(
                "Invalid screenshot data for buffer creation".to_string(),
            )
        })?;

        // Convert to DynamicImage
        let dynamic_image = DynamicImage::ImageRgba8(img_buffer);

        // Directly await the OCR operation within the existing async context
        let engine = OcrEngine::new(OcrProvider::Auto).map_err(|e| {
            AutomationError::PlatformError(format!("Failed to create OCR engine: {}", e))
        })?;

        let (text, _language, _confidence) = engine
            .recognize_image(&dynamic_image) // Use recognize_image
            .await // << Directly await here
            .map_err(|e| {
                AutomationError::PlatformError(format!("OCR recognition failed: {}", e))
            })?;

        Ok(text)
    }

    fn activate_browser_window_by_title(&self, _title: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    async fn get_current_browser_window(&self) -> Result<UIElement, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "get_current_browser_window not yet implemented for Linux".to_string(),
        ))
    }

    async fn get_current_window(&self) -> Result<UIElement, AutomationError> {
        let applications = self.get_applications()?;
        let mut all_windows = Vec::new();
        for application in &applications {
            if let Ok(children) = application.children() {
                all_windows.extend(children);
            }
        }
        find_focused_element_async(&all_windows).await
    }

    async fn get_current_application(&self) -> Result<UIElement, AutomationError> {
        let apps = self.get_applications()?;
        find_focused_element_async(&apps).await
    }

    fn get_window_tree(
        &self,
        pid: u32,
        title: Option<&str>,
        _config: crate::platforms::TreeBuildConfig,
    ) -> Result<crate::UINode, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(format!(
            "get_window_tree for PID {} and title {:?} not yet implemented for Linux",
            pid, title
        )))
    }

    // Method kept for backward compatibility; not recommended for new code
    async fn get_active_monitor_name(&self) -> Result<String, AutomationError> {
        let monitor = self.get_active_monitor().await?;
        Ok(monitor.name)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn press_key(&self, _key: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "press_key is not implemented for LinuxEngine yet".to_string(),
        ))
    }

    fn zoom_in(&self, _level: u32) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "zoom_in is not implemented for LinuxEngine yet".to_string(),
        ))
    }

    fn zoom_out(&self, _level: u32) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "zoom_out is not implemented for LinuxEngine yet".to_string(),
        ))
    }

    fn set_zoom(&self, _percentage: u32) -> Result<(), AutomationError> {
        // On Linux, zoom is typically controlled via Ctrl+Plus/Minus/0
        // The implementation would be similar to Windows
        // For now, returning unimplemented
        Err(AutomationError::UnsupportedOperation(
            "set_zoom is not implemented for LinuxEngine yet".to_string(),
        ))
    }
}

impl LinuxUIElement {
    fn description(&self) -> Option<String> {
        let (resp_tx, resp_rx): OptionStringChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result: Result<Option<String>, AutomationError> = rt.block_on(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                    .path(this.path.as_str())
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?
                    .build()
                    .await
                    .map_err(|e| AutomationError::PlatformError(e.to_string()))?;
                Ok(proxy.description().await.ok())
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap().unwrap_or(None)
    }
}

impl UIElementImpl for LinuxUIElement {
    fn object_id(&self) -> usize {
        generate_element_id(&self.connection, &self.destination, &self.path).unwrap_or(0)
    }

    fn id(&self) -> Option<String> {
        Some(self.object_id().to_string())
    }

    fn role(&self) -> String {
        let (resp_tx, resp_rx): StringChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let builder = AccessibleProxy::builder(&this.connection);
                let builder = match builder.destination(this.destination.as_str()) {
                    Ok(b) => b,
                    Err(_) => return Ok(String::new()),
                };
                let builder = match builder.path(this.path.as_str()) {
                    Ok(b) => b,
                    Err(_) => return Ok(String::new()),
                };
                let proxy_result = builder.build().await;
                let role = match proxy_result {
                    Ok(proxy) => proxy
                        .get_role()
                        .await
                        .map(|r| r.to_string())
                        .unwrap_or_default(),
                    Err(_) => String::new(),
                };
                Ok(role)
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap().unwrap_or_default()
    }

    fn attributes(&self) -> UIElementAttributes {
        let mut properties = HashMap::new();
        if let Ok(attrs) = get_accessible_attributes(self) {
            properties = attrs
                .into_iter()
                .map(|(k, v)| (k, Some(serde_json::Value::String(v))))
                .collect();
        }

        UIElementAttributes {
            role: self.role(),
            name: self.name(),
            label: self.name(),
            value: self.get_text(0).ok(),
            description: self.description(),
            text: self.get_text(0).ok(),
            is_keyboard_focusable: self.is_keyboard_focusable().ok(),
            is_focused: self.is_focused().ok(),
            enabled: self.is_enabled().ok(),
            bounds: self.bounds().ok(),
            is_toggled: None,
            is_selected: None,
            child_count: None,
            index_in_parent: None,
            properties,
        }
    }

    fn name(&self) -> Option<String> {
        let (resp_tx, resp_rx): OptionStringChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())
                    .map_err(|_| AutomationError::PlatformError("No destination".to_string()))?
                    .path(this.path.as_str())
                    .map_err(|_| AutomationError::PlatformError("No path".to_string()))?
                    .build()
                    .await
                    .map_err(|_| AutomationError::PlatformError("Build failed".to_string()))?;
                Ok(proxy.name().await.ok())
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap().unwrap_or(None)
    }

    fn children(&self) -> Result<Vec<UIElement>, AutomationError> {
        let (resp_tx, resp_rx) = mpsc::channel();
        let this = self.clone();
        let req = Box::new(move || {
            erase_future(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let children = proxy
                    .get_children()
                    .await
                    .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
                let mut elements = Vec::new();
                for child in children {
                    let proxy = child
                        .into_accessible_proxy(this.connection.as_ref())
                        .await
                        .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
                    elements.push(UIElement::new(Box::new(LinuxUIElement {
                        connection: std::sync::Arc::clone(&this.connection),
                        destination: proxy.inner().destination().to_string(),
                        path: proxy.inner().path().to_string(),
                    })));
                }
                Ok(elements)
            })
        });
        get_worker().send((req, resp_tx)).unwrap();
        resp_rx.recv().unwrap()
    }

    fn parent(&self) -> Result<Option<UIElement>, AutomationError> {
        let (resp_tx, resp_rx): OptionUIElementChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                if let Ok(parent) = proxy.parent().await {
                    let proxy = parent
                        .into_accessible_proxy(this.connection.as_ref())
                        .await
                        .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
                    Ok(Some(UIElement::new(Box::new(LinuxUIElement {
                        connection: std::sync::Arc::clone(&this.connection),
                        destination: proxy.inner().destination().to_string(),
                        path: proxy.inner().path().to_string(),
                    }))))
                } else {
                    Ok(None)
                }
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError> {
        let (resp_tx, resp_rx): BoundsChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let component = ComponentProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let extents = component.get_extents(CoordType::Screen).await?;
                Ok((
                    extents.0 as f64,
                    extents.1 as f64,
                    extents.2 as f64,
                    extents.3 as f64,
                ))
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn click(&self) -> Result<ClickResult, AutomationError> {
        let (resp_tx, resp_rx): ClickResultChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let component = ComponentProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let extents = component.get_extents(CoordType::Screen).await?;
                let x = extents.0 + (extents.2 / 2);
                let y = extents.1 + (extents.3 / 2);
                let device_controller = DeviceEventControllerProxy::new(&this.connection).await?;
                device_controller.generate_mouse_event(x, y, "b1p").await?;
                device_controller.generate_mouse_event(x, y, "b1r").await?;
                Ok(ClickResult {
                    method: "click".to_string(),
                    coordinates: Some((x as f64, y as f64)),
                    details: format!("Clicked at ({}, {})", x, y),
                })
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn double_click(&self) -> Result<ClickResult, AutomationError> {
        let (resp_tx, resp_rx): ClickResultChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let component = ComponentProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let extents = component.get_extents(CoordType::Screen).await?;
                let x = extents.0 + (extents.2 / 2);
                let y = extents.1 + (extents.3 / 2);
                let device_controller = DeviceEventControllerProxy::new(&this.connection).await?;
                device_controller.generate_mouse_event(x, y, "b1p").await?;
                device_controller.generate_mouse_event(x, y, "b1c").await?;
                device_controller.generate_mouse_event(x, y, "b1c").await?;
                device_controller.generate_mouse_event(x, y, "b1r").await?;
                Ok(ClickResult {
                    method: "double_click".to_string(),
                    coordinates: Some((x as f64, y as f64)),
                    details: format!("Double clicked at ({}, {})", x, y),
                })
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn right_click(&self) -> Result<(), AutomationError> {
        let (resp_tx, resp_rx): UnitChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let component = ComponentProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let extents = component.get_extents(CoordType::Screen).await?;
                let x = extents.0 + (extents.2 / 2);
                let y = extents.1 + (extents.3 / 2);
                let device_controller = DeviceEventControllerProxy::new(&this.connection).await?;
                device_controller.generate_mouse_event(x, y, "b3p").await?;
                device_controller.generate_mouse_event(x, y, "b3r").await?;
                Ok(())
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn hover(&self) -> Result<(), AutomationError> {
        let (resp_tx, resp_rx): UnitChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let component = ComponentProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let extents = component.get_extents(CoordType::Screen).await?;
                let x = extents.0 + (extents.2 / 2);
                let y = extents.1 + (extents.3 / 2);
                let device_controller = DeviceEventControllerProxy::new(&this.connection).await?;
                device_controller.generate_mouse_event(x, y, "abs").await?;
                Ok(())
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn focus(&self) -> Result<(), AutomationError> {
        let (resp_tx, resp_rx): UnitChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let states = proxy.get_state().await?;
                if !states.contains(state::State::Focusable) {
                    return Err(AutomationError::UnsupportedOperation(
                        "Element is not focusable".to_string(),
                    ));
                }
                let component = ComponentProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let focused = component.grab_focus().await?;
                if !focused {
                    return Err(AutomationError::UnsupportedOperation(
                        "Failed to focus element".to_string(),
                    ));
                }
                Ok(())
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn type_text(&self, text: &str, _use_clipboard: bool) -> Result<(), AutomationError> {
        let (resp_tx, resp_rx): UnitChannel = std::sync::mpsc::channel();
        let this = self.clone();
        let text = text.to_string();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let states = proxy.get_state().await?;
                if !states.contains(state::State::Focusable) {
                    return Err(AutomationError::UnsupportedOperation(
                        "Element is not focusable".to_string(),
                    ));
                }
                let device_controller = DeviceEventControllerProxy::new(&this.connection).await?;
                device_controller
                    .generate_keyboard_event(0, "", KeySynthType::Press)
                    .await?;
                device_controller
                    .generate_keyboard_event(0, "", KeySynthType::Release)
                    .await?;
                if let Ok(text_proxy) = TextProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await
                {
                    let char_count = text_proxy.character_count().await?;
                    text_proxy.get_text(0, char_count).await?;
                    text_proxy.get_text(0, text.len() as i32).await?;
                    Ok(())
                } else {
                    for c in text.chars() {
                        device_controller
                            .generate_keyboard_event(c as i32, &c.to_string(), KeySynthType::String)
                            .await?;
                    }
                    Ok(())
                }
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn press_key(&self, key: &str) -> Result<(), AutomationError> {
        let (resp_tx, resp_rx): UnitChannel = std::sync::mpsc::channel();
        let this = self.clone();
        let key = key.to_string();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let _proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())
                    .map_err(|_| AutomationError::PlatformError("No destination".to_string()))?
                    .path(this.path.as_str())
                    .map_err(|_| AutomationError::PlatformError("No path".to_string()))?
                    .build()
                    .await?;
                let keysym = match key.to_lowercase().as_str() {
                    "enter" => 0xff0d,
                    "tab" => 0xff09,
                    "space" => 0x020,
                    "backspace" => 0xff08,
                    "delete" => 0xffff,
                    "escape" => 0xff1b,
                    "up" => 0xff52,
                    "down" => 0xff54,
                    "left" => 0xff51,
                    "right" => 0xff53,
                    _ => key.chars().next().map(|c| c as i32).unwrap_or(0),
                };
                let device_controller = DeviceEventControllerProxy::new(&this.connection).await?;
                device_controller
                    .generate_keyboard_event(keysym, "", KeySynthType::Press)
                    .await?;
                device_controller
                    .generate_keyboard_event(keysym, "", KeySynthType::Release)
                    .await?;
                Ok(())
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn mouse_click_and_hold(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        let (resp_tx, resp_rx): UnitChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let component = ComponentProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let extents = component.get_extents(CoordType::Screen).await?;
                let abs_x = (extents.0 as f64 + x) as i32;
                let abs_y = (extents.1 as f64 + y) as i32;
                let device_controller = DeviceEventControllerProxy::new(&this.connection).await?;
                device_controller
                    .generate_mouse_event(abs_x, abs_y, "b1p")
                    .await?;
                Ok(())
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn mouse_move(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        let (resp_tx, resp_rx): UnitChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let component = ComponentProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let extents = component.get_extents(CoordType::Screen).await?;
                let abs_x = (extents.0 as f64 + x) as i32;
                let abs_y = (extents.1 as f64 + y) as i32;
                let device_controller = DeviceEventControllerProxy::new(&this.connection).await?;
                device_controller
                    .generate_mouse_event(abs_x, abs_y, "abs")
                    .await?;
                Ok(())
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn mouse_release(&self) -> Result<(), AutomationError> {
        let (resp_tx, resp_rx): UnitChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let component = ComponentProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let extents = component.get_extents(CoordType::Screen).await?;
                let device_controller = DeviceEventControllerProxy::new(&this.connection).await?;
                device_controller
                    .generate_mouse_event(extents.0, extents.1, "b1r")
                    .await?;
                Ok(())
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn mouse_drag(
        &self,
        _start_x: f64,
        _start_y: f64,
        _end_x: f64,
        _end_y: f64,
    ) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn get_text(&self, _max_depth: usize) -> Result<String, AutomationError> {
        let (resp_tx, resp_rx): StringChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let text_proxy = TextProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let char_count = text_proxy.character_count().await?;
                Ok(text_proxy.get_text(0, char_count).await?)
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn is_keyboard_focusable(&self) -> Result<bool, AutomationError> {
        let (resp_tx, resp_rx): BoolChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let states = proxy.get_state().await?;
                Ok(states.contains(state::State::Focusable))
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn perform_action(&self, action: &str) -> Result<(), AutomationError> {
        let (resp_tx, resp_rx): UnitChannel = std::sync::mpsc::channel();
        let this = self.clone();
        let action = action.to_string();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                debug!("Performing action: {:?}", action);
                let action_proxy = ActionProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .interface("org.a11y.atspi.Action")?
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await?;
                let mut found = false;
                let mut i = 0;
                loop {
                    match action_proxy.get_name(i).await {
                        Ok(name) => {
                            if name.to_lowercase() == action.to_lowercase() {
                                action_proxy.do_action(i).await?;
                                found = true;
                                break;
                            }
                        }
                        Err(_) => break, // No more actions
                    }
                    i += 1;
                }
                if found {
                    Ok(())
                } else {
                    Err(AutomationError::UnsupportedOperation(format!(
                        "Action '{}' not supported for this element",
                        action
                    )))
                }
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn create_locator(&self, selector: Selector) -> Result<Locator, AutomationError> {
        // Create a new LinuxEngine instance (with default args)
        let engine = LinuxEngine::new(false, false)?;
        // Wrap self as a UIElement
        let self_element = UIElement::new(Box::new(self.clone()));
        // Create a locator for the selector with the engine, then set root to this element
        let locator = Locator::new(std::sync::Arc::new(engine), selector).within(self_element);
        Ok(locator)
    }

    fn scroll(&self, _direction: &str, _amount: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn application(&self) -> Result<Option<UIElement>, AutomationError> {
        let (resp_tx, resp_rx): OptionUIElementChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                if let Ok(application) = proxy.get_application().await {
                    Ok(Some(UIElement::new(Box::new(LinuxUIElement {
                        connection: std::sync::Arc::clone(&this.connection),
                        destination: application.name.to_string(),
                        path: application.path.to_string(),
                    }))))
                } else {
                    Ok(None)
                }
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn window(&self) -> Result<Option<UIElement>, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn highlight(
        &self,
        _color: Option<u32>,
        _duration: Option<std::time::Duration>,
    ) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn activate_window(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn minimize_window(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn maximize_window(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn process_id(&self) -> Result<u32, AutomationError> {
        let (resp_tx, resp_rx): U32Channel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let dbus_proxy = DBusProxy::new(&this.connection).await?;
                // Use the proxy's destination (bus name) directly for PID lookup, with correct type conversion
                let bus_name = match proxy.inner().destination().as_str().try_into() {
                    Ok(b) => b,
                    Err(e) => {
                        return Err(AutomationError::PlatformError(format!(
                            "Failed to convert bus name: {}",
                            e
                        )))
                    }
                };
                if let Ok(pid) = dbus_proxy.get_connection_unix_process_id(bus_name).await {
                    return Ok(pid);
                }
                Err(AutomationError::PlatformError(format!(
                    "Failed to get process ID for bus name: {}",
                    this.destination
                )))
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn clone_box(&self) -> Box<dyn UIElementImpl> {
        Box::new(LinuxUIElement {
            connection: Arc::clone(&self.connection),
            destination: self.destination.clone(),
            path: self.path.clone(),
        })
    }

    fn is_enabled(&self) -> Result<bool, AutomationError> {
        let (resp_tx, resp_rx): BoolChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let states = proxy.get_state().await?;
                Ok(states.contains(state::State::Enabled))
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn is_visible(&self) -> Result<bool, AutomationError> {
        let (resp_tx, resp_rx): BoolChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let states = proxy.get_state().await?;
                Ok(states.contains(state::State::Visible))
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn is_focused(&self) -> Result<bool, AutomationError> {
        let (resp_tx, resp_rx): BoolChannel = std::sync::mpsc::channel();
        let this = self.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let states = proxy.get_state().await?;
                Ok(states.contains(state::State::Focused))
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn set_value(&self, value: &str) -> Result<(), AutomationError> {
        let (resp_tx, resp_rx): UnitChannel = std::sync::mpsc::channel();
        let this = self.clone();
        let value = value.to_string();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async move {
                let proxy = AccessibleProxy::builder(&this.connection)
                    .destination(this.destination.as_str())?
                    .path(this.path.as_str())?
                    .build()
                    .await?;
                let states = proxy.get_state().await?;
                if !states.contains(state::State::Focusable) {
                    return Err(AutomationError::UnsupportedOperation(
                        "Element is not focusable".to_string(),
                    ));
                }
                let device_controller = DeviceEventControllerProxy::new(&this.connection).await?;
                device_controller
                    .generate_keyboard_event(0, "", KeySynthType::Press)
                    .await?;
                device_controller
                    .generate_keyboard_event(0, "", KeySynthType::Release)
                    .await?;
                for c in value.chars() {
                    device_controller
                        .generate_keyboard_event(c as i32, &c.to_string(), KeySynthType::String)
                        .await
                        .map_err(|e: zbus::Error| AutomationError::PlatformError(e.to_string()))?;
                }
                Ok(())
            });
            let _ = resp_tx.send(result);
        });
        resp_rx.recv().unwrap()
    }

    fn capture(&self) -> Result<ScreenshotResult, AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn set_transparency(&self, _percentage: u8) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn close(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedPlatform(
            "Linux implementation is not yet available".to_string(),
        ))
    }

    fn url(&self) -> Option<String> {
        None
    }

    fn select_option(&self, _option_name: &str) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "select_option is not implemented for Linux yet".to_string(),
        ))
    }

    fn list_options(&self) -> Result<Vec<String>, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "list_options is not implemented for Linux yet".to_string(),
        ))
    }

    fn is_toggled(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "is_toggled is not implemented for Linux yet".to_string(),
        ))
    }

    fn set_toggled(&self, _state: bool) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "set_toggled is not implemented for Linux yet".to_string(),
        ))
    }

    fn get_range_value(&self) -> Result<f64, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "get_range_value is not implemented for Linux yet".to_string(),
        ))
    }

    fn set_range_value(&self, _value: f64) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "set_range_value is not implemented for Linux yet".to_string(),
        ))
    }

    fn is_selected(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "is_selected is not implemented for Linux yet".to_string(),
        ))
    }

    fn set_selected(&self, _state: bool) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "set_selected is not implemented for Linux yet".to_string(),
        ))
    }

    fn invoke(&self) -> Result<(), AutomationError> {
        self.click().map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_engine_creation() {
        let engine_result = LinuxEngine::new(false, false);
        assert!(
            engine_result.is_ok(),
            "Should be able to create Linux engine"
        );

        if let Ok(engine) = engine_result {
            let root = engine.get_root_element();
            assert!(root.id().is_some(), "Root element should have an ID");
            assert_eq!(
                root.role(),
                "desktop frame",
                "Root element should have 'desktop frame' role"
            );
        }
    }
}
