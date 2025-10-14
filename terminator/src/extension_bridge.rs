use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot, Mutex, RwLock},
    task::JoinHandle,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

use crate::AutomationError;

#[derive(Debug, thiserror::Error)]
pub enum ExtensionBridgeError {
    #[error("Failed to bind to port {port}: {source}")]
    PortBindError {
        port: u16,
        #[source]
        source: std::io::Error,
    },
    #[error("Port {port} is in use by another process (PID: {pid})")]
    PortInUse { port: u16, pid: u32 },
    #[error("Failed to kill existing process: {0}")]
    ProcessKillError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

const DEFAULT_WS_ADDR: &str = "127.0.0.1:17373";

// Reduce type complexity for Clippy
type BridgeResult = Result<serde_json::Value, String>;
type PendingMap = HashMap<String, oneshot::Sender<BridgeResult>>;
type Pending = Arc<Mutex<PendingMap>>;
type Clients = Arc<Mutex<Vec<Client>>>;

#[derive(Debug, Serialize, Deserialize)]
struct EvalRequest {
    id: String,
    action: String,
    code: String,
    #[serde(default)]
    await_promise: bool,
}

#[derive(Debug, Serialize)]
struct ResetRequest {
    action: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum BridgeIncoming {
    EvalResult {
        id: String,
        ok: bool,
        result: Option<serde_json::Value>,
        error: Option<String>,
        #[serde(rename = "tabId")]
        tab_id: Option<u32>,
    },
    Typed(TypedIncoming),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum TypedIncoming {
    #[serde(rename = "hello")]
    Hello { from: Option<String> },
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "console_event")]
    ConsoleEvent {
        id: String,
        level: Option<String>,
        args: Option<serde_json::Value>,
        #[serde(rename = "stackTrace")]
        stack_trace: Option<serde_json::Value>,
        ts: Option<f64>,
    },
    #[serde(rename = "exception_event")]
    ExceptionEvent {
        id: String,
        details: Option<serde_json::Value>,
    },
    #[serde(rename = "log_event")]
    LogEvent {
        id: String,
        entry: Option<serde_json::Value>,
    },
    #[serde(rename = "network_request")]
    NetworkRequest {
        #[serde(rename = "tabId")]
        tab_id: u32,
        #[serde(rename = "requestId")]
        request_id: String,
        request: NetworkRequestData,
        #[serde(rename = "documentURL")]
        document_url: Option<String>,
        timestamp: f64,
        #[serde(rename = "wallTime")]
        #[allow(dead_code)]
        wall_time: Option<f64>,
        initiator: Option<serde_json::Value>,
        #[serde(rename = "resourceType")]
        resource_type: Option<String>,
        #[serde(rename = "frameId")]
        #[allow(dead_code)]
        frame_id: Option<String>,
        #[serde(rename = "redirectResponse")]
        #[allow(dead_code)]
        redirect_response: Option<NetworkResponseSummary>,
    },
    #[serde(rename = "network_response")]
    NetworkResponse {
        #[serde(rename = "tabId")]
        tab_id: u32,
        #[serde(rename = "requestId")]
        request_id: String,
        response: NetworkResponseData,
        timestamp: f64,
        #[serde(rename = "resourceType")]
        #[allow(dead_code)]
        resource_type: Option<String>,
        #[serde(rename = "frameId")]
        #[allow(dead_code)]
        frame_id: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRequestData {
    pub url: String,
    pub method: String,
    pub headers: serde_json::Value,
    #[serde(rename = "postData")]
    pub post_data: Option<String>,
    #[serde(rename = "hasPostData")]
    pub has_post_data: Option<bool>,
    #[serde(rename = "initialPriority")]
    pub initial_priority: Option<String>,
    #[serde(rename = "referrerPolicy")]
    pub referrer_policy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkResponseData {
    pub url: String,
    pub status: i32,
    #[serde(rename = "statusText")]
    pub status_text: String,
    pub headers: serde_json::Value,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "requestHeaders")]
    pub request_headers: Option<serde_json::Value>,
    #[serde(rename = "connectionReused")]
    pub connection_reused: Option<bool>,
    #[serde(rename = "connectionId")]
    pub connection_id: Option<i32>,
    #[serde(rename = "remoteIPAddress")]
    pub remote_ip_address: Option<String>,
    #[serde(rename = "remotePort")]
    pub remote_port: Option<i32>,
    #[serde(rename = "fromDiskCache")]
    pub from_disk_cache: Option<bool>,
    #[serde(rename = "fromServiceWorker")]
    pub from_service_worker: Option<bool>,
    #[serde(rename = "fromPrefetchCache")]
    pub from_prefetch_cache: Option<bool>,
    #[serde(rename = "encodedDataLength")]
    pub encoded_data_length: Option<f64>,
    pub timing: Option<serde_json::Value>,
    pub protocol: Option<String>,
    #[serde(rename = "securityState")]
    pub security_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetworkResponseSummary {
    url: String,
    status: i32,
    #[serde(rename = "statusText")]
    status_text: String,
}

#[derive(Debug, Clone)]
pub struct NetworkRequestEntry {
    pub request_id: String,
    pub tab_id: u32,
    pub request: NetworkRequestData,
    pub response: Option<NetworkResponseData>,
    pub document_url: Option<String>,
    pub request_timestamp: f64,
    pub response_timestamp: Option<f64>,
    pub resource_type: Option<String>,
    pub frame_id: Option<String>,
    pub initiator: Option<serde_json::Value>,
}

struct Client {
    sender: mpsc::UnboundedSender<Message>,
    connected_at: std::time::Instant,
}

pub struct ExtensionBridge {
    _server_task: JoinHandle<()>,
    clients: Clients,
    pending: Pending,
}

// Supervised bridge that can auto-restart if the server task dies
static BRIDGE_SUPERVISOR: OnceCell<Arc<RwLock<Option<Arc<ExtensionBridge>>>>> = OnceCell::new();

// Global storage for network requests per tab
static NETWORK_STORAGE: OnceCell<Arc<RwLock<HashMap<u32, Vec<NetworkRequestEntry>>>>> =
    OnceCell::new();

fn get_network_storage() -> Arc<RwLock<HashMap<u32, Vec<NetworkRequestEntry>>>> {
    NETWORK_STORAGE
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}

impl ExtensionBridge {
    pub async fn global() -> Arc<ExtensionBridge> {
        let supervisor = BRIDGE_SUPERVISOR.get_or_init(|| Arc::new(RwLock::new(None)));

        // Check if bridge exists and is alive
        let needs_create = {
            let guard = supervisor.read().await;
            match &*guard {
                None => true,
                Some(bridge) => {
                    // Check if server task is still running
                    bridge._server_task.is_finished()
                }
            }
        };

        if needs_create {
            // Create new bridge
            let mut guard = supervisor.write().await;

            // Double-check after acquiring write lock (another task may have created it)
            let should_create = match &*guard {
                None => true,
                Some(existing) => existing._server_task.is_finished(),
            };

            if should_create {
                if guard.is_some() {
                    tracing::warn!("Extension bridge server task died, recreating...");
                } else {
                    tracing::info!("Creating initial extension bridge...");
                }

                match ExtensionBridge::start(DEFAULT_WS_ADDR).await {
                    Ok(bridge) => {
                        let new_bridge = Arc::new(bridge);
                        *guard = Some(new_bridge.clone());
                        return new_bridge;
                    }
                    Err(e) => {
                        tracing::error!("Failed to create extension bridge: {}", e);
                        // Don't store anything in the supervisor so we'll retry next time
                        *guard = None;
                        // Create a minimal bridge that will properly report it's not functional
                        // This bridge has no server task and no clients
                        return Arc::new(ExtensionBridge {
                            _server_task: tokio::spawn(async {}), // Immediately finished task
                            clients: Arc::new(Mutex::new(Vec::new())),
                            pending: Arc::new(Mutex::new(HashMap::new())),
                        });
                    }
                }
            }
        }

        // Return existing healthy bridge
        supervisor.read().await.as_ref().unwrap().clone()
    }

    async fn start(addr: &str) -> Result<ExtensionBridge, ExtensionBridgeError> {
        let clients: Clients = Arc::new(Mutex::new(Vec::new()));
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        // Extract port from address string
        let port: u16 = addr
            .split(':')
            .next_back()
            .and_then(|p| p.parse().ok())
            .unwrap_or(17373);

        // Try to bind the websocket listener; handle port conflicts properly
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                let kind = e.kind();
                if kind == std::io::ErrorKind::AddrInUse {
                    tracing::warn!(
                        %addr,
                        ?e,
                        "Port in use, checking for existing terminator process..."
                    );

                    // Try to find and kill existing terminator process
                    if let Some(pid) = Self::find_process_on_port(port).await {
                        tracing::info!(
                            "Found process {} on port {}, attempting to kill...",
                            pid,
                            port
                        );
                        if let Err(kill_err) = Self::kill_process(pid).await {
                            tracing::warn!("Failed to kill process {}: {}", pid, kill_err);
                        } else {
                            tracing::info!("Successfully killed process {}, waiting for port to be released...", pid);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }

                    // Try binding again after cleanup attempt
                    match TcpListener::bind(addr).await {
                        Ok(l) => l,
                        Err(e2) => {
                            tracing::error!(
                                %addr,
                                ?e2,
                                "Failed to bind after cleanup attempt"
                            );
                            return Err(ExtensionBridgeError::PortBindError { port, source: e2 });
                        }
                    }
                } else {
                    return Err(ExtensionBridgeError::IoError(e));
                }
            }
        };
        let clients_clone = clients.clone();
        let pending_clone = pending.clone();
        let addr_parsed: SocketAddr = listener.local_addr().expect("addr");
        tracing::info!("Terminator extension bridge listening on {}", addr_parsed);

        let server_task = tokio::spawn(async move {
            loop {
                let (stream, _peer) = match listener.accept().await {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!("ws accept error: {}", e);
                        continue;
                    }
                };
                let ws_clients = clients_clone.clone();
                let ws_pending = pending_clone.clone();
                tokio::spawn(async move {
                    let ws_stream = match accept_async(stream).await {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!("ws handshake error: {}", e);
                            return;
                        }
                    };
                    let (mut sink, mut stream) = ws_stream.split();
                    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

                    // writer task
                    let writer = tokio::spawn(async move {
                        while let Some(msg) = rx.recv().await {
                            if let Err(e) = sink.send(msg).await {
                                tracing::warn!("ws send error: {}", e);
                                break;
                            }
                        }
                    });

                    // register client
                    {
                        ws_clients.lock().await.push(Client {
                            sender: tx.clone(),
                            connected_at: std::time::Instant::now(),
                        });
                    }

                    // reader loop
                    while let Some(Ok(msg)) = stream.next().await {
                        if !msg.is_text() {
                            continue;
                        }
                        let txt = msg.into_text().unwrap_or_default();
                        match serde_json::from_str::<BridgeIncoming>(&txt) {
                            Ok(BridgeIncoming::EvalResult {
                                id,
                                ok,
                                result,
                                error,
                                tab_id,
                            }) => {
                                if ok {
                                    let size =
                                        result.as_ref().map(|r| r.to_string().len()).unwrap_or(0);
                                    tracing::info!(id = %id, ok = ok, result_size = size, tab_id = ?tab_id, "Bridge received EvalResult");
                                } else {
                                    let err_str =
                                        error.clone().unwrap_or_else(|| "unknown error".into());
                                    // Try direct JSON parse first
                                    if let Ok(val) =
                                        serde_json::from_str::<serde_json::Value>(&err_str)
                                    {
                                        let code =
                                            val.get("code").and_then(|v| v.as_str()).unwrap_or("");
                                        let msg = val
                                            .get("message")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");
                                        let details = val
                                            .get("details")
                                            .cloned()
                                            .unwrap_or(serde_json::Value::Null);
                                        tracing::error!(id = %id, code = code, message = msg, details = %details, raw = %err_str, "Bridge received EvalResult error (structured)");
                                    } else {
                                        // Not JSON, just log raw (truncate to avoid log spam)
                                        let head: String = err_str.chars().take(400).collect();
                                        tracing::error!(id = %id, error = %head, "Bridge received EvalResult error (raw)");
                                    }
                                }
                                if let Some(tx) = ws_pending.lock().await.remove(&id) {
                                    let _ = tx.send(if ok {
                                        Ok(result.unwrap_or(serde_json::Value::Null))
                                    } else {
                                        Err(error.unwrap_or_else(|| "unknown error".into()))
                                    });
                                }
                            }
                            Ok(BridgeIncoming::Typed(TypedIncoming::ConsoleEvent {
                                id,
                                level,
                                args,
                                stack_trace,
                                ts,
                            })) => {
                                let level_str = level.unwrap_or_else(|| "log".into());
                                let args_str =
                                    args.map(|v| v.to_string()).unwrap_or_else(|| "[]".into());
                                let ts_ms = ts.unwrap_or(0.0);
                                match level_str.as_str() {
                                    "error" => {
                                        tracing::error!(id = %id, ts = ts_ms, args = %args_str, stack = %stack_trace.as_ref().map(|v| v.to_string()).unwrap_or_default(), "Console error event")
                                    }
                                    "warning" | "warn" => {
                                        tracing::warn!(id = %id, ts = ts_ms, args = %args_str, "Console warn event")
                                    }
                                    "debug" => {
                                        tracing::debug!(id = %id, ts = ts_ms, args = %args_str, "Console debug event")
                                    }
                                    "info" => {
                                        tracing::info!(id = %id, ts = ts_ms, args = %args_str, "Console info event")
                                    }
                                    _ => {
                                        tracing::info!(id = %id, ts = ts_ms, args = %args_str, "Console log event")
                                    }
                                }
                            }
                            Ok(BridgeIncoming::Typed(TypedIncoming::ExceptionEvent {
                                id,
                                details,
                            })) => {
                                let details_val = details.unwrap_or(serde_json::Value::Null);
                                tracing::error!(id = %id, details = %details_val, "Runtime exception event");
                            }
                            Ok(BridgeIncoming::Typed(TypedIncoming::LogEvent { id, entry })) => {
                                let entry_val = entry.unwrap_or(serde_json::Value::Null);
                                tracing::info!(id = %id, entry = %entry_val, "Log.entryAdded event");
                            }
                            Ok(BridgeIncoming::Typed(TypedIncoming::NetworkRequest {
                                tab_id,
                                request_id,
                                request,
                                document_url,
                                timestamp,
                                wall_time: _,
                                initiator,
                                resource_type,
                                frame_id,
                                redirect_response: _,
                            })) => {
                                tracing::debug!(
                                    tab_id = tab_id,
                                    request_id = %request_id,
                                    method = %request.method,
                                    url = %request.url,
                                    "Network request captured"
                                );

                                let storage = get_network_storage();
                                let mut map = storage.write().await;
                                let entries = map.entry(tab_id).or_insert_with(Vec::new);

                                entries.push(NetworkRequestEntry {
                                    request_id: request_id.clone(),
                                    tab_id,
                                    request,
                                    response: None,
                                    document_url,
                                    request_timestamp: timestamp,
                                    response_timestamp: None,
                                    resource_type,
                                    frame_id,
                                    initiator,
                                });

                                // Limit storage to last 1000 requests per tab
                                if entries.len() > 1000 {
                                    entries.drain(0..entries.len() - 1000);
                                }
                            }
                            Ok(BridgeIncoming::Typed(TypedIncoming::NetworkResponse {
                                tab_id,
                                request_id,
                                response,
                                timestamp,
                                resource_type: _,
                                frame_id: _,
                            })) => {
                                tracing::debug!(
                                    tab_id = tab_id,
                                    request_id = %request_id,
                                    status = response.status,
                                    url = %response.url,
                                    "Network response captured"
                                );

                                let storage = get_network_storage();
                                let mut map = storage.write().await;

                                if let Some(entries) = map.get_mut(&tab_id) {
                                    // Find matching request and update with response
                                    if let Some(entry) = entries.iter_mut().find(|e| e.request_id == request_id) {
                                        entry.response = Some(response);
                                        entry.response_timestamp = Some(timestamp);
                                    }
                                }
                            }
                            Ok(BridgeIncoming::Typed(TypedIncoming::Hello { .. })) => {
                                tracing::info!("Extension connected");
                            }
                            Ok(BridgeIncoming::Typed(TypedIncoming::Pong)) => {}
                            Err(e) => tracing::warn!("Invalid incoming JSON: {}", e),
                        }
                    }

                    // Clean up disconnected client and pending requests
                    {
                        let mut clients = ws_clients.lock().await;
                        clients.retain(|c| !c.sender.is_closed());
                        let remaining = clients.len();

                        // Clear all pending requests when last client disconnects
                        // This prevents memory leaks and ensures clean state for next connection
                        if remaining == 0 {
                            let mut pending = ws_pending.lock().await;
                            let pending_count = pending.len();
                            if pending_count > 0 {
                                tracing::warn!(
                                    "Last client disconnected with {} pending requests - clearing all",
                                    pending_count
                                );
                                pending.clear();
                            } else {
                                tracing::info!("Last client disconnected, extension bridge idle");
                            }
                        } else {
                            tracing::info!(
                                "Client disconnected, {} client(s) remaining",
                                remaining
                            );
                        }
                    }

                    writer.abort();
                });
            }
        });

