use anyhow::Result;
use rmcp::{
    RoleClient,
    ServiceExt,
    service::RunningService,
    transport::{
        TokioChildProcess,
        StreamableHttpClientTransport,
    }, 
    model::{
        ClientInfo,
        Implementation,
        ClientCapabilities,
        InitializeRequestParam
    },
};
use crate::{
    workflow_exec::workflow::Transport,
    utils::{find_executable, create_command},

};

pub async fn connect_to_mcp(transport: Transport) -> Result<RunningService<RoleClient, InitializeRequestParam>> {
    match transport {
        Transport::Http(url) => {
            println!("Connecting to MCP server: {url}");
            let transport = StreamableHttpClientTransport::from_uri(url.as_str());
            let client_info = ClientInfo {
                protocol_version: Default::default(),
                capabilities: ClientCapabilities::default(),
                client_info: Implementation {
                    name: "terminator-cli-ai".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            };
            Ok(client_info.serve(transport).await?)
        }
        Transport::Stdio(command) => {
            println!("Starting MCP server: {}", command.join(" "));
            let executable = find_executable(&command[0]).unwrap_or_else(|| command[0].clone());
            let command_args: Vec<String> = if command.len() > 1 {
                command[1..].to_vec()
            } else {
                vec![]
            };
            let cmd = create_command(&executable, &command_args);
            let transport = TokioChildProcess::new(cmd)?;
            let client_info = ClientInfo {
                protocol_version: Default::default(),
                capabilities: ClientCapabilities::default(),
                client_info: Implementation {
                    name: "terminator-cli-ai".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            };
            Ok(client_info.serve(transport).await?)
        }
    }
}
