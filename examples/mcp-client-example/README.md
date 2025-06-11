# MCP Client Example

This example demonstrates how to use the Model Context Protocol (MCP) in Rust as a client to connect to the `terminator-mcp-agent` server and interact with UI automation tools.

## What This Example Does

The MCP client example showcases:

1. **MCP Connection**: Establishes a connection to the terminator-mcp-agent via stdio transport
2. **Basic Client Setup**: Shows the minimal code needed to create an MCP client in Rust
3. **Error Handling**: Demonstrates proper error handling for MCP connections
4. **Connection Management**: Shows how to manage the lifecycle of an MCP connection

## Prerequisites

1. **Build the MCP Agent**: First, build the terminator-mcp-agent:
   ```bash
   cargo build --release --bin terminator-mcp-agent
   ```

2. **Platform Requirements**: 
   - **Windows**: Full UI automation support
   - **Linux**: Requires desktop environment (won't work in headless/Docker environments)
   - **macOS**: Requires desktop environment and accessibility permissions

## Running the Example

### Option 1: From the example directory
```bash
cd examples/mcp-client-example
cargo run --bin mcp_client_example
```

### Option 2: From the workspace root
```bash
cargo run --manifest-path examples/mcp-client-example/Cargo.toml --bin mcp_client_example
```

## Expected Behavior

### In Desktop Environments
The example should output:
```
INFO  Starting MCP Client Example
INFO  Looking for terminator-mcp-agent at: /path/to/target/release/terminator-mcp-agent
INFO  Spawning terminator-mcp-agent process...
INFO  ✅ Successfully connected to terminator-mcp-agent via MCP!
INFO  � The MCP client is connected and ready!
INFO  🎉 MCP Client example completed successfully!
```

### In Headless Environments (Expected)
The example may fail with a terminator initialization error like:
```
Error: Failed to initialize terminator desktop("Platform-specific error: ZBus Error: Address...")
```

**This is expected behavior** in headless environments (CI, Docker, SSH sessions without X11 forwarding). The MCP connection itself works correctly, but the terminator UI automation library requires a desktop environment.

## Understanding the Code

### Key Components

1. **MCP Client Setup**:
   ```rust
   let _client = ()
       .serve(TokioChildProcess::new(&mut cmd)?)
       .await?;
   ```

2. **Transport Configuration**:
   The example uses `TokioChildProcess` which spawns the MCP server as a subprocess and communicates via stdio.

3. **Connection Management**:
   The example demonstrates the basic connection lifecycle but keeps it simple for educational purposes.

### Success Indicators

Even if the final connection fails due to desktop environment issues, the example is working correctly if you see:
- ✅ MCP agent binary found
- ✅ Process spawning successful  
- ✅ MCP initialization attempted
- ✅ Clean error handling

## Extending the Example

In a desktop environment, you could extend this example to:

- **List Tools**: Query available UI automation tools
- **Call Tools**: Execute specific automation commands  
- **Handle Responses**: Process tool results and errors
- **Resource Management**: Access and subscribe to resources
- **Interactive Mode**: Build a CLI or UI for manual tool invocation

Example extension (for desktop environments):
```rust
// After successful connection, you could add:
// let tools = client.list_tools().await?;
// let result = client.call_tool("get_applications", json!({})).await?;
```

## Troubleshooting

### "MCP agent binary not found"
- Ensure you've built the agent: `cargo build --release --bin terminator-mcp-agent`
- Check the path in the error message

### "Failed to initialize terminator desktop"
- **Expected in headless environments** (CI, Docker containers, SSH without X11)
- Try running on a system with a desktop environment
- On Linux, ensure you have a working X11 or Wayland session

### Permission Errors
- On Linux, you may need accessibility permissions
- On macOS, accessibility permissions may be required
- On Windows, some operations may require administrator privileges

## MCP Resources

- [Model Context Protocol Specification](https://spec.modelcontextprotocol.io/)
- [RMCP Rust SDK Documentation](https://docs.rs/rmcp/)
- [Terminator UI Automation Library](https://github.com/mediar-ai/terminator)