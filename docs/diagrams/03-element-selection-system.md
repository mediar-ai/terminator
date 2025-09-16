# Element Selection & Locator System

## Overview
This diagram illustrates how Terminator's sophisticated selector system works, from parsing CSS-like syntax to finding elements in the accessibility tree.

```mermaid
flowchart TB
    subgraph "Selector Input"
        SEL[Selector String<br/>'role:Button|name:Submit']
    end

    subgraph "Parser Stage"
        PARSE[Selector Parser]
        TOKENIZE[Tokenize Components]
        BUILD[Build Criteria Tree]
    end

    subgraph "Selector Types"
        ROLE[Role Selector<br/>role:Button]
        NAME[Name Selector<br/>name:Submit]
        ID[ID Selector<br/>#12345]
        NATIVE[Native ID<br/>nativeid:btn_submit]
        POS[Positional<br/>near:, above:, etc.]
    end

    subgraph "Locator Engine"
        LOCATOR[Create Locator]
        FILTER[Apply Filters]
        CHAIN[Handle Chaining<br/>parent >> child]
    end

    subgraph "Tree Traversal"
        WALK[Walk UI Tree]
        MATCH[Match Criteria]
        COLLECT[Collect Results]
    end

    subgraph "Results"
        SINGLE[Single Element]
        MULTI[Multiple Elements]
        NONE[No Match]
    end

    SEL --> PARSE
    PARSE --> TOKENIZE
    TOKENIZE --> BUILD

    BUILD --> ROLE
    BUILD --> NAME
    BUILD --> ID
    BUILD --> NATIVE
    BUILD --> POS

    ROLE --> LOCATOR
    NAME --> LOCATOR
    ID --> LOCATOR
    NATIVE --> LOCATOR
    POS --> LOCATOR

    LOCATOR --> FILTER
    FILTER --> CHAIN

    CHAIN --> WALK
    WALK --> MATCH
    MATCH --> COLLECT

    COLLECT --> SINGLE
    COLLECT --> MULTI
    COLLECT --> NONE

    style SEL fill:#e3f2fd
    style LOCATOR fill:#fff3e0
    style WALK fill:#e8f5e9
```

## Selector Syntax Examples

```mermaid
graph LR
    subgraph "Basic Selectors"
        B1[role:Button]
        B2[name:Submit]
        B3[#12345]
        B4[text:Click Me]
    end

    subgraph "Compound Selectors"
        C1[role:Button|name:Submit]
        C2[role:Edit|text:Email]
        C3[window:Calculator|role:Button]
    end

    subgraph "Chained Selectors"
        CH1[window:App >> role:Form >> role:Button]
        CH2[role:Dialog >> #submitBtn]
    end

    subgraph "Positional Selectors"
        P1[near:Label|role:Edit]
        P2[rightof:Name|role:Edit]
        P3[below:Header|role:Button]
        P4[above:Footer|nth:0]
    end

    style Basic Selectors fill:#e1f5fe
    style Compound Selectors fill:#fff3e0
    style Chained Selectors fill:#e8f5e9
    style Positional Selectors fill:#fce4ec
```

## Selector Resolution Strategy

```mermaid
flowchart LR
    subgraph "Primary Attempt"
        PRIM[Primary Selector<br/>role:Button|name:OK]
    end

    subgraph "Alternative Selectors"
        ALT1[Alternative 1<br/>#okButton]
        ALT2[Alternative 2<br/>text:OK]
        ALT3[Alternative 3<br/>nativeid:btn_ok]
    end

    subgraph "Fallback Chain"
        FB1[Fallback 1<br/>role:Button|nth:0]
        FB2[Fallback 2<br/>near:Cancel|role:Button]
    end

    PRIM -->|Parallel| ALT1
    PRIM -->|Parallel| ALT2
    PRIM -->|Parallel| ALT3

    PRIM -->|If all fail| FB1
    FB1 -->|If fails| FB2

    style PRIM fill:#bbdefb
    style ALT1 fill:#c8e6c9
    style ALT2 fill:#c8e6c9
    style ALT3 fill:#c8e6c9
    style FB1 fill:#ffe0b2
    style FB2 fill:#ffccbc
```

