# Component Communication Architecture

## Overview
This diagram shows how different Terminator components communicate with each other through various protocols and data formats.

```mermaid
graph TB
    subgraph "External Clients"
        CLAUDE[Claude/AI Models]
        USER[End User]
        SCRIPTS[Automation Scripts]
    end

    subgraph "Communication Protocols"
        MCP_STDIO[MCP stdio]
        MCP_HTTP[MCP HTTP<br/>Port 3000]
        WEBSOCKET[WebSocket<br/>Port 17373]
        GRPC[gRPC<br/>Future]
    end

    subgraph "Terminator Components"
        MCP_AGENT[MCP Agent]
        CLI[CLI Tool]
        SDK_PY[Python SDK]
        SDK_JS[Node.js SDK]
        CORE[Core Library]
        RECORDER[Recorder]
        EXTENSION[Browser Ext]
    end

    subgraph "Data Formats"
        JSON_RPC[JSON-RPC 2.0]
        YAML_WF[YAML Workflows]
        JSON_WF[JSON Workflows]
        BINARY[Binary Protocol]
    end

    CLAUDE --> MCP_STDIO
    CLAUDE --> MCP_HTTP
    USER --> CLI
    SCRIPTS --> SDK_PY
    SCRIPTS --> SDK_JS

    MCP_STDIO --> MCP_AGENT
    MCP_HTTP --> MCP_AGENT

    CLI --> CORE
    SDK_PY --> CORE
    SDK_JS --> CORE
    MCP_AGENT --> CORE

    CORE <--> WEBSOCKET
    WEBSOCKET <--> EXTENSION

    RECORDER --> YAML_WF
    CLI --> YAML_WF
    CLI --> JSON_WF

    MCP_AGENT --> JSON_RPC
    EXTENSION --> JSON_RPC

    style CLAUDE fill:#e3f2fd
    style CORE fill:#fff3e0
    style MCP_AGENT fill:#c8e6c9
```

## Protocol Details

### MCP (Model Context Protocol)

```mermaid
sequenceDiagram
    participant Client as MCP Client
    participant Server as MCP Server
    participant Core as Terminator Core

    Note over Client,Server: JSON-RPC 2.0 Protocol

    Client->>Server: {"jsonrpc": "2.0", "method": "initialize"}
    Server-->>Client: {"capabilities": {...}}

    Client->>Server: {"method": "tools/call", "params": {"name": "click_element"}}
    Server->>Core: Desktop.locator().click()
    Core-->>Server: Result
    Server-->>Client: {"result": {...}}

    Client->>Server: {"method": "tools/list"}
    Server-->>Client: {"tools": [50+ tools]}
```

### WebSocket Bridge (Browser Extension)

```mermaid
sequenceDiagram
    participant T as Terminator
    participant WS as WebSocket Server
    participant E as Extension
    participant P as Web Page

    T->>WS: Start server :17373
    E->>WS: Connect
    WS-->>E: Connection ACK

    T->>WS: Execute script request
    WS->>E: Forward request
    E->>P: Inject & execute
    P-->>E: Execution result
    E-->>WS: Send result
    WS-->>T: Return result
```

## Inter-Process Communication

```mermaid
graph LR
    subgraph "Process 1: MCP Agent"
        MCP_PROC[MCP Process<br/>Node.js/Rust]
    end

    subgraph "Process 2: Core"
        CORE_PROC[Terminator Core<br/>Rust Native]
    end

    subgraph "Process 3: Browser"
        BROWSER[Chrome/Edge<br/>+ Extension]
    end

    subgraph "IPC Methods"
        STDIO[stdio pipes]
        SHARED_MEM[Shared Memory]
        SOCKET[TCP/Unix Socket]
    end

    MCP_PROC <--> STDIO
    STDIO <--> CORE_PROC

    CORE_PROC <--> SOCKET
    SOCKET <--> BROWSER

    style MCP_PROC fill:#e3f2fd
    style CORE_PROC fill:#fff3e0
    style BROWSER fill:#fce4ec
```

## Message Flow Patterns

### Request-Response Pattern

```mermaid
flowchart LR
    subgraph "Client"
        REQ[Request<br/>ID: 123]
        WAIT[Wait for Response]
        PROC[Process Result]
    end

    subgraph "Server"
        RECV[Receive Request]
        EXEC[Execute Action]
        RESP[Send Response<br/>ID: 123]
    end

    REQ --> RECV
    RECV --> EXEC
    EXEC --> RESP
    RESP --> WAIT
    WAIT --> PROC

    style REQ fill:#e3f2fd
    style RESP fill:#c8e6c9
```

### Event Streaming Pattern

```mermaid
flowchart TB
    subgraph "Recorder"
        EVENT1[Mouse Click]
        EVENT2[Key Press]
        EVENT3[Text Input]
    end

    subgraph "Event Queue"
        QUEUE[Event Buffer]
        BATCH[Batch Processor]
    end

    subgraph "Workflow Builder"
        BUILD[Build Steps]
        OPTIMIZE[Optimize Sequence]
        SAVE[Save YAML]
    end

    EVENT1 --> QUEUE
    EVENT2 --> QUEUE
    EVENT3 --> QUEUE

    QUEUE --> BATCH
    BATCH --> BUILD
    BUILD --> OPTIMIZE
    OPTIMIZE --> SAVE

    style EVENT1 fill:#ffecb3
    style SAVE fill:#c8e6c9
```

## Port & Endpoint Map

```mermaid
graph TB
    subgraph "Network Ports"
        P3000[Port 3000<br/>MCP HTTP Server]
        P17373[Port 17373<br/>WebSocket Bridge]
        STDIO_PIPE[stdio<br/>Process Pipes]
    end

    subgraph "HTTP Endpoints"
        INIT[POST /initialize]
        TOOLS[GET /tools]
        CALL[POST /tools/call]
        HEALTH[GET /health]
    end

    subgraph "WebSocket Messages"
        WS_EXEC[execute_script]
        WS_EVAL[evaluate]
        WS_TAB[tab_control]
    end

    P3000 --> INIT
    P3000 --> TOOLS
    P3000 --> CALL
    P3000 --> HEALTH

    P17373 --> WS_EXEC
    P17373 --> WS_EVAL
    P17373 --> WS_TAB

    style P3000 fill:#e3f2fd
    style P17373 fill:#fff3e0
```

## Data Serialization

```mermaid
graph LR
    subgraph "Rust Structures"
        RUST[Native Rust Types<br/>UIElement, Desktop]
    end

    subgraph "Serialization"
        SERDE[Serde Framework]
        JSON_S[JSON Serializer]
        BINCODE[Bincode]
    end

    subgraph "Language Bindings"
        PY_OBJ[Python Objects]
        JS_OBJ[JavaScript Objects]
        JSON_OBJ[JSON Objects]
    end

    RUST --> SERDE
    SERDE --> JSON_S
    SERDE --> BINCODE

    JSON_S --> PY_OBJ
    JSON_S --> JS_OBJ
    JSON_S --> JSON_OBJ

    style RUST fill:#dce775
    style SERDE fill:#fff3e0
    style JSON_OBJ fill:#c8e6c9
```

## Error Propagation

```mermaid
flowchart TB
    subgraph "Error Origin"
        OS_ERR[OS API Error]
        TIMEOUT[Timeout Error]
        NOT_FOUND[Element Not Found]
    end

    subgraph "Core Layer"
        CORE_ERR[TerminatorError]
        MAP_ERR[Error Mapping]
    end

    subgraph "Transport Layer"
        JSON_ERR[JSON-RPC Error]
        HTTP_ERR[HTTP Status Code]
    end

    subgraph "Client Layer"
        PY_EXC[Python Exception]
        JS_EXC[JavaScript Error]
        MCP_ERR[MCP Tool Error]
    end

    OS_ERR --> CORE_ERR
    TIMEOUT --> CORE_ERR
    NOT_FOUND --> CORE_ERR

    CORE_ERR --> MAP_ERR
    MAP_ERR --> JSON_ERR
    MAP_ERR --> HTTP_ERR

    JSON_ERR --> PY_EXC
    JSON_ERR --> JS_EXC
    JSON_ERR --> MCP_ERR

    style OS_ERR fill:#ffcdd2
    style CORE_ERR fill:#ffecb3
    style MCP_ERR fill:#ef5350
```

## Performance Considerations

1. **Connection Pooling**: Reuse WebSocket connections
2. **Message Batching**: Group multiple operations
3. **Binary Protocol**: Use for high-frequency data
4. **Async Processing**: Non-blocking I/O everywhere
5. **Cache Layer**: Reduce repeated tree queries