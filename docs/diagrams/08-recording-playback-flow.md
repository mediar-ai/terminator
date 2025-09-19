# Recording & Playback Flow

## Overview
This diagram illustrates how Terminator records human interactions and converts them into reusable automation workflows.

```mermaid
flowchart TB
    subgraph "Human Interaction"
        USER[User Actions<br/>Click, Type, Scroll]
    end

    subgraph "Event Capture Layer"
        HOOK[UI Automation Events<br/>Windows Only]
        FILTER[Event Filter<br/>Remove Noise]
        BUFFER[Event Buffer]
    end

    subgraph "Processing Pipeline"
        ANALYZER[Action Analyzer]
        MERGER[Similar Action Merger]
        OPTIMIZER[Sequence Optimizer]
    end

    subgraph "Workflow Generation"
        BUILDER[YAML Builder]
        SELECTOR[Selector Generator]
        VALIDATOR[Validation Logic]
    end

    subgraph "Output Formats"
        YAML[YAML Workflow]
        JSON[JSON Format]
        CODE[SDK Code]
    end

    subgraph "Playback Engine"
        LOADER[Workflow Loader]
        EXECUTOR[Step Executor]
        MONITOR[Execution Monitor]
    end

    USER --> HOOK
    HOOK --> FILTER
    FILTER --> BUFFER

    BUFFER --> ANALYZER
    ANALYZER --> MERGER
    MERGER --> OPTIMIZER

    OPTIMIZER --> BUILDER
    BUILDER --> SELECTOR
    SELECTOR --> VALIDATOR

    VALIDATOR --> YAML
    VALIDATOR --> JSON
    VALIDATOR --> CODE

    YAML --> LOADER
    LOADER --> EXECUTOR
    EXECUTOR --> MONITOR

    style USER fill:#e3f2fd
    style HOOK fill:#fff3e0
    style BUILDER fill:#c8e6c9
    style YAML fill:#e8f5e9
```

## Recording Process

```mermaid
sequenceDiagram
    participant U as User
    participant R as Recorder
    participant E as Event System
    participant P as Processor
    participant W as Workflow

    U->>R: Start Recording
    R->>E: Register Event Handlers
    E-->>R: Ready

    Note over U,E: User performs actions

    U->>E: Click Button
    E->>R: UIAutomationEvent
    R->>P: Process Click Event

    U->>E: Type Text
    E->>R: ValueChangeEvent
    R->>P: Process Text Input

    U->>E: Select Dropdown
    E->>R: SelectionChangeEvent
    R->>P: Process Selection

    U->>R: Stop Recording

    R->>P: Finalize Sequence
    P->>W: Generate YAML
    W-->>U: Save Workflow
```

## Event Processing Pipeline

```mermaid
graph LR
    subgraph "Raw Events"
        E1[MouseMove]
        E2[MouseClick]
        E3[KeyPress 'H']
        E4[KeyPress 'e']
        E5[KeyPress 'l']
        E6[MouseMove]
        E7[FocusChange]
    end

    subgraph "Filter Stage"
        FILTER[Remove Noise<br/>- Redundant moves<br/>- Focus changes<br/>- System events]
    end

    subgraph "Merge Stage"
        MERGE[Combine Events<br/>- Keystrokes → Text<br/>- Rapid clicks → Double-click]
    end

    subgraph "Clean Events"
        C1[Click at Button]
        C2[Type 'Hello']
    end

    E1 --> FILTER
    E2 --> FILTER
    E3 --> FILTER
    E4 --> FILTER
    E5 --> FILTER
    E6 --> FILTER
    E7 --> FILTER

    FILTER --> MERGE

    MERGE --> C1
    MERGE --> C2

    style E1 fill:#ffcdd2
    style E6 fill:#ffcdd2
    style E7 fill:#ffcdd2
    style C1 fill:#c8e6c9
    style C2 fill:#c8e6c9
```

## Selector Generation Strategy