        Ok(ExtensionBridge {
            _server_task: server_task,
            clients,
            pending,
        })
    }

    #[cfg(target_os = "windows")]
    async fn find_process_on_port(port: u16) -> Option<u32> {
        use tokio::process::Command;

        // Use netstat to find the process
        let output = Command::new("cmd")
            .args([
                "/C",
                &format!("netstat -ano | findstr :{port} | findstr LISTENING"),
            ])
            .output()
            .await
            .ok()?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        // Parse output like: "  TCP    127.0.0.1:17373        0.0.0.0:0              LISTENING       6728"
        for line in output_str.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(pid_str) = parts.last() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    // Verify it's a terminator process
                    if Self::is_terminator_process(pid).await {
                        return Some(pid);
                    }
                }
            }
        }
        None
    }

    #[cfg(not(target_os = "windows"))]
    async fn find_process_on_port(port: u16) -> Option<u32> {
        use tokio::process::Command;

        // Use lsof on Unix-like systems
        let output = Command::new("lsof")
            .args(&["-i", &format!(":{}", port), "-t"])
            .output()
            .await
            .ok()?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        if let Ok(pid) = output_str.trim().parse::<u32>() {
            // Verify it's a terminator process
            if Self::is_terminator_process(pid).await {
                return Some(pid);
            }
        }
        None
    }

    #[cfg(target_os = "windows")]
    async fn is_terminator_process(pid: u32) -> bool {
        use tokio::process::Command;

        let output = Command::new("wmic")
            .args([
                "process",
                "where",
                &format!("ProcessID={pid}"),
                "get",
                "Name",
            ])
            .output()
            .await
            .ok();

        if let Some(output) = output {
            let output_str = String::from_utf8_lossy(&output.stdout);
            output_str.contains("terminator-mcp-agent")
        } else {
            false
        }
    }

    #[cfg(not(target_os = "windows"))]
    async fn is_terminator_process(pid: u32) -> bool {
        use tokio::process::Command;

        let output = Command::new("ps")
            .args(&["-p", &pid.to_string(), "-o", "comm="])
            .output()
            .await
            .ok();

        if let Some(output) = output {
            let output_str = String::from_utf8_lossy(&output.stdout);
            output_str.contains("terminator")
        } else {
            false
        }
    }

    #[cfg(target_os = "windows")]
    async fn kill_process(pid: u32) -> Result<(), ExtensionBridgeError> {
        use tokio::process::Command;

        let output = Command::new("wmic")
            .args(["process", "where", &format!("ProcessID={pid}"), "delete"])
            .output()
            .await
            .map_err(|e| ExtensionBridgeError::ProcessKillError(e.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(ExtensionBridgeError::ProcessKillError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    #[cfg(not(target_os = "windows"))]
    async fn kill_process(pid: u32) -> Result<(), ExtensionBridgeError> {
        use tokio::process::Command;

        let output = Command::new("kill")
            .args(&["-9", &pid.to_string()])
            .output()
            .await
            .map_err(|e| ExtensionBridgeError::ProcessKillError(e.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(ExtensionBridgeError::ProcessKillError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    pub async fn is_client_connected(&self) -> bool {
        !self.clients.lock().await.is_empty()
    }

    /// Get health status of the bridge for monitoring
    pub async fn health_status() -> serde_json::Value {
        let supervisor = BRIDGE_SUPERVISOR.get_or_init(|| Arc::new(RwLock::new(None)));

        let guard = supervisor.read().await;
        match &*guard {
            None => serde_json::json!({
                "connected": false,
                "status": "not_initialized",
                "clients": 0
            }),
            Some(bridge) => {
                let is_alive = !bridge._server_task.is_finished();
                let client_count = if is_alive {
                    bridge.clients.lock().await.len()
                } else {
                    0
                };

                serde_json::json!({
                    "connected": is_alive && client_count > 0,
                    "status": if !is_alive { "dead" } else if client_count > 0 { "healthy" } else { "waiting_for_clients" },
                    "clients": client_count,
                    "server_task_alive": is_alive
                })
            }
        }
    }

    pub async fn send_reset_command(&self) -> Result<(), AutomationError> {
        let req = ResetRequest {
            action: "reset".into(),
        };
        let payload = serde_json::to_string(&req)
            .map_err(|e| AutomationError::PlatformError(format!("serialize reset: {e}")))?;

        let mut clients = self.clients.lock().await;
        // Clean up dead clients first
        clients.retain(|c| !c.sender.is_closed());

        // Use the most recent client (last connected)
        if let Some(c) = clients.last() {
            if c.sender.send(Message::Text(payload)).is_ok() {
                tracing::info!("Sent reset command to extension");
                // Give the extension time to reset
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
        Ok(())
    }

    /// Get network requests for a specific tab with optional filtering
    pub async fn get_network_requests(
        tab_id: u32,
        url_pattern: Option<String>,
        method_filter: Option<String>,
        status_filter: Option<i32>,
    ) -> Vec<NetworkRequestEntry> {
        let storage = get_network_storage();
        let map = storage.read().await;

        let entries = match map.get(&tab_id) {
            Some(entries) => entries.clone(),
            None => return Vec::new(),
        };

        // Apply filters
        let filtered: Vec<NetworkRequestEntry> = entries
            .into_iter()
            .filter(|entry| {
                // URL pattern filter
                if let Some(ref pattern) = url_pattern {
                    if !entry.request.url.contains(pattern) {
                        return false;
                    }
                }

                // Method filter
                if let Some(ref method) = method_filter {
                    if entry.request.method.to_uppercase() != method.to_uppercase() {
                        return false;
                    }
                }

                // Status filter (only if response exists)
                if let Some(status) = status_filter {
                    if let Some(ref response) = entry.response {
                        if response.status != status {
                            return false;
                        }
                    } else {
                        // No response yet, doesn't match status filter
                        return false;
                    }
                }

                true
            })
            .collect();

        tracing::debug!(
            tab_id = tab_id,
            total_requests = map.get(&tab_id).map(|v| v.len()).unwrap_or(0),
            filtered_count = filtered.len(),
            url_pattern = ?url_pattern,
            method_filter = ?method_filter,
            status_filter = ?status_filter,
            "Retrieved network requests"
        );

        filtered
    }

    /// Clear all network requests for a specific tab
    pub async fn clear_network_requests(tab_id: u32) {
        let storage = get_network_storage();
        let mut map = storage.write().await;

        if let Some(entries) = map.remove(&tab_id) {
            tracing::info!(
                tab_id = tab_id,
                count = entries.len(),
                "Cleared network requests for tab"
            );
        } else {
            tracing::debug!(
                tab_id = tab_id,
                "No network requests to clear for tab"
            );
        }
    }

    /// Clear all network requests across all tabs
    pub async fn clear_all_network_requests() {
        let storage = get_network_storage();
        let mut map = storage.write().await;

        let total_tabs = map.len();
        let total_requests: usize = map.values().map(|v| v.len()).sum();

        map.clear();

        tracing::info!(
            total_tabs = total_tabs,
            total_requests = total_requests,
            "Cleared all network requests"
        );
    }

    pub async fn eval_in_active_tab(
        &self,
        code: &str,
        timeout: Duration,
    ) -> Result<Option<String>, AutomationError> {
        // Auto-retry logic: retry for up to 10 seconds if no clients connected
        const MAX_RETRY_DURATION: Duration = Duration::from_secs(10);
        const RETRY_INTERVAL: Duration = Duration::from_millis(500);
        let start_time = tokio::time::Instant::now();

        loop {
            let client_count = self.clients.lock().await.len();
            if client_count > 0 {
                // Clients connected, proceed with evaluation
                tracing::debug!("ExtensionBridge: {} client(s) connected", client_count);
                break;
            }

            // No clients connected yet
            if start_time.elapsed() >= MAX_RETRY_DURATION {
                tracing::warn!("ExtensionBridge: no clients connected after {} seconds; extension not available",
                    MAX_RETRY_DURATION.as_secs());
                return Ok(None);
            }

            // Log retry attempt
            tracing::info!(
                "ExtensionBridge: no clients connected, retrying in {}ms... (elapsed: {:.1}s)",
                RETRY_INTERVAL.as_millis(),
                start_time.elapsed().as_secs_f32()
            );

            // Wait before retrying
            tokio::time::sleep(RETRY_INTERVAL).await;
        }

        // Now we have clients, continue with original logic
        tracing::debug!(
            "ExtensionBridge: proceeding with evaluation after {:.1}s",
            start_time.elapsed().as_secs_f32()
        );
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<BridgeResult>();
        self.pending.lock().await.insert(id.clone(), tx);
        let req = EvalRequest {
            id: id.clone(),
            action: "eval".into(),
            code: code.to_string(),
            await_promise: true,
        };
        let payload = serde_json::to_string(&req)
            .map_err(|e| AutomationError::PlatformError(format!("bridge serialize: {e}")))?;

        // Clean up dead clients and send to most recent client
        let mut ok = false;
        {
            let mut clients = self.clients.lock().await;
            // Remove dead clients before attempting to send
            clients.retain(|c| !c.sender.is_closed());

            tracing::info!(clients = clients.len(), preview = %payload.chars().take(120).collect::<String>(), "Sending eval to extension");

            // Use the most recent client (last connected) instead of first
            if let Some(c) = clients.last() {
                ok = c.sender.send(Message::Text(payload)).is_ok();
                if ok {
                    tracing::debug!(
                        "Successfully sent eval to most recent client (connected at {:?})",
                        c.connected_at
                    );
                }
            }
        }
        if !ok {
            self.pending.lock().await.remove(&id);
            tracing::warn!("ExtensionBridge: failed to send eval - no active clients available");
            return Ok(None);
        }

        let res = tokio::time::timeout(timeout, rx).await;
        match res {
            Ok(Ok(Ok(val))) => Ok(Some(match val {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            })),
            Ok(Ok(Err(err))) => Ok(Some(format!("ERROR: {err}"))),
            Ok(Err(_canceled)) => {
                tracing::warn!("ExtensionBridge: oneshot canceled by receiver");
                Ok(None)
            }
            Err(_elapsed) => {
                // timeout
                let _ = self.pending.lock().await.remove(&id);
                tracing::warn!(
                    "ExtensionBridge: timed out waiting for EvalResult (id={})",
                    id
                );
                Ok(None)
            }
        }
    }
}

pub async fn try_eval_via_extension(
    code: &str,
    timeout: Duration,
) -> Result<Option<String>, AutomationError> {
    let bridge = ExtensionBridge::global().await;
    if bridge._server_task.is_finished() {
        tracing::error!(
            "Extension bridge server task is not running - attempting to recreate bridge"
        );

        // Clear the broken bridge from supervisor
        let supervisor = BRIDGE_SUPERVISOR.get_or_init(|| Arc::new(RwLock::new(None)));
        {
            let mut guard = supervisor.write().await;
            *guard = None;
        }

        // Try to create a new bridge
        let new_bridge = ExtensionBridge::global().await;
        if new_bridge._server_task.is_finished() {
            tracing::error!(
                "Failed to recreate extension bridge - WebSocket server still unavailable"
            );
            return Ok(None);
        }

        tracing::info!("Successfully recreated extension bridge");
        return new_bridge.eval_in_active_tab(code, timeout).await;
    }
    bridge.eval_in_active_tab(code, timeout).await
}