## Tree Traversal Algorithm

```mermaid
graph TD
    ROOT[Root Element<br/>Desktop/Window]

    ROOT --> W1[Window: Calculator]
    ROOT --> W2[Window: Notepad]

    W1 --> G1[Group: Main]
    G1 --> B1[Button: 7]
    G1 --> B2[Button: 8]
    G1 --> B3[Button: 9]
    G1 --> B4[Button: +]

    W2 --> M1[Menu Bar]
    W2 --> E1[Edit: Text Area]
    M1 --> M2[Menu: File]
    M1 --> M3[Menu: Edit]

    style ROOT fill:#e1f5fe
    style W1 fill:#fff3e0,stroke:#ff9800,stroke-width:3px
    style B2 fill:#c8e6c9,stroke:#4caf50,stroke-width:3px

    classDef matched fill:#c8e6c9,stroke:#4caf50,stroke-width:3px
    classDef searching fill:#fff3e0,stroke:#ff9800,stroke-width:3px
```

## Special Selector Features

### 1. Nth Selector
```
role:Button|nth:2  // Third button (0-indexed)
role:Edit|nth:0    // First edit field
```

### 2. Text Matching
```
text:Submit        // Exact match
text:*Submit*      // Contains
text:/^Submit$/    // Regex
```

### 3. Window Context
```
window:Chrome >> role:Edit    // Edit within Chrome
window:* >> role:Button       // Button in any window
```

### 4. Attribute Matching
```
enabled:true|role:Button      // Only enabled buttons
focused:true|role:Edit        // Currently focused edit
selected:true|role:RadioButton // Selected radio button
```

## Performance Optimizations

1. **ID-based Selection**: Fastest, direct lookup
2. **Role + Name**: Good balance of speed and reliability
3. **Text Search**: Slower, requires content inspection
4. **Positional**: Most expensive, requires geometry calculations

## Edge Cases & Solutions

```mermaid
graph TB
    subgraph "Common Challenges"
        C1[Dynamic IDs<br/>Changes every load]
        C2[Shadow DOM<br/>Hidden elements]
        C3[Virtual Scrolling<br/>Elements not rendered]
        C4[Async Loading<br/>Elements appear later]
        C5[Similar Elements<br/>Multiple matches]
    end

    subgraph "Solutions"
        S1[Use stable attributes<br/>data-testid, aria-label]
        S2[Browser extension<br/>DOM access]
        S3[Scroll into view<br/>Force rendering]
        S4[Wait strategies<br/>Polling, observers]
        S5[Nth selector<br/>Index-based]
    end

    C1 --> S1
    C2 --> S2
    C3 --> S3
    C4 --> S4
    C5 --> S5

    style Common Challenges fill:#ffcdd2
    style Solutions fill:#c8e6c9
```

## Advanced Selector Patterns

### Complex Application Scenarios

```yaml
# Electron/CEF Apps
selector: "window:MyElectronApp >> role:WebView >> role:Button|name:Submit"

# Multi-frame Applications
selector: "role:Frame|name:MainFrame >> role:Frame|name:SubFrame >> role:Edit"

# Dynamic SPAs
selector: "role:Region|name:Content >> role:List >> role:ListItem|nth:2"

# Data-driven Selection
selector: "role:Cell|value:{{targetValue}} >> near:Edit"

# Accessibility Landmarks
selector: "role:Main >> role:Navigation >> role:Link|name:Home"
```

## Selector Scoring Algorithm

```mermaid
flowchart TB
    subgraph "Scoring Factors"
        F1[Uniqueness<br/>+100 pts]
        F2[Stability<br/>+50 pts]
        F3[Performance<br/>+30 pts]
        F4[Readability<br/>+20 pts]
    end

    subgraph "Score Calculation"
        CALC[Total Score = Σ(Factor × Weight)]
    end

    subgraph "Examples"
        E1["#uniqueId<br/>Score: 180"]
        E2["role:Button|name:Submit<br/>Score: 150"]
        E3["nth:3|role:Button<br/>Score: 80"]
        E4["near:Label|role:Edit<br/>Score: 60"]
    end

    F1 --> CALC
    F2 --> CALC
    F3 --> CALC
    F4 --> CALC

    CALC --> E1
    CALC --> E2
    CALC --> E3
    CALC --> E4

    style E1 fill:#4caf50
    style E2 fill:#8bc34a
    style E3 fill:#ffc107
    style E4 fill:#ff9800
```

## Platform-Specific Selectors

### Windows
```
# UIA Automation ID
nativeid:ButtonSubmit

# Control Type + ClassName
role:Button|class:WindowsForms10.BUTTON

# Legacy patterns
pattern:Invoke|name:OK
```

### macOS
```
# AX Attributes
role:AXButton|description:Submit form

# Subrole matching
subrole:AXCloseButton

# Help text
help:Click to submit the form
```

### Linux
```
# AT-SPI States
state:ENABLED|role:push-button

# Desktop environment specific
desktop:GNOME|role:Button
```

## Fuzzy Matching

```mermaid
graph LR
    subgraph "Input"
        IN[User Selector<br/>"role:Buton|name:Submt"]
    end

    subgraph "Fuzzy Engine"
        LEV[Levenshtein<br/>Distance]
        SOUND[Soundex<br/>Algorithm]
        STEM[Word<br/>Stemming]
    end

    subgraph "Candidates"
        C1[role:Button ✓]
        C2[name:Submit ✓]
        C3[name:Submitted ✓]
    end

    subgraph "Result"
        RES[Best Match<br/>role:Button|name:Submit<br/>Confidence: 95%]
    end

    IN --> LEV
    IN --> SOUND
    IN --> STEM

    LEV --> C1
    LEV --> C2
    SOUND --> C3

    C1 --> RES
    C2 --> RES

    style IN fill:#ffecb3
    style RES fill:#c8e6c9
```

## Error Recovery Strategies

```mermaid
stateDiagram-v2
    [*] --> TryPrimary
    TryPrimary --> Success: Found
    TryPrimary --> TryAlternatives: Not Found

    TryAlternatives --> Success: Found
    TryAlternatives --> TryFallbacks: Not Found

    TryFallbacks --> Success: Found
    TryFallbacks --> RefreshTree: Not Found

    RefreshTree --> TryBroader: Tree Updated
    TryBroader --> Success: Found
    TryBroader --> TryID: Not Found

    TryID --> Success: Found
    TryID --> TryPositional: Not Found

    TryPositional --> Success: Found
    TryPositional --> UseOCR: Not Found

    UseOCR --> Success: Text Found
    UseOCR --> Fail: No Match

    Success --> [*]
    Fail --> [*]
```

## Performance Optimization Tips

1. **ID Selection**: O(1) - Direct lookup, fastest
2. **Role + Name**: O(n) - Single pass, efficient
3. **Text Search**: O(n*m) - Content inspection, slower
4. **Positional**: O(n²) - Geometry calculations, expensive
5. **OCR Fallback**: O(n³) - Image processing, last resort

## Caching Strategy

```mermaid
graph TB
    subgraph "Cache Layers"
        L1[L1: Selector → Element<br/>TTL: 100ms]
        L2[L2: Tree Structure<br/>TTL: 500ms]
        L3[L3: OCR Results<br/>TTL: 5s]
    end

    subgraph "Invalidation"
        INV1[Window Change]
        INV2[Focus Change]
        INV3[User Action]
    end

    L1 --> L2 --> L3
    INV1 --> L1
    INV2 --> L1
    INV3 --> L2

    style L1 fill:#e3f2fd
    style L2 fill:#fff3e0
    style L3 fill:#f0f4c3
```