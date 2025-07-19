//! Example demonstrating Prometheus metrics for the MCP server
//! 
//! This example shows how to start the MCP server with metrics enabled
//! and provides information about accessing the metrics endpoint.

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Terminator MCP Server Metrics Demo");
    println!("=====================================");
    println!();
    
    println!("To enable Prometheus metrics for the MCP server:");
    println!();
    println!("1. Build with metrics feature:");
    println!("   cargo build --features metrics");
    println!();
    println!("2. Run with HTTP transport and metrics enabled:");
    println!("   cargo run --features metrics -- --transport http --enable-metrics");
    println!();
    println!("3. Access endpoints:");
    println!("   • MCP Server: http://127.0.0.1:3000/mcp");
    println!("   • Health Check: http://127.0.0.1:3000/health");
    println!("   • Metrics: http://127.0.0.1:3000/metrics");
    println!();
    
    println!("📊 Available Metrics:");
    println!("   • mcp_tool_calls_total - Total number of tool calls by tool name and status");
    println!("   • mcp_tool_execution_duration_seconds - Tool execution time histogram");
    println!("   • mcp_http_requests_total - HTTP request counts by method, path, and status");
    println!("   • mcp_http_request_duration_seconds - HTTP request duration histogram");
    println!("   • mcp_errors_total - Error counts by type and component");
    println!("   • mcp_server_starts_total - Server restart counter");
    println!("   • mcp_active_connections - Current number of active connections");
    println!("   • mcp_connection_duration_seconds - Connection duration histogram");
    println!();
    
    println!("🔧 Example Prometheus Config:");
    println!("   scrape_configs:");
    println!("     - job_name: 'terminator-mcp'");
    println!("       static_configs:");
    println!("         - targets: ['localhost:3000']");
    println!("       metrics_path: '/metrics'");
    println!();
    
    println!("📈 Example Grafana Queries:");
    println!("   • Tool usage rate: rate(mcp_tool_calls_total[5m])");
    println!("   • Error rate: rate(mcp_errors_total[5m])");
    println!("   • Average tool execution time: rate(mcp_tool_execution_duration_seconds_sum[5m]) / rate(mcp_tool_execution_duration_seconds_count[5m])");
    println!("   • P95 HTTP response time: histogram_quantile(0.95, rate(mcp_http_request_duration_seconds_bucket[5m]))");
    println!();
    
    #[cfg(not(feature = "metrics"))]
    {
        println!("⚠️  Metrics feature is NOT enabled!");
        println!("   Rebuild with: cargo build --features metrics");
    }
    
    #[cfg(feature = "metrics")]
    {
        println!("✅ Metrics feature is enabled!");
        println!("   You can now start the server with --enable-metrics flag");
    }
    
    Ok(())
}