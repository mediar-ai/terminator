# Java Access Bridge Support for Terminator

This document describes the Java Access Bridge (JAB) implementation in Terminator, which enables automation of Java applications on Windows.

## Overview

Java Access Bridge is a Windows-specific technology developed by Oracle that exposes the Java Accessibility API through a Windows DLL. This allows assistive technologies and automation tools to interact with Java applications (Swing, AWT, etc.) running on Windows systems.

## Features

### ✅ Implemented Features

- **Java Application Detection**: Automatically detect if a window belongs to a Java application
- **Element Discovery**: Find and interact with Java UI elements (buttons, text fields, etc.)
- **Standard Operations**: Click, type text, get properties, navigate element hierarchy
- **Selector Support**: Use `javaapp` selectors to target Java applications specifically
- **Memory Management**: Proper cleanup of Java objects to prevent memory leaks
- **Thread Safety**: Safe to use from multiple threads
- **Event Handling**: Support for focus events (extensible for other events)

### 🔧 Architecture

The implementation consists of several key components:

1. **`java_access_bridge.rs`**: Core JAB API bindings and wrapper
2. **`java_access_bridge_element.rs`**: UIElement implementation for Java elements
3. **Windows Engine Integration**: Seamless integration with existing Windows automation
4. **Selector Extensions**: New `javaapp` selector type for targeting Java apps

## Prerequisites

### System Requirements

- **Operating System**: Windows (any version with Java runtime)
- **Java Runtime**: JRE/JDK installed
- **Java Access Bridge**: Enabled (see setup below)

### Setup Instructions

1. **Enable Java Access Bridge**:
   ```cmd
   # Run as Administrator
   jabswitch -enable
   ```

2. **Restart Java Applications**: Any running Java applications must be restarted after enabling JAB

3. **Verify Installation**: Check for JAB DLLs:
   - `C:\Windows\System32\WindowsAccessBridge-64.dll`
   - `C:\Windows\System32\WindowsAccessBridge-32.dll`
   - `C:\Windows\SysWOW64\WindowsAccessBridge-32.dll`

## Usage Examples

### Basic Java Application Automation

```rust
use terminator::{Desktop, Selector};

// Create desktop automation instance
let desktop = Desktop::new()?;

// Find any Java application
let java_app = desktop.locator(Selector::from("javaapp")).find_element()?;

// Find specific Java application by name
let eclipse = desktop.locator(Selector::from("javaapp:Eclipse")).find_element()?;

// Find Java elements within the application
let button = java_app.locator(Selector::from("role:button")).find_element()?;
let text_field = java_app.locator(Selector::from("role:text")).find_element()?;

// Interact with Java elements
button.click()?;
text_field.type_text("Hello from Terminator!")?;
```

### Advanced Selector Chaining

```rust
// Chain selectors to find specific elements
let ok_button = desktop
    .locator(Selector::from("javaapp:MyApp >> role:button >> name:OK"))
    .find_element()?;

// Find elements by various properties
let text_fields = java_app
    .locator(Selector::from("role:text"))
    .find_elements()?;

// Use accessibility properties
let enabled_buttons = java_app
    .locator(Selector::from("role:button"))
    .find_elements()?
    .into_iter()
    .filter(|btn| btn.is_enabled().unwrap_or(false))
    .collect::<Vec<_>>();
```

### Element Inspection

```rust
// Get detailed information about Java elements
let element = java_app.locator(Selector::from("role:button")).find_element()?;

println!("Name: {}", element.get_name()?);
println!("Role: {}", element.get_role()?);
println!("Value: {}", element.get_value()?);
println!("Enabled: {}", element.is_enabled()?);
println!("Visible: {}", element.is_visible()?);

// Get bounding rectangle
let (x, y, width, height) = element.get_bounding_rectangle()?;
println!("Bounds: ({}, {}) {}x{}", x, y, width, height);

// Get all attributes (including JAB-specific ones)
let attributes = element.get_all_attributes()?;
for (key, value) in attributes.attributes {
    println!("{}: {}", key, value);
}
```

## Supported Java UI Elements

The implementation supports all standard Java accessibility roles:

- **Buttons**: `role:button`
- **Text Fields**: `role:text`, `role:textfield`
- **Labels**: `role:label`
- **Menus**: `role:menu`, `role:menuitem`, `role:menubar`
- **Lists**: `role:list`, `role:listitem`
- **Trees**: `role:tree`, `role:treeitem`
- **Tables**: `role:table`, `role:cell`
- **Panels**: `role:panel`
- **Windows**: `role:window`, `role:dialog`
- **And many more...**

## Selector Reference

### Java Application Selectors

| Selector | Description | Example |
|----------|-------------|---------|
| `javaapp` | Any Java application | `desktop.locator("javaapp")` |
| `javaapp:AppName` | Specific Java app by name | `desktop.locator("javaapp:Eclipse")` |

