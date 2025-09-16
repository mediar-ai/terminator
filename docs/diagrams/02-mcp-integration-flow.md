# MCP Integration & AI Workflow

## Overview
This diagram shows how AI models interact with Terminator through the Model Context Protocol (MCP), demonstrating the request flow from AI to desktop automation.

```mermaid
sequenceDiagram
    participant AI as AI Model<br/>(Claude/GPT)
    participant MCP as MCP Server<br/>(terminator-mcp-agent)
    participant CORE as Terminator Core<br/>(Desktop API)
    participant OS as Operating System<br/>(UI Automation)
    participant APP as Target Application

    Note over AI,MCP: MCP Protocol (stdio/HTTP)

    AI->>MCP: Tool Request<br/>(e.g., click_element)
    activate MCP

    MCP->>MCP: Parse Arguments<br/>Validate Selector

    MCP->>CORE: Desktop.locator()<br/>Find Element
    activate CORE

    CORE->>OS: Query Accessibility Tree
    activate OS
    OS->>APP: Get UI Elements
    activate APP
    APP-->>OS: Element Tree
    deactivate APP
    OS-->>CORE: Matching Elements
    deactivate OS

    CORE-->>MCP: UIElement Reference
    deactivate CORE

    MCP->>CORE: element.click()
    activate CORE
    CORE->>OS: SendInput/Click Event
    activate OS
    OS->>APP: Mouse Click
    activate APP
    APP-->>OS: Event Handled
    deactivate APP
    OS-->>CORE: Success
    deactivate OS
    CORE-->>MCP: Action Result
    deactivate CORE

    MCP->>CORE: Get UI Tree<br/>(if requested)
    activate CORE
    CORE->>OS: Refresh Tree
    OS-->>CORE: Updated Tree
    deactivate CORE

    MCP-->>AI: Tool Response<br/>+ UI State
    deactivate MCP

    Note over AI: AI processes result<br/>Plans next action
```

## MCP Tools Categories

```mermaid
graph LR
    subgraph "Discovery Tools"
        GET_TREE[get_window_tree<br/>get_focused_window_tree]
        GET_APPS[get_applications]
        VALIDATE[validate_element]
        LIST_OPT[list_options]
    end

    subgraph "Action Tools"
        CLICK[click_element]
        TYPE[type_into_element]
        PRESS[press_key]
        DRAG[mouse_drag]
        SCROLL[scroll_element]
    end

    subgraph "State Tools"
        SET_VAL[set_value]
        SET_TOG[set_toggled]
        SET_SEL[set_selected]
        SET_RANGE[set_range_value]
    end

    subgraph "Workflow Tools"
        EXEC_SEQ[execute_sequence]
        RECORD[record_workflow]
        IMPORT[import_workflow]
        EXPORT[export_workflow]
    end

    subgraph "Browser Tools"
        NAV[navigate_browser]
        EXEC_JS[execute_browser_script]
        TABS[browser_tabs]
    end

    subgraph "Utility Tools"
        SCREENSHOT[capture_element_screenshot]
        HIGHLIGHT[highlight_element]
        WAIT[wait_for_element]
        DELAY[delay]
    end

    style Discovery Tools fill:#e3f2fd
    style Action Tools fill:#ffecb3
    style State Tools fill:#c8e6c9
    style Workflow Tools fill:#f3e5f5
    style Browser Tools fill:#ffccbc
    style Utility Tools fill:#f1f8e9
```

## Request/Response Flow

```mermaid
flowchart TB
    subgraph "AI Request"
        REQ[Tool: click_element<br/>Args: selector, timeout_ms, include_tree]
    end

    subgraph "MCP Processing"
        PARSE[Parse & Validate Arguments]
        RESOLVE[Resolve Selector<br/>Alternative Fallbacks]
        EXEC[Execute Action]
        TREE[Get UI Tree<br/>Optional]
    end

    subgraph "Response Structure"
        SUCCESS[Success Response<br/>- Result: true/false<br/>- Element Info<br/>- UI Tree Optional]
        ERROR[Error Response<br/>- Error Type<br/>- Message<br/>- Suggestions]
    end

    REQ --> PARSE
    PARSE --> RESOLVE
    RESOLVE --> EXEC
    EXEC --> TREE
    TREE --> SUCCESS
    EXEC -.->|On Error| ERROR
```

