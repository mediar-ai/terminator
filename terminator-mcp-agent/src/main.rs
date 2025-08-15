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
use terminator_mcp_agent::server;
use terminator_mcp_agent::utils::init_logging;
use tower_http::cors::CorsLayer;
use tracing::{error, info};

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

    init_logging()?;

    tracing::info!("Initializing Terminator MCP server...");
    tracing::info!("Transport mode: {:?}", args.transport);
    if args.cors {
        tracing::info!("CORS enabled for web transports");
    }

    match args.transport {
        TransportMode::Stdio => {
            tracing::info!("Starting stdio transport...");
            let desktop = server::DesktopWrapper::new()?;
            let service = desktop.serve(stdio()).await.inspect_err(|e| {
                tracing::error!("Serving error: {:?}", e);
            })?;

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

            // Busy-aware concurrency state
            #[derive(Clone)]
            struct AppState {
                active_requests: Arc<AtomicUsize>,
                last_activity: Arc<Mutex<String>>, // ISO-8601
                max_concurrent: usize,
            }

            let max_concurrent = std::env::var("MCP_MAX_CONCURRENT")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1);

            let app_state = AppState {
                active_requests: Arc::new(AtomicUsize::new(0)),
                last_activity: Arc::new(Mutex::new(chrono::Utc::now().to_rfc3339())),
                max_concurrent,
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

                    state.active_requests.fetch_add(1, Ordering::SeqCst);
                    if let Ok(mut ts) = state.last_activity.lock() {
                        *ts = chrono::Utc::now().to_rfc3339();
                    }

                    let response = next.run(req).await;

                    state.active_requests.fetch_sub(1, Ordering::SeqCst);
                    if let Ok(mut ts) = state.last_activity.lock() {
                        *ts = chrono::Utc::now().to_rfc3339();
                    }

                    return response;
                }

                next.run(req).await
            }

            // Build a sub-router for /mcp that uses the service with concurrency gate middleware
            let mcp_router = Router::new().fallback_service(service).layer(
                axum::middleware::from_fn_with_state(app_state.clone(), mcp_gate),
            );

            let mut router: Router = Router::new()
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
            info!("Connect your MCP client to: http://{addr}/mcp");
            info!("Status endpoint available at: http://{addr}/status");
            info!("Health check available at: http://{addr}/health");
            info!("Press Ctrl+C to stop");

            axum::serve(tcp_listener, router)
                .with_graceful_shutdown(async {
                    tokio::signal::ctrl_c().await.ok();
                })
                .await?;

            tracing::info!("Shutting down HTTP server");
        }
    }

    Ok(())
}

async fn health_check() -> impl axum::response::IntoResponse {
    (
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({"status": "ok"})),
    )
}