### Chaining with Standard Selectors

```rust
// Examples of chaining Java selectors with standard selectors
"javaapp >> role:button"                    // Any button in any Java app
"javaapp:Eclipse >> role:menu >> name:File" // File menu in Eclipse
"javaapp >> role:text >> visible:true"     // Visible text fields in Java apps
```

## Integration with Windows Engine

The Java Access Bridge integration is seamlessly built into the Windows accessibility engine:

```rust
// The Windows engine automatically detects Java applications
let engine = WindowsEngine::new(false, false)?;

// Check if a window is a Java application
let is_java = engine.is_java_window(hwnd);

// Try to create a Java element from a window handle
let java_element = engine.try_create_java_element(hwnd);
```

## Error Handling

The implementation provides comprehensive error handling:

```rust
use terminator::AutomationError;

match desktop.locator("javaapp").find_element() {
    Ok(java_app) => {
        // Successfully found Java application
        println!("Found Java app: {}", java_app.get_name()?);
    }
    Err(AutomationError::ElementNotFound(_)) => {
        println!("No Java applications found");
    }
    Err(AutomationError::PlatformError(msg)) => {
        println!("Java Access Bridge error: {}", msg);
    }
    Err(e) => {
        println!("Other error: {}", e);
    }
}
```

## Memory Management

The implementation automatically handles Java object lifecycle:

- Java objects are automatically released when elements are dropped
- Reference counting prevents premature garbage collection
- Thread-safe access to the Java Access Bridge instance
- Proper cleanup on application shutdown

## Performance Considerations

- **Lazy Loading**: Java Access Bridge is only loaded when needed
- **Caching**: Element information is cached to reduce API calls
- **Efficient Searches**: Uses JAB's built-in search capabilities
- **Background Availability**: Gracefully degrades when JAB is not available

## Testing

The implementation includes comprehensive tests:

```bash
# Run basic tests (don't require Java apps)
cargo test java_access_bridge

# Run integration tests (require Java apps to be running)
cargo test java_access_bridge -- --ignored

# Test specific functionality
cargo test test_java_app_selector_parsing
cargo test test_wide_string_conversion
```

## Troubleshooting

### Common Issues

1. **"Java Access Bridge not available"**
   - Solution: Run `jabswitch -enable` as Administrator
   - Restart Java applications after enabling

2. **"No Java applications found"**
   - Ensure Java apps are running with accessibility enabled
   - Some Java apps may need specific JVM flags: `-Djava.awt.headless=false`

3. **"Failed to create Java Access Bridge element"**
   - Check that the window is actually a Java application
   - Verify the Java application has accessible elements

4. **Performance Issues**
   - Use more specific selectors to reduce search scope
   - Cache frequently accessed elements
   - Consider using batch operations

### Debugging Tips

```rust
// Enable detailed logging
env_logger::init();

// Check if Java Access Bridge is available
if !is_java_access_bridge_available() {
    println!("Java Access Bridge is not available");
}

// List all accessible elements in a Java app
let java_app = desktop.locator("javaapp").find_element()?;
let all_elements = java_app.locator("*").find_elements()?;
for element in all_elements {
    println!("Element: {} ({})", 
        element.get_name().unwrap_or_default(),
        element.get_role().unwrap_or_default()
    );
}
```

## Limitations

- **Windows Only**: Java Access Bridge is Windows-specific
- **Java Applications Only**: Only works with Java Swing/AWT applications
- **JAB Dependency**: Requires Java Access Bridge to be installed and enabled
- **Application Restart**: Java apps must be restarted after enabling JAB
- **Limited Pattern Support**: Some advanced accessibility patterns not yet implemented

## Future Enhancements

Potential improvements for future versions:

- **More Accessibility Patterns**: Expand/collapse, selection, scrolling patterns
- **Event Subscription**: More comprehensive event handling
- **Performance Optimization**: Async operations, batch queries
- **Java Version Detection**: Automatic Java runtime detection
- **Enhanced Debugging**: Built-in Java application inspector

## Examples

See the complete example at `examples/java_access_bridge_example.rs` for a full demonstration of Java Access Bridge capabilities.

## API Reference

### Core Types

- `JavaAccessBridge`: Main JAB API wrapper
- `JavaAccessBridgeElement`: UIElement implementation for Java elements
- `AccessibleContext`: Handle to Java accessible objects
- `VmId`: Java Virtual Machine identifier

### Key Functions

- `is_java_access_bridge_available()`: Check JAB availability
- `JavaAccessBridge::get_instance()`: Get global JAB instance
- `JavaAccessBridgeElement::from_hwnd()`: Create element from window handle
- `wide_string_to_string()`: Convert JAB wide strings to Rust strings

This implementation provides a solid foundation for automating Java applications on Windows through the industry-standard Java Access Bridge technology.