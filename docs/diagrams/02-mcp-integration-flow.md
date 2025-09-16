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

## Key Integration Features

### 1. Selector Strategy
- **Primary**: `role:Button|name:Submit`
- **Alternatives**: Parallel attempts
- **Fallbacks**: Sequential if primary fails
- **ID-based**: `#12345` for precision

### 2. Error Handling
- Element not found → Suggest alternatives
- Timeout → Increase wait or check state
- Disabled element → Check prerequisites
- Focus lost → Activate window first

### 3. Performance Optimizations
- `include_tree: false` by default
- Batch operations in sequences
- Cache element references
- Smart retry logic

### 4. AI-Friendly Design
- Descriptive error messages
- Suggested next actions
- State verification tools
- Visual highlighting for debugging