# Terminator Architecture Diagrams

This directory contains architectural and flow diagrams for the Terminator desktop automation framework. These diagrams are designed to help understand the system architecture, data flows, and component interactions.

## Diagrams

### 1. [System Architecture](01-system-architecture.md)
Complete overview of Terminator's multi-layered architecture, showing how AI models, SDKs, core engine, and platform layers interact.

### 2. [MCP Integration Flow](02-mcp-integration-flow.md)
Details how AI models interact with Terminator through the Model Context Protocol, including the 50+ available tools and request/response patterns.

### 3. [Element Selection System](03-element-selection-system.md)
Illustrates the sophisticated CSS-like selector syntax, parsing pipeline, and tree traversal algorithms for finding UI elements.

### 4. [Workflow Execution Pipeline](04-workflow-execution-pipeline.md)
Shows the complete pipeline from YAML/JSON workflow definitions through variable substitution, execution, and output processing.

### 5. [Browser Extension Architecture](05-browser-extension-architecture.md)
Explains how the Chrome/Edge browser extension provides DOM-level access beyond what accessibility APIs can see.

### 6. [Cross-Platform Abstraction](06-cross-platform-abstraction.md)
Demonstrates how platform-specific APIs (Windows UI Automation, macOS Accessibility, Linux AT-SPI) are abstracted into a unified interface.

### 7. [Component Communication](07-component-communication.md)
Maps out how different components communicate through various protocols (MCP, WebSocket, stdio) and data formats.

### 8. [Recording & Playback Flow](08-recording-playback-flow.md)
Illustrates how human interactions are recorded, processed, optimized, and converted into reusable automation workflows.

## Viewing the Diagrams

All diagrams use Mermaid syntax and can be viewed:
- Directly on GitHub (automatic rendering)
- In VS Code with a Mermaid preview extension
- In any Markdown viewer that supports Mermaid
- Online at [mermaid.live](https://mermaid.live/)

## Technology Stack

The diagrams cover the following key technologies:
- **Languages**: Rust (core), Python, TypeScript/JavaScript, YAML
- **Protocols**: MCP (Model Context Protocol), WebSocket, JSON-RPC 2.0
- **Platform APIs**: Windows UI Automation, macOS Accessibility, Linux AT-SPI
- **Integration**: Chrome/Edge extensions, AI models (Claude, GPT)

## Use Cases

These diagrams are useful for:
- **Developers**: Understanding the codebase architecture
- **Integrators**: Learning how to integrate Terminator with AI models
- **Contributors**: Finding where to add new features
- **Architects**: Evaluating the system design
- **Presenters**: Explaining Terminator in talks and demos

## Contributing

When adding new diagrams:
1. Use Mermaid syntax for consistency
2. Include both overview and detailed views
3. Add descriptions explaining key concepts
4. Update this README with the new diagram

## Related Documentation

- [Terminator README](../../README.md)
- [MCP Agent Documentation](../../terminator-mcp-agent/README.md)
- [CLI Documentation](../../terminator-cli/README.md)
- [SDK Examples](../../bindings/)