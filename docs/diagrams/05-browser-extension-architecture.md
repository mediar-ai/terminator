# Browser Extension Architecture

## Overview
This diagram shows how the browser extension integrates with Terminator to provide DOM-level access beyond accessibility APIs.

```mermaid
flowchart TB
    subgraph "Terminator Core"
        CORE[Desktop API]
        BRIDGE[Extension Bridge<br/>Port 17373]
        INSTALLER[Extension Installer]
    end

    subgraph "Browser Process"
        EXT[Chrome Extension]
        BG[Background Script]
        CS[Content Scripts]
    end

    subgraph "Web Pages"
        TAB1[Tab 1: Page A]
        TAB2[Tab 2: Page B]
        TAB3[Tab 3: Page C]
    end

    subgraph "Communication Layer"
        WS[WebSocket Server]
        MSG[Message Router]
        QUEUE[Request Queue]
    end

    subgraph "DOM Access"
        HTML[HTML Elements]
        JS[JavaScript Context]
        EVENTS[Event Listeners]
    end

    CORE --> INSTALLER
    INSTALLER -.->|Auto Install| EXT

    CORE --> BRIDGE
    BRIDGE --> WS
    WS <--> BG

    BG <--> MSG
    MSG <--> CS

    CS <--> TAB1
    CS <--> TAB2
    CS <--> TAB3

    TAB1 --> HTML
    TAB1 --> JS
    TAB1 --> EVENTS

    WS --> QUEUE
    QUEUE --> MSG

    style CORE fill:#e3f2fd
    style EXT fill:#c8e6c9
    style WS fill:#fff3e0
    style TAB1 fill:#fce4ec
```

## Installation Flow

```mermaid
sequenceDiagram
    participant T as Terminator
    participant UI as UI Automation
    participant C as Chrome
    participant E as Extension

    T->>T: Check Extension Status
    T->>UI: Open Chrome Extensions Page
    UI->>C: Navigate to chrome://extensions

    T->>UI: Enable Developer Mode
    UI->>C: Toggle Developer Switch

    T->>UI: Click Load Unpacked
    UI->>C: Open File Dialog

    T->>UI: Select Extension Folder
    UI->>C: Load Extension Files

    C->>E: Initialize Extension
    E->>E: Start Background Script
    E->>E: Open WebSocket

    E-->>T: Connection Established
    T-->>T: Extension Ready
```

## Message Protocol

```mermaid
graph TB
    subgraph "Request Types"
        EXEC[Execute Script]
        EVAL[Evaluate Expression]
        FIND[Find Elements]
        CLICK[Simulate Click]
    end

    subgraph "Request Structure"
        REQ["{<br/>  id: '123',<br/>  type: 'execute',<br/>  tabId: 456,<br/>  script: '...'<br/>}"]
    end

    subgraph "Response Structure"
        RES["{<br/>  id: '123',<br/>  success: true,<br/>  result: {...},<br/>  error: null<br/>}"]
    end

    EXEC --> REQ
    EVAL --> REQ
    FIND --> REQ
    CLICK --> REQ

    REQ --> RES

    style REQ fill:#e3f2fd
    style RES fill:#c8e6c9
```

## Script Execution Flow

```mermaid
flowchart LR
    subgraph "Terminator"
        TOOL[execute_browser_script]
        SELECTOR[Browser Selector]
    end

    subgraph "Bridge"
        VALIDATE[Validate Target]
        PREPARE[Prepare Script]
        SEND[Send via WebSocket]
    end

    subgraph "Extension"
        RECEIVE[Receive Message]
        INJECT[Inject Script]
        EXECUTE[Execute in Page]
        COLLECT[Collect Result]
    end

    subgraph "Page Context"
        DOM[DOM Access]
        WINDOW[Window Object]
        DOC[Document Object]
    end

    TOOL --> SELECTOR
    SELECTOR --> VALIDATE
    VALIDATE --> PREPARE
    PREPARE --> SEND

    SEND --> RECEIVE
    RECEIVE --> INJECT
    INJECT --> EXECUTE

    EXECUTE --> DOM
    EXECUTE --> WINDOW
    EXECUTE --> DOC

    DOM --> COLLECT
    WINDOW --> COLLECT
    DOC --> COLLECT

    COLLECT --> SEND
    SEND --> TOOL

    style TOOL fill:#fff3e0
    style EXECUTE fill:#c8e6c9
```

## DOM vs Accessibility Comparison

```mermaid
graph TB
    subgraph "Accessibility Tree"
        A_WIN[Window: Chrome]
        A_TAB[Tab: Page Title]
        A_BTN[Button: Submit]
        A_EDIT[Edit: Email]
    end

    subgraph "DOM Tree"
        D_HTML[html]
        D_BODY[body]
        D_DIV1[div.container]
        D_FORM[form#loginForm]
        D_INPUT1[input#email type='email']
        D_INPUT2[input#password type='password']
        D_BTN[button.btn-primary]
        D_HIDDEN[input type='hidden' name='csrf']
        D_SCRIPT[script data-config='...']
    end

    A_WIN -.->|Limited View| D_HTML
    A_TAB -.->|Limited View| D_BODY
    A_EDIT -.->|Maps to| D_INPUT1
    A_BTN -.->|Maps to| D_BTN

    D_HTML --> D_BODY
    D_BODY --> D_DIV1
    D_DIV1 --> D_FORM
    D_FORM --> D_INPUT1
    D_FORM --> D_INPUT2
    D_FORM --> D_BTN
    D_FORM --> D_HIDDEN
    D_BODY --> D_SCRIPT

    style A_WIN fill:#e3f2fd
    style D_HIDDEN fill:#ffcdd2
    style D_SCRIPT fill:#ffcdd2

    classDef invisible fill:#ffcdd2
    class D_HIDDEN,D_SCRIPT invisible
```

## Extension Capabilities

### What Extension Can Access
- Full HTML DOM structure
- Hidden form fields
- Data attributes
- JavaScript variables
- CSS computed styles
- Event listeners
- Network requests
- Console output
- Local storage
- Session storage
- Cookies

### What Accessibility API Can't See
- Hidden inputs (type="hidden")
- Data attributes (data-*)
- Script tags content
- CSS pseudo-elements
- Disabled elements (sometimes)
- Shadow DOM content
- iFrames content
- Dynamic JavaScript state

## Security & Performance

```mermaid
graph LR
    subgraph "Security Features"
        ISOLATE[Isolated Contexts]
        SANDBOX[Sandboxed Scripts]
        TIMEOUT[Execution Timeouts]
        VALIDATE[Input Validation]
    end

    subgraph "Performance"
        CACHE[Result Caching]
        BATCH[Batch Operations]
        ASYNC[Async Execution]
        LIMIT[Size Limits: 30KB]
    end

    style Security Features fill:#ffecb3
    style Performance fill:#c8e6c9
```

## Common Use Cases

1. **Extract Full HTML**: `document.documentElement.outerHTML`
2. **Get Form Data**: Collect all form inputs including hidden
3. **Read Data Attributes**: Access `data-*` attributes
4. **Execute Page Functions**: Call existing page JavaScript
5. **Monitor Network**: Track API calls and responses
6. **Debug Accessibility**: Compare DOM vs accessibility tree