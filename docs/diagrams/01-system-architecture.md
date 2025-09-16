# Terminator System Architecture

## Overview
This diagram illustrates the complete system architecture of Terminator, showing how different components interact to provide desktop automation capabilities.

```mermaid
graph TB
    subgraph "AI Integration Layer"
        AI[AI Models<br/>Claude, GPT, etc.]
        MCP[MCP Protocol<br/>50+ Tools]
    end

    subgraph "Client Layer"
        CLI[Terminator CLI<br/>Workflow Executor]
        PY[Python SDK<br/>terminator.py]
        JS[Node.js/TS SDK<br/>terminator.js]
        RUST[Rust SDK<br/>Native API]
    end

    subgraph "Core Engine"
        DESKTOP[Desktop API<br/>Unified Interface]
        ENGINE[Accessibility Engine<br/>Trait Abstraction]
        LOCATOR[Locator System<br/>Element Selection]
        SELECTOR[Selector Parser<br/>CSS-like Syntax]
    end

    subgraph "Platform Layer"
        WIN[Windows<br/>UI Automation]
        MAC[macOS<br/>Accessibility API]
        LINUX[Linux<br/>AT-SPI]
    end

    subgraph "Extensions"
        BROWSER[Browser Extension<br/>Chrome/Edge]
        BRIDGE[WebSocket Bridge<br/>Port 17373]
        RECORDER[Workflow Recorder<br/>Event Capture]
    end

    subgraph "Storage & Config"
        YAML[YAML Workflows]
        JSON[JSON Config]
        CACHE[Element Cache]
    end

    AI --> MCP
    MCP --> CLI

    CLI --> DESKTOP
    PY --> DESKTOP
    JS --> DESKTOP
    RUST --> DESKTOP

    DESKTOP --> ENGINE
    ENGINE --> LOCATOR
    LOCATOR --> SELECTOR

    ENGINE --> WIN
    ENGINE --> MAC
    ENGINE --> LINUX

    BROWSER --> BRIDGE
    BRIDGE --> DESKTOP

    RECORDER --> WIN
    RECORDER --> YAML

    CLI --> YAML
    CLI --> JSON

    DESKTOP --> CACHE

    style AI fill:#e1f5fe
    style MCP fill:#e1f5fe
    style DESKTOP fill:#fff3e0
    style ENGINE fill:#fff3e0
    style WIN fill:#e8f5e9
    style MAC fill:#e8f5e9
    style LINUX fill:#e8f5e9
    style BROWSER fill:#fce4ec
    style RECORDER fill:#fce4ec
```

## Component Descriptions

### AI Integration Layer
- **AI Models**: Claude, GPT, and other LLMs that use Terminator for automation
- **MCP Protocol**: Model Context Protocol server with 50+ automation tools

### Client Layer
- **CLI**: Command-line interface for executing workflows
- **Python SDK**: Native Python bindings using PyO3
- **Node.js SDK**: TypeScript/JavaScript bindings using NAPI-RS
- **Rust SDK**: Direct access to core API

### Core Engine
- **Desktop API**: Main entry point providing unified interface
- **Accessibility Engine**: Platform-agnostic trait system
- **Locator System**: Element finding and filtering
- **Selector Parser**: Parses CSS-like selector syntax

### Platform Layer
- **Windows**: UI Automation API implementation
- **macOS**: Accessibility API implementation
- **Linux**: AT-SPI implementation

### Extensions
- **Browser Extension**: Chrome/Edge extension for DOM access
- **WebSocket Bridge**: Communication channel on port 17373
- **Workflow Recorder**: Captures user interactions (Windows only)

### Storage & Config
- **YAML Workflows**: Human-readable workflow definitions
- **JSON Config**: Configuration and settings
- **Element Cache**: Performance optimization for repeated operations

## Key Architectural Patterns

1. **Trait-Based Abstraction**: Platform differences hidden behind common traits
2. **Async-First Design**: All operations use async/await patterns
3. **Language-Agnostic**: Multiple SDK bindings for different ecosystems
4. **AI-Native**: Built specifically for LLM integration via MCP
5. **Hybrid Approach**: Combines accessibility APIs with browser DOM access