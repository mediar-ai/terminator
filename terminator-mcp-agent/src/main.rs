use anyhow::Result;
use axum::middleware::Next;
use axum::{
    body::Body,
    extract::State,
    http::{Method, Request, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
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
    time::SystemTime,
};
use sysinfo::{ProcessesToUpdate, System};
use terminator_mcp_agent::cancellation::RequestManager;
use terminator_mcp_agent::server;
use terminator_mcp_agent::utils::init_logging;
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info};
use uuid::Uuid;

#[cfg(feature = "rdp")]
mod rdp_server;

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

    /// Authentication token for HTTP/SSE transports (can also use MCP_AUTH_TOKEN env var)
    /// When set, clients must provide matching Bearer token in Authorization header
    #[arg(long, env = "MCP_AUTH_TOKEN")]
    auth_token: Option<String>,

    /// Enable RDP server for remote viewing/control
    #[cfg(feature = "rdp")]
    #[arg(long)]
    rdp: bool,

    /// RDP server bind address
    #[cfg(feature = "rdp")]
    #[arg(long, default_value = "127.0.0.1:3389")]
    rdp_bind: String,
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

fn kill_previous_mcp_instances() {
    let current_pid = std::process::id();
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let mut killed_count = 0;
    for (pid, process) in system.processes() {
        let process_name = process.name().to_string_lossy().to_lowercase();

        // Kill other terminator-mcp-agent processes
        if process_name.contains("terminator-mcp-agent") && pid.as_u32() != current_pid {
            eprintln!(
                "Found existing MCP agent with PID {}, killing it...",
                pid.as_u32()
            );
            if process.kill() {
                killed_count += 1;
                eprintln!("Successfully killed MCP agent with PID {}", pid.as_u32());
            } else {
                eprintln!(
                    "Failed to kill MCP agent with PID {} (may require elevated permissions)",
                    pid.as_u32()
                );
            }
        }

        // Also kill any bridge service processes
        if process_name.contains("terminator-bridge-service") {
            eprintln!(
                "Found bridge service with PID {}, killing it...",
                pid.as_u32()
            );
            if process.kill() {
                killed_count += 1;
                eprintln!(
                    "Successfully killed bridge service with PID {}",
                    pid.as_u32()
                );
            } else {
                eprintln!("Failed to kill bridge service with PID {}", pid.as_u32());
            }
        }
    }

    if killed_count > 0 {
        eprintln!(
            "Killed {killed_count} previous instance(s), waiting for ports to be released..."
        );
        // Increase wait time to 2 seconds for Windows to properly release ports
        std::thread::sleep(std::time::Duration::from_millis(2000));

        // Verify port 17373 is available
        let mut retries = 0;
        while retries < 5 {
            match std::net::TcpListener::bind("127.0.0.1:17373") {
                Ok(listener) => {
                    drop(listener); // Immediately release the port
                    eprintln!("Port 17373 is now available");
                    break;
                }
                Err(_) => {
                    retries += 1;
                    if retries < 5 {
                        eprintln!("Port 17373 still unavailable, waiting... (attempt {retries}/5)");
                        std::thread::sleep(std::time::Duration::from_millis(1000));
                    } else {
                        eprintln!("WARNING: Port 17373 is still not available after 5 attempts");
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Kill any previous MCP instances before starting
    kill_previous_mcp_instances();

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

    let log_capture = init_logging()?;

    // Initialize OpenTelemetry if telemetry feature is enabled (after logging is set up)
    terminator_mcp_agent::telemetry::init_telemetry()?;

    // Add binary identification logging
    tracing::info!("========================================");
    tracing::info!("Terminator MCP Server v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!(
        "Build profile: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );

    // Get executable path and timestamp
    if let Ok(exe_path) = std::env::current_exe() {
        tracing::info!("Binary path: {}", exe_path.display());

        // Get binary modification time
        if let Ok(metadata) = std::fs::metadata(&exe_path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                    let datetime = DateTime::<Utc>::from_timestamp(duration.as_secs() as i64, 0)
                        .unwrap_or_default();
                    tracing::info!("Binary built: {} UTC", datetime.format("%Y-%m-%d %H:%M:%S"));
                }
            }

            // File size can help distinguish builds
            tracing::info!("Binary size: {} bytes", metadata.len());
        }
    }

    // Add git and build info if available
    if let Some(git_hash) = option_env!("GIT_HASH") {
        tracing::info!("Git commit: {}", git_hash);
    }
    if let Some(git_branch) = option_env!("GIT_BRANCH") {
        tracing::info!("Git branch: {}", git_branch);
    }
    if let Some(build_time) = option_env!("BUILD_TIMESTAMP") {
        tracing::info!("Build timestamp: {}", build_time);
    }

    tracing::info!("========================================");

    // Check for Visual C++ Redistributables on Windows (one-time at startup)
    if cfg!(windows) {
        terminator_mcp_agent::vcredist_check::check_vcredist_installed();
    }

    tracing::info!("Initializing Terminator MCP server...");
    tracing::info!("Transport mode: {:?}", args.transport);
    if args.cors {
        tracing::info!("CORS enabled for web transports");
    }

    match args.transport {
        TransportMode::Stdio => {
            tracing::info!("Starting stdio transport...");

            // Initialize with error recovery (pattern used by other MCP servers)
            let desktop = match server::DesktopWrapper::new_with_log_capture(log_capture.clone()) {
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

            if args.auth_token.is_some() {
                tracing::warn!("⚠️  SSE transport does not support authentication yet");
                tracing::warn!("⚠️  Use HTTP transport for Bearer token authentication");
                tracing::warn!("   Command: terminator-mcp-agent -t http --auth-token YOUR_TOKEN --port {}", args.port);
            }

            let desktop = server::DesktopWrapper::new_with_log_capture(log_capture.clone())?;

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

            // If RDP is enabled, we need to eagerly initialize DesktopWrapper to start RDP server
            #[cfg(feature = "rdp")]
            let desktop_for_rdp = if args.rdp {
                Some(Arc::new(server::DesktopWrapper::new_with_log_capture(log_capture.clone())?))
            } else {
                None
            };

            #[cfg(not(feature = "rdp"))]
            let desktop_for_rdp: Option<Arc<server::DesktopWrapper>> = None;

            // Start RDP server if enabled
            #[cfg(feature = "rdp")]
            if args.rdp {
                let rdp_config = rdp_server::RdpServerConfig {
                    bind_address: args.rdp_bind.parse().expect("Invalid RDP bind address"),
                    ..Default::default()
                };

                if let Some(desktop) = &desktop_for_rdp {
                    let rdp_server = rdp_server::RdpServerRunner::new(rdp_config, desktop.desktop.clone());
                    tokio::spawn(async move {
                        if let Err(e) = rdp_server.run().await {
                            error!("RDP server error: {:#}", e);
                        }
                    });
                    tracing::info!("✅ RDP server enabled on {}", args.rdp_bind);
                }
            }

            // Create service - lazy init if no RDP, eager init if RDP enabled
            let service = if let Some(desktop_arc) = desktop_for_rdp {
                // Eager initialization for RDP - use cloned wrapper
                StreamableHttpService::new(
                    {
                        let desktop_clone = desktop_arc.clone();
                        move || Ok((*desktop_clone).clone())
                    },
                    LocalSessionManager::default().into(),
                    Default::default(),
                )
            } else {
                // Lazy initialization (original behavior) - for health check on CI
                StreamableHttpService::new(
                    {
                        let log_capture = log_capture.clone();
                        move || {
                            server::DesktopWrapper::new_with_log_capture(log_capture.clone())
                                .map_err(|e| std::io::Error::other(e.to_string()))
                        }
                    },
                    LocalSessionManager::default().into(),
                    Default::default(),
                )
            };

            // Busy-aware concurrency state with request tracking
            #[derive(Clone)]
            struct AppState {
                active_requests: Arc<AtomicUsize>,
                last_activity: Arc<Mutex<String>>, // ISO-8601
                max_concurrent: usize,
                request_manager: RequestManager,
                auth_token: Option<String>,
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
                auth_token: args.auth_token.clone(),
            };

            // Log authentication status
            if app_state.auth_token.is_some() {
                tracing::info!("🔒 Authentication enabled - Bearer token required");
            } else {
                tracing::warn!("⚠️  Authentication disabled - server is publicly accessible");
            }

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

            // Authentication middleware - validates Bearer token if auth is enabled
            async fn auth_middleware(
                State(state): State<AppState>,
                req: Request<Body>,
                next: Next,
            ) -> impl IntoResponse {
                // Skip auth if no token is configured
                if state.auth_token.is_none() {
                    return next.run(req).await;
                }

                // Extract Authorization header
                let auth_header = req
                    .headers()
                    .get(axum::http::header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok());

                // Validate token
                if let Some(auth_value) = auth_header {
                    if let Some(token) = auth_value.strip_prefix("Bearer ") {
                        if state.auth_token.as_deref() == Some(token) {
                            // Token valid, proceed
                            return next.run(req).await;
                        }
                    }
                }

                // Authentication failed
                debug!("Authentication failed - invalid or missing Bearer token");
                let body = serde_json::json!({
                    "error": {
                        "code": -32001,
                        "message": "Unauthorized - invalid or missing Bearer token"
                    }
                });
                (StatusCode::UNAUTHORIZED, Json(body)).into_response()
            }

            async fn mcp_gate(
                State(state): State<AppState>,
                req: Request<Body>,
                next: Next,
            ) -> impl IntoResponse {
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

                    return response;
                }

                next.run(req).await
            }

            // Build a sub-router for /mcp that uses the service with auth and concurrency gate middleware
            let mcp_router = Router::new()
                .fallback_service(service)
                .layer(axum::middleware::from_fn_with_state(app_state.clone(), mcp_gate))
                .layer(axum::middleware::from_fn_with_state(app_state.clone(), auth_middleware));

            let mut router: Router = Router::new()
                .route("/", get(root_handler))
                .route("/health", get(health_check))
                .route("/status", get(status_handler))
                .nest("/mcp", mcp_router)
                .with_state(app_state.clone());

            if args.cors {
                router = router.layer(CorsLayer::permissive());
            }

            let tcp_listener = tokio::net::TcpListener::bind(addr).await?;

            info!("Streamable HTTP server running on http://{addr}");
            if args.cors {
                info!("CORS enabled - accessible from web browsers");
            }
            info!("Available endpoints:");
            info!("  Root (endpoint list): http://{addr}/");
            info!("  MCP client endpoint: http://{addr}/mcp");
            info!("  Status endpoint: http://{addr}/status");
            info!("  Health check: http://{addr}/health");
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

    // Shutdown telemetry before exiting
    terminator_mcp_agent::telemetry::shutdown_telemetry();

    Ok(())
}

async fn root_handler() -> impl axum::response::IntoResponse {
    (
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({
            "name": "Terminator MCP Server",
            "description": "Desktop automation via Model Context Protocol",
            "version": env!("CARGO_PKG_VERSION"),
            "endpoints": {
                "/": "This endpoint - lists available endpoints",
                "/mcp": "MCP protocol endpoint - connect your MCP client here",
                "/health": "Health check endpoint - returns server status",
                "/status": "Status endpoint - shows active requests and concurrency info"
            },
            "usage": {
                "mcp_client": "Connect your MCP client to: /mcp",
                "example": "http://127.0.0.1:3000/mcp"
            },
            "documentation": "https://github.com/mediar-ai/terminator",
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
    )
}

async fn health_check() -> impl axum::response::IntoResponse {
    use terminator::health::{check_automation_health, HealthStatus};

    // Get bridge health status
    let bridge_health = terminator::extension_bridge::ExtensionBridge::health_status().await;

    // Check platform-specific automation API health
    let automation_health = check_automation_health().await;

    // Build response body
    let response_body = serde_json::json!({
        "status": match automation_health.status {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
        },
        "extension_bridge": bridge_health,
        "automation": {
            "api_available": automation_health.api_available,
            "desktop_accessible": automation_health.desktop_accessible,
            "can_enumerate_elements": automation_health.can_enumerate_elements,
            "check_duration_ms": automation_health.check_duration_ms,
            "error_message": automation_health.error_message,
            "diagnostics": automation_health.diagnostics,
        },
        "platform": automation_health.platform,
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    // Return appropriate HTTP status based on health
    let http_status = match automation_health.status.to_http_status() {
        200 => axum::http::StatusCode::OK,
        206 => axum::http::StatusCode::PARTIAL_CONTENT,
        503 => axum::http::StatusCode::SERVICE_UNAVAILABLE,
        _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
    };

    (http_status, axum::Json(response_body))
}