## Real-World Request Examples

```mermaid
graph TB
    subgraph "Login Automation"
        L1[1. get_window_tree]
        L2[2. type_into_element<br/>username field]
        L3[3. type_into_element<br/>password field]
        L4[4. click_element<br/>login button]
        L5[5. wait_for_element<br/>dashboard]
    end

    subgraph "Data Extraction"
        D1[1. navigate_browser<br/>target URL]
        D2[2. execute_browser_script<br/>extract DOM]
        D3[3. parse & process<br/>structured data]
    end

    subgraph "Form Filling"
        F1[1. execute_sequence<br/>load workflow]
        F2[2. parallel fill<br/>multiple fields]
        F3[3. validate_element<br/>check states]
        F4[4. submit & verify]
    end

    L1 --> L2 --> L3 --> L4 --> L5
    D1 --> D2 --> D3
    F1 --> F2 --> F3 --> F4

    style L1 fill:#e3f2fd
    style D1 fill:#fff3e0
    style F1 fill:#e8f5e9
```

## Performance Metrics

```mermaid
graph LR
    subgraph "Response Times"
        RT1[get_tree: 20-100ms]
        RT2[click: 10-30ms]
        RT3[type: 50-200ms]
        RT4[screenshot: 100-500ms]
    end

    subgraph "Throughput"
        T1[Requests/sec: 100-500]
        T2[Concurrent: 10-50]
        T3[Queue depth: 1000]
    end

    subgraph "Success Rates"
        S1[Element found: 95%+]
        S2[Action success: 98%+]
        S3[Auto-retry: 80% recovery]
    end

    style Response Times fill:#e1f5fe
    style Throughput fill:#fff3e0
    style Success Rates fill:#c8e6c9
```

## Common Integration Patterns

### 1. Robust Element Selection
```json
{
  "method": "click_element",
  "params": {
    "selector": "role:Button|name:Submit",
    "alternative_selectors": "#submitBtn, text:Submit",
    "fallback_selectors": "nth:0|role:Button",
    "timeout_ms": 5000,
    "retries": 3,
    "include_tree": false
  }
}
```

### 2. Conditional Workflows
```json
{
  "method": "execute_sequence",
  "params": {
    "steps": [
      {
        "tool_name": "validate_element",
        "arguments": {"selector": "role:Dialog"},
        "id": "check_dialog"
      },
      {
        "tool_name": "click_element",
        "arguments": {"selector": "role:Button|name:Close"},
        "if": "env.check_dialog.exists"
      }
    ]
  }
}
```

### 3. Error Recovery
- **Automatic retries** with exponential backoff
- **Fallback selectors** for resilience
- **Alternative actions** (click → invoke → press Enter)
- **State verification** before and after actions

### 4. AI Learning Loop
```mermaid
sequenceDiagram
    participant AI as AI Model
    participant MCP as MCP Server
    participant REC as Recorder
    participant WF as Workflow

    AI->>MCP: Attempt automation
    MCP-->>AI: Partial failure

    AI->>REC: Start recording
    Note over AI: Human demonstrates
    REC-->>WF: Generate workflow

    WF-->>AI: Learn pattern
    AI->>MCP: Retry with learning
    MCP-->>AI: Success

    Note over AI: Knowledge updated
```

## Best Practices

1. **Always verify state** before actions
2. **Use multiple selector strategies**
3. **Include error context** in responses
4. **Batch related operations** for efficiency
5. **Cache UI trees** when doing multiple operations
6. **Use highlighting** for debugging
7. **Implement graceful degradation**