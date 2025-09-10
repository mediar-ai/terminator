use anyhow::Result;
use axum::middleware::Next;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, Method, Request, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use clap::{Parser, ValueEnum};
use rmcp::{
    transport::sse_server::SseServer,
    transport::stdio,
    transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpService,
    },
    ServiceExt,
};
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use terminator_mcp_agent::cancellation::RequestManager;
use terminator_mcp_agent::event_bus;
use terminator_mcp_agent::server;
use terminator_mcp_agent::utils::init_logging;
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Terminator MCP Server - Desktop automation via Model Context Protocol"
)]
struct Args {
    /// Transport mode to use
    #[arg(short, long, value_enum, default_value = "stdio")]
    transport: TransportMode,

    /// Port to listen on (only used for SSE and HTTP transports)
    #[arg(short, long, default_value = "3000")]
    port: u16,

    /// Host to bind to (only used for SSE and HTTP transports)
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Enable CORS for HTTP and SSE transports
    #[arg(long)]
    cors: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum TransportMode {
    /// Standard I/O transport (default)
    Stdio,
    /// Server-Sent Events transport for web integrations
    Sse,
    /// Streamable HTTP transport for HTTP-based clients
    Http,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Install panic hook to prevent stdout corruption (used by other MCP servers)
    std::panic::set_hook(Box::new(|panic_info| {
        // CRITICAL: Never write to stdout during panic - it corrupts the JSON-RPC stream
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            eprintln!("MCP Server Panic: {s}");
        } else {
            eprintln!("MCP Server Panic occurred");
        }
        if let Some(location) = panic_info.location() {
            eprintln!("Panic location: {}:{}", location.file(), location.line());
        }
    }));

    // Fix Windows encoding issues (IBM437 -> UTF-8)
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "chcp", "65001"])
            .output();
        eprintln!("Set Windows console to UTF-8 mode");
    }

    init_logging()?;

    tracing::info!("Initializing Terminator MCP server...");
    tracing::info!("Transport mode: {:?}", args.transport);
    if args.cors {
        tracing::info!("CORS enabled for web transports");
    }

    match args.transport {
        TransportMode::Stdio => {
            tracing::info!("Starting stdio transport...");

            // Initialize with error recovery (pattern used by other MCP servers)
            let desktop = match server::DesktopWrapper::new() {
                Ok(d) => d,
                Err(e) => {
                    tracing::error!("Failed to initialize desktop wrapper: {}", e);
                    eprintln!("Fatal: Failed to initialize MCP server: {e}");
                    // Exit with code 1 to signal Cursor to potentially restart
                    std::process::exit(1);
                }
            };

            // Serve with better error handling
            let service = desktop.serve(stdio()).await.inspect_err(|e| {
                tracing::error!("Serving error: {:?}", e);
                eprintln!("Fatal: stdio communication error: {e}");
                // Many successful MCP servers exit cleanly on stdio errors
                // This signals to Cursor that the server needs to be restarted
                std::process::exit(1);
            })?;

            // Log periodic stats to help debug disconnections
            tokio::spawn(async {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    eprintln!("MCP server still running (stdio mode)");
                }
            });

            service.waiting().await?;
        }
        TransportMode::Sse => {
            let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
            tracing::info!("Starting SSE server on http://{}", addr);

            if args.cors {
                error!("SSE transport does not support CORS");
                info!("Use HTTP transport for CORS support:");
                info!(
                    "   terminator-mcp-agent -t http --cors --port {}",
                    args.port
                );
                info!("   Then connect to: http://{}:{}/mcp", args.host, args.port);
                return Ok(());
            }

            let desktop = server::DesktopWrapper::new()?;
            let ct = SseServer::serve(addr)
                .await?
                .with_service(move || desktop.clone());

            info!("SSE server running on http://{addr}");
            info!("Connect your MCP client to:");
            info!("  SSE endpoint: http://{addr}/sse");
            info!("  Message endpoint: http://{addr}/message");
            info!("Press Ctrl+C to stop");

            tokio::signal::ctrl_c().await?;
            ct.cancel();
            tracing::info!("Shutting down SSE server");
        }
        TransportMode::Http => {
            let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
            tracing::info!("Starting streamable HTTP server on http://{}", addr);

            // Lazy-initialize DesktopWrapper on first /mcp use so that /health can succeed on CI
            let service = StreamableHttpService::new(
                move || {
                    server::DesktopWrapper::new().map_err(|e| std::io::Error::other(e.to_string()))
                },
                LocalSessionManager::default().into(),
                Default::default(),
            );

            // Busy-aware concurrency state with request tracking
            #[derive(Clone)]
            struct AppState {
                active_requests: Arc<AtomicUsize>,
                last_activity: Arc<Mutex<String>>, // ISO-8601
                max_concurrent: usize,
                request_manager: RequestManager,
            }

            let max_concurrent = std::env::var("MCP_MAX_CONCURRENT")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1);

            let app_state = AppState {
                active_requests: Arc::new(AtomicUsize::new(0)),
                last_activity: Arc::new(Mutex::new(chrono::Utc::now().to_rfc3339())),
                max_concurrent,
                request_manager: RequestManager::new(),
            };

            async fn status_handler(State(state): State<AppState>) -> impl IntoResponse {
                let active = state.active_requests.load(Ordering::SeqCst);
                let busy = active >= state.max_concurrent;
                let last_activity = state
                    .last_activity
                    .lock()
                    .map(|s| s.clone())
                    .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());
                let code = if busy {
                    StatusCode::SERVICE_UNAVAILABLE
                } else {
                    StatusCode::OK
                };
                let body = serde_json::json!({
                    "busy": busy,
                    "activeRequests": active,
                    "maxConcurrent": state.max_concurrent,
                    "lastActivity": last_activity,
                });
                (code, Json(body))
            }

            async fn mcp_gate(
                State(state): State<AppState>,
                req: Request<Body>,
                next: Next,
            ) -> impl IntoResponse {
                let request_start = std::time::Instant::now();
                if req.method() == Method::POST {
                    let active = state.active_requests.load(Ordering::SeqCst);
                    if active >= state.max_concurrent {
                        let last_activity = state
                            .last_activity
                            .lock()
                            .map(|s| s.clone())
                            .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());
                        let body = serde_json::json!({
                            "busy": true,
                            "activeRequests": active,
                            "maxConcurrent": state.max_concurrent,
                            "lastActivity": last_activity,
                        });
                        return (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response();
                    }

                    // Extract request ID from headers or generate one
                    let headers = req.headers();
                    let request_id = headers
                        .get("x-request-id")
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| Uuid::new_v4().to_string());

                    // Extract timeout from headers
                    let timeout_ms = headers
                        .get("x-request-timeout-ms")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .or_else(|| {
                            std::env::var("MCP_DEFAULT_TIMEOUT_MS")
                                .ok()
                                .and_then(|s| s.parse::<u64>().ok())
                        });

                    debug!(
                        "Processing request {} with timeout {:?}ms",
                        request_id, timeout_ms
                    );

                    // Register the request with cancellation support
                    let context = state
                        .request_manager
                        .register(request_id.clone(), timeout_ms)
                        .await;

                    state.active_requests.fetch_add(1, Ordering::SeqCst);
                    if let Ok(mut ts) = state.last_activity.lock() {
                        *ts = chrono::Utc::now().to_rfc3339();
                    }

                    // Clone for cleanup
                    let request_id_cleanup = request_id.clone();
                    let manager_cleanup = state.request_manager.clone();
                    let state_cleanup = state.clone();

                    // Execute the request with cancellation support
                    let response = tokio::select! {
                        res = next.run(req) => res,
                        _ = context.cancellation_token.cancelled() => {
                            debug!("Request {} was cancelled", request_id);
                            let body = serde_json::json!({
                                "error": {
                                    "code": -32001,
                                    "message": format!("Request {} was cancelled", request_id)
                                }
                            });
                            (StatusCode::REQUEST_TIMEOUT, Json(body)).into_response()
                        }
                    };

                    // Cleanup
                    manager_cleanup.unregister(&request_id_cleanup).await;
                    state_cleanup.active_requests.fetch_sub(1, Ordering::SeqCst);
                    if let Ok(mut ts) = state_cleanup.last_activity.lock() {
                        *ts = chrono::Utc::now().to_rfc3339();
                    }

                    // Record request metrics
                    let duration = request_start.elapsed();
                    let status_code = response.status().as_u16();
                    terminator_mcp_agent::telemetry::record_request_metrics(
                        "/mcp",
                        "POST",
                        status_code,
                        duration,
                    );

                    return response;
                }

                let response = next.run(req).await;
                let duration = request_start.elapsed();
                let status_code = response.status().as_u16();
                terminator_mcp_agent::telemetry::record_request_metrics(
                    "(other)",
                    req.method().as_str(),
                    status_code,
                    duration,
                );
                response
            }

            // Build a sub-router for /mcp that uses the service with concurrency gate middleware
            let mcp_router = Router::new().fallback_service(service).layer(
                axum::middleware::from_fn_with_state(app_state.clone(), mcp_gate),
            );

            let mut router: Router = Router::new()
                .route("/health", get(health_check))
                .route("/status", get(status_handler))
                .route("/events", get(events_sse))
                .nest("/mcp", mcp_router)
                .with_state(app_state.clone());

            // Optional Prometheus metrics endpoint under telemetry feature
            #[cfg(feature = "telemetry")]
            {
                use axum::response::Response;
                async fn metrics_handler() -> Response {
                    if let Some(body) = terminator_mcp_agent::telemetry::prometheus_scrape() {
                        let mut resp = axum::http::Response::builder()
                            .status(StatusCode::OK)
                            .header(
                                axum::http::header::CONTENT_TYPE,
                                "text/plain; version=0.0.4",
                            )
                            .body(Body::from(body))
                            .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR).into_response().into_body());
                        return resp;
                    }
                    (StatusCode::NOT_FOUND).into_response()
                }

                router = router.route("/metrics", get(metrics_handler));
            }

            if args.cors {
                router = router.layer(CorsLayer::permissive());
            }

            let tcp_listener = tokio::net::TcpListener::bind(addr).await?;

            info!("Streamable HTTP server running on http://{addr}");
            if args.cors {
                info!("CORS enabled - accessible from web browsers");
            }
            info!("Connect your MCP client to: http://{addr}/mcp");
            info!("Status endpoint available at: http://{addr}/status");
            info!("Health check available at: http://{addr}/health");
            info!("Press Ctrl+C to stop");

            axum::serve(tcp_listener, router)
                .with_graceful_shutdown(async move {
                    tokio::signal::ctrl_c().await.ok();
                    info!("Received shutdown signal, cancelling active requests...");
                    app_state.request_manager.cancel_all().await;
                })
                .await?;

            tracing::info!("Shutting down HTTP server");
        }
    }

    Ok(())
}

async fn health_check() -> impl axum::response::IntoResponse {
    // Get bridge health status
    let bridge_health = terminator::extension_bridge::ExtensionBridge::health_status().await;

    (
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({
            "status": "ok",
            "extension_bridge": bridge_health,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
    )
}

async fn events_sse() -> impl IntoResponse {
    use axum::response::sse::{Event, KeepAlive, Sse};
    use futures_util::stream::Stream;
    use futures_util::StreamExt;
    use std::convert::Infallible;
    use tokio_stream::wrappers::BroadcastStream;

    // Subscribe to the global event bus
    let rx = event_bus::subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(|msg| async move { msg.ok() })
        .map(|json| {
            let mut ev = Event::default()
                .event("sequence")
                .data(serde_json::to_string(&json).unwrap_or_else(|_| "{}".to_string()));
            if let Some(id) = json.get("request_id").and_then(|v| v.as_str()) {
                ev = ev.id(id.to_string());
            }
            Ok::<Event, Infallible>(ev)
        });

    let sse = Sse::new(stream).keep_alive(KeepAlive::new());

    // Add CORS headers explicitly for SSE if needed
    let mut resp = sse.into_response();
    let headers = resp.headers_mut();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );
    headers.insert(
        axum::http::header::CONNECTION,
        HeaderValue::from_static("keep-alive"),
    );
    resp
}
