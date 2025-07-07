use tracing::debug;
use serde_json::Value;
use tokio::process::Command;
use anyhow::{Result, anyhow};
use rmcp::model::CallToolResult;
use crate::utils::get_agent_binary_path;
use rmcp::transport::TokioChildProcess;
use rmcp::{model::CallToolRequestParam, ServiceExt};
use rmcp::transport::ConfigureCommandExt;

pub async fn get_mcp_tool_result(tool_name: String, args: Option<serde_json::Map<String, Value>>) -> Result<Value> {
    let agent_path = get_agent_binary_path();

    if !agent_path.exists() {
        eprintln!("HELP: Run 'cargo build --bin terminator-mcp-agent --release' first");
        return Err(anyhow!(format!("Terminator MCP agent binary not found at: {:?}", agent_path)));
    }

    let transport = ().serve(TokioChildProcess::new(Command::new(agent_path).configure(|cmd| {
        cmd.arg("-t").arg("stdio");
    }))?).await?;
    
    let request = CallToolRequestParam {
        name: std::borrow::Cow::Owned(tool_name.clone()),
        arguments: args,
    };

    let result: CallToolResult = transport.call_tool(request).await?;
    let result_as_json = serde_json::to_value(&result)?;
    debug!("Terminator MCP Tool '{:?}' Result: {:#?}", tool_name, &result);

    Ok(result_as_json)
}

