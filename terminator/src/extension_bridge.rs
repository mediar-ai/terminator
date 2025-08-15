use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot, Mutex},
    task::JoinHandle,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

use crate::AutomationError;

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum BridgeIncoming {
    EvalResult {
        id: String,
        ok: bool,
        result: Option<serde_json::Value>,
        error: Option<String>,
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
}

struct Client {
    sender: mpsc::UnboundedSender<Message>,
}

pub struct ExtensionBridge {
    _server_task: JoinHandle<()>,
    clients: Clients,
    pending: Pending,
}

static GLOBAL: OnceCell<Arc<ExtensionBridge>> = OnceCell::new();

impl ExtensionBridge {
    pub async fn global() -> Arc<ExtensionBridge> {
        if let Some(h) = GLOBAL.get() {
            return h.clone();
        }
        let bridge = ExtensionBridge::start(DEFAULT_WS_ADDR).await;
        let arc = Arc::new(bridge);
        let _ = GLOBAL.set(arc.clone());
        arc
    }

    async fn start(addr: &str) -> ExtensionBridge {
        let clients: Clients = Arc::new(Mutex::new(Vec::new()));
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        // Try to bind the websocket listener; avoid panicking if the port is already in use.
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                let kind = e.kind();
                if kind == std::io::ErrorKind::AddrInUse {
                    tracing::warn!(
                        %addr,
                        ?e,
                        "Port in use, waiting 2 seconds and retrying once..."
                    );
                    // Wait a bit for the port to be released
                    tokio::time::sleep(Duration::from_secs(2)).await;

                    // Try one more time
                    match TcpListener::bind(addr).await {
                        Ok(l) => l,
                        Err(e2) => {
                            tracing::error!(
                                %addr,
                                ?e2,
                                "Failed to bind after retry. Extension bridge will be non-functional."
                            );
                            return ExtensionBridge {
                                _server_task: tokio::spawn(async move {}),
                                clients,
                                pending,
                            };
                        }
                    }
                } else {
                    tracing::warn!(%addr, ?e, "failed to bind ws");
                    return ExtensionBridge {
                        _server_task: tokio::spawn(async move {}),
                        clients,
                        pending,
                    };
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
                        ws_clients.lock().await.push(Client { sender: tx.clone() });
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
                            }) => {
                                if ok {
                                    let size =
                                        result.as_ref().map(|r| r.to_string().len()).unwrap_or(0);
                                    tracing::info!(id = %id, ok = ok, result_size = size, "Bridge received EvalResult");
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
                            Ok(BridgeIncoming::Typed(TypedIncoming::Hello { .. })) => {
                                tracing::info!("Extension connected");
                            }
                            Ok(BridgeIncoming::Typed(TypedIncoming::Pong)) => {}
                            Err(e) => tracing::warn!("Invalid incoming JSON: {}", e),
                        }
                    }

                    writer.abort();
                });
            }
        });

        ExtensionBridge {
            _server_task: server_task,
            clients,
            pending,
        }
    }

    pub async fn is_client_connected(&self) -> bool {
        !self.clients.lock().await.is_empty()
    }

    pub async fn eval_in_active_tab(
        &self,
        code: &str,
        timeout: Duration,
    ) -> Result<Option<String>, AutomationError> {
        if self.clients.lock().await.is_empty() {
            tracing::info!("ExtensionBridge: no clients connected; skipping extension path");
            return Ok(None);
        }
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

        // send over first client
        let mut ok = false;
        {
            let clients = self.clients.lock().await;
            tracing::info!(clients = clients.len(), preview = %payload.chars().take(120).collect::<String>(), "Sending eval to extension");
            if let Some(c) = clients.first() {
                ok = c.sender.send(Message::Text(payload)).is_ok();
            }
        }
        if !ok {
            self.pending.lock().await.remove(&id);
            tracing::warn!("ExtensionBridge: failed to send eval to first client");
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
    bridge.eval_in_active_tab(code, timeout).await
}