```mermaid
flowchart TB
    subgraph "Element Info"
        ELEM[Clicked Element<br/>- Name: Submit<br/>- Role: Button<br/>- ID: 12345<br/>- Parent: Form]
    end

    subgraph "Selector Generation"
        ANALYZE[Analyze Properties]
        SCORE[Score Uniqueness]
        BUILD[Build Options]
    end

    subgraph "Selector Options"
        OPT1[Primary: role:Button|name:Submit]
        OPT2[Alternative: #12345]
        OPT3[Fallback: window:App >> role:Form >> role:Button]
    end

    subgraph "Final Selector"
        FINAL[selector: role:Button|name:Submit<br/>alternative_selectors: #12345<br/>fallback_selectors: window:App >> role:Button]
    end

    ELEM --> ANALYZE
    ANALYZE --> SCORE
    SCORE --> BUILD

    BUILD --> OPT1
    BUILD --> OPT2
    BUILD --> OPT3

    OPT1 --> FINAL
    OPT2 --> FINAL
    OPT3 --> FINAL

    style ELEM fill:#e3f2fd
    style FINAL fill:#c8e6c9
```

## Workflow Optimization

```mermaid
graph TB
    subgraph "Raw Recording"
        R1[Click Edit1]
        R2[Clear Edit1]
        R3[Type 'John']
        R4[Click Edit2]
        R5[Clear Edit2]
        R6[Type 'Doe']
        R7[Click Submit]
        R8[Wait 100ms]
        R9[Click Submit]
    end

    subgraph "Optimization Rules"
        RULE1[Merge Clear+Type]
        RULE2[Remove Duplicates]
        RULE3[Add Smart Waits]
        RULE4[Group Related]
    end

    subgraph "Optimized Workflow"
        O1[Group: Fill Form<br/>- Type 'John' in Edit1<br/>- Type 'Doe' in Edit2]
        O2[Click Submit]
        O3[Wait for: Success Message]
    end

    R1 --> RULE1
    R2 --> RULE1
    R3 --> RULE1
    R4 --> RULE1
    R5 --> RULE1
    R6 --> RULE1

    R7 --> RULE2
    R8 --> RULE3
    R9 --> RULE2

    RULE1 --> O1
    RULE2 --> O2
    RULE3 --> O3
    RULE4 --> O1

    style Raw Recording fill:#ffecb3
    style Optimized Workflow fill:#c8e6c9
```

## Playback Execution

```mermaid
stateDiagram-v2
    [*] --> LoadWorkflow
    LoadWorkflow --> ParseYAML
    ParseYAML --> InitializeContext

    InitializeContext --> ExecuteStep
    ExecuteStep --> FindElement
    FindElement --> PerformAction
    PerformAction --> VerifyResult

    VerifyResult --> Success: Action succeeded
    VerifyResult --> Retry: Action failed, retries left
    VerifyResult --> Fallback: Use fallback step
    VerifyResult --> Fail: No retries/fallback

    Success --> NextStep
    Retry --> ExecuteStep
    Fallback --> ExecuteFallback
    ExecuteFallback --> NextStep

    NextStep --> ExecuteStep: More steps
    NextStep --> Complete: No more steps

    Fail --> ErrorHandler
    ErrorHandler --> Complete: Continue on error
    ErrorHandler --> [*]: Stop on error

    Complete --> [*]
```

## Recording Features

### Smart Detection
- **Form Recognition**: Groups form field interactions
- **Repetition Detection**: Identifies loops and patterns
- **Wait Inference**: Adds waits based on loading patterns

### Noise Filtering
- Mouse movements without clicks
- Rapid focus changes
- System notifications
- Tooltip hovers

### Action Enhancement
- Converts coordinates to element selectors
- Adds verification steps automatically
- Includes error recovery logic

## Playback Features

### Adaptive Execution
- **Dynamic Waits**: Wait for element states
- **Smart Retries**: Exponential backoff
- **Alternative Paths**: Fallback strategies
- **State Verification**: Ensure expected outcomes

### Error Recovery
```yaml
steps:
  - tool_name: click_element
    arguments:
      selector: "role:Button|name:Submit"
    retries: 3
    continue_on_error: true
    fallback_id: keyboard_submit

  - id: keyboard_submit
    tool_name: press_key
    arguments:
      key: "{Enter}"
```

## Generated Workflow Example

```yaml
name: "Login Flow"
description: "Recorded on 2024-01-15"

steps:
  - tool_name: type_into_element
    arguments:
      selector: "role:Edit|name:Email"
      text_to_type: "user@example.com"
      clear_before_typing: true

  - tool_name: type_into_element
    arguments:
      selector: "role:Edit|name:Password"
      text_to_type: "{{password}}"
      clear_before_typing: true

  - tool_name: click_element
    arguments:
      selector: "role:Button|name:Sign In"
      alternative_selectors: "#loginBtn"

  - tool_name: wait_for_element
    arguments:
      selector: "role:Heading|name:Dashboard"
      condition: "visible"
      timeout_ms: 5000
```