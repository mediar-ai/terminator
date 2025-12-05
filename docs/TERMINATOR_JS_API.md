# Terminator.js Complete API Reference

## Overview
Terminator.js is a Node.js binding for desktop automation through accessibility APIs. It provides programmatic control over UI elements across Windows, macOS, and Linux platforms.

## Installation & Usage

### In run_command with engine mode:
```javascript
{
  "engine": "javascript",
  "run": `
    const { Desktop } = require('terminator.js');
    const desktop = new Desktop();
    // Your automation code here
  `
}
```

### Import available classes:
```javascript
const {
  Desktop,
  Element,
  Locator,
  Selector,
  // Error classes
  ElementNotFoundError,
  TimeoutError,
  PermissionDeniedError,
  PlatformError,
  UnsupportedOperationError,
  UnsupportedPlatformError,
  InvalidArgumentError,
  InternalError
} = require('terminator.js');
```

## Desktop Class

The main entry point for desktop automation.

### Constructor
```javascript
new Desktop(useBackgroundApps?: boolean, activateApp?: boolean, logLevel?: string)
```

### Application Management
- `root(): Element` - Get root UI element of desktop
- `applications(): Array<Element>` - List all running applications
- `application(name: string): Element` - Get application by name
- `openApplication(name: string): Element` - Open application
- `activateApplication(name: string): void` - Activate/focus application
- `windowsForApplication(name: string): Promise<Array<Element>>` - Get all windows for an app
- `getAllApplicationsTree(): Promise<Array<UINode>>` - Get UI trees for all apps

### Window Management
- `getCurrentWindow(): Promise<Element>` - Get currently focused window
- `getCurrentApplication(): Promise<Element>` - Get currently focused application
- `getCurrentBrowserWindow(): Promise<Element>` - Get current browser window
- `activateBrowserWindowByTitle(title: string): void` - Activate browser by title
- `getWindowTree(pid: number, title?: string, config?: TreeBuildConfig): UINode` - Get UI tree for window

### Element Location
- `locator(selector: string | Selector): Locator` - Create element locator
  - **Important:** `desktop.locator()` searches ALL windows/applications
  - For window-specific searches, use `element.locator()` on a window element
- `focusedElement(): Element` - Get currently focused element

### Browser & File Operations
- `openUrl(url: string, browser?: string): Element` - Open URL in browser
  - Browser options: "Default", "Chrome", "Firefox", "Edge", "Brave", "Opera", "Vivaldi", or custom path
- `openFile(filePath: string): void` - Open file with default app

### Command Execution
- `runCommand(windowsCommand?: string, unixCommand?: string): Promise<CommandOutput>` - Run shell command
- `run(command: string, shell?: string, workingDirectory?: string): Promise<CommandOutput>` - GitHub Actions-style command

### OCR (Optical Character Recognition)
- `ocrImagePath(imagePath: string): Promise<string>` - OCR on image file
- `ocrScreenshot(screenshot: ScreenshotResult): Promise<string>` - OCR on screenshot

### Monitor/Display Management
- `listMonitors(): Promise<Array<Monitor>>` - List all monitors
- `getPrimaryMonitor(): Promise<Monitor>` - Get primary monitor
- `getActiveMonitor(): Promise<Monitor>` - Get monitor with focused window
- `getMonitorById(id: string): Promise<Monitor>` - Get monitor by ID
- `getMonitorByName(name: string): Promise<Monitor>` - Get monitor by name
- `captureMonitor(monitor: Monitor): Promise<ScreenshotResult>` - Capture specific monitor
- `captureAllMonitors(): Promise<Array<MonitorScreenshotPair>>` - Capture all monitors

### Global Input
- `pressKey(key: string): Promise<void>` - Press key globally (e.g., "Enter", "Ctrl+C", "F1")
- `setZoom(percentage: number): Promise<void>` - Set zoom percentage

## Element Class

Represents a UI element in the accessibility tree.

### Properties & Attributes
- `id(): string | null` - Get element ID
- `role(): string` - Get element role (e.g., "button", "textfield")
- `name(): string | null` - Get element name
- `attributes(): UIElementAttributes` - Get all attributes
- `bounds(): Bounds` - Get bounds {x, y, width, height}
- `processId(): number` - Get process ID of containing app

### Navigation
- `parent(): Element | null` - Get parent element
- `children(): Array<Element>` - Get child elements
- `application(): Element | null` - Get containing application
- `window(): Element | null` - Get containing window
- `locator(selector: string | Selector): Locator` - Create locator from element
- `monitor(): Monitor` - Get containing monitor

### State Checking
- `isVisible(): boolean` - Check if visible
- `isEnabled(): boolean` - Check if enabled
- `isFocused(): boolean` - Check if focused
- `isKeyboardFocusable(): boolean` - Check if can receive keyboard focus
- `isToggled(): boolean` - Check if toggled (checkboxes, switches)

### Mouse Interactions
- `click(): ClickResult` - Click element
- `doubleClick(): ClickResult` - Double click
- `rightClick(): void` - Right click
- `hover(): void` - Hover over element
- `mouseDrag(startX, startY, endX, endY): void` - Drag from start to end
- `mouseClickAndHold(x, y): void` - Press and hold at coordinates
- `mouseMove(x, y): void` - Move mouse to coordinates
- `mouseRelease(): void` - Release mouse button

### Keyboard Interactions
- `focus(): void` - Focus element
- `typeText(text: string, useClipboard?: boolean): void` - Type text
- `pressKey(key: string): void` - Press key while focused
- `setValue(value: string): void` - Set element value directly

### Text Operations
- `text(maxDepth?: number): string` - Get text content

### Control Operations
- `performAction(action: string): void` - Perform named action
- `invoke(): void` - Trigger default action (more reliable than click for some controls)
- `setToggled(state: boolean): void` - Set toggle state

### Dropdown/List Operations
- `selectOption(optionName: string): void` - Select dropdown option
- `listOptions(): Array<string>` - List available options

### Scrolling
- `scroll(direction: string, amount: number): void` - Scroll element

### Window Operations
- `activateWindow(): void` - Activate containing window
- `minimizeWindow(): void` - Minimize window
- `maximizeWindow(): void` - Maximize window
- `setTransparency(percentage: number): void` - Set window transparency (0-100)
- `close(): void` - Close element (windows/apps)

### Visual Operations
- `highlight(color?: number, durationMs?: number, text?: string, textPosition?: TextPosition, fontStyle?: FontStyle): HighlightHandle` - Highlight with border
- `capture(): ScreenshotResult` - Capture screenshot of element

### Browser Scripting
- `executeBrowserScript(script: string): Promise<string>` - Execute JavaScript in browser

### Tree Extraction
- `getTree(maxDepth?: number): UINode` - Get UI tree starting from this element (default depth: 100)

## Locator Class

For finding UI elements by selector.

### Methods
- `first(): Promise<Element>` - Get first matching element
- `all(timeoutMs?: number, depth?: number): Promise<Array<Element>>` - Get all matches
- `wait(timeoutMs?: number): Promise<Element>` - Wait for element
- `timeout(timeoutMs: number): Locator` - Set default timeout
- `within(element: Element): Locator` - Set root element
- `locator(selector: string | Selector): Locator` - Chain selector

## Selector Class

Typed selector API (alternative to string selectors).

### Static Factory Methods
- `Selector.name(name: string): Selector` - Match by name
- `Selector.role(role: string, name?: string): Selector` - Match by role
- `Selector.id(id: string): Selector` - Match by ID
- `Selector.text(text: string): Selector` - Match by text content
- `Selector.path(path: string): Selector` - XPath-like path
- `Selector.nativeId(id: string): Selector` - Native automation ID
- `Selector.className(name: string): Selector` - Match by class
- `Selector.attributes(attributes: Record<string, string>): Selector` - Match by attributes
- `Selector.nth(index: number): Selector` - Select nth element (0-based, negative from end)
- `Selector.has(innerSelector: Selector): Selector` - Has descendant matching selector
- `Selector.parent(): Selector` - Navigate to parent

### Instance Methods
- `chain(other: Selector): Selector` - Chain another selector
- `visible(isVisible: boolean): Selector` - Filter by visibility

## Selector String Syntax

String-based selectors support these patterns:
- `role:button` - Match by role
- `name:Save` - Match by name
- `role:button|Save` - Role with name
- `text:Submit` - Match by text content
- `id:submit-btn` - Match by ID
- Multiple criteria: `role:button name:Save`

## Error Classes

Custom error types for better error handling:
- `ElementNotFoundError` - Element not found
- `TimeoutError` - Operation timed out
- `PermissionDeniedError` - Permission denied
- `PlatformError` - Platform-specific error
- `UnsupportedOperationError` - Operation not supported
- `UnsupportedPlatformError` - Platform not supported
- `InvalidArgumentError` - Invalid argument
- `InternalError` - Internal error

## Data Types

### Bounds
```typescript
interface Bounds {
  x: number
  y: number
  width: number
  height: number
}
```

### Monitor
```typescript
interface Monitor {
  id: string
  name: string
  isPrimary: boolean
  width: number
  height: number
  x: number
  y: number
  scaleFactor: number
}
```

### ScreenshotResult
```typescript
interface ScreenshotResult {
  width: number
  height: number
  imageData: Array<number>
  monitor?: Monitor
}
```

### CommandOutput
```typescript
interface CommandOutput {
  exitStatus?: number
  stdout: string
  stderr: string
}
```

### TreeBuildConfig
```typescript
interface TreeBuildConfig {
  propertyMode: PropertyLoadingMode  // Fast | Complete | Smart
  timeoutPerOperationMs?: number      // Timeout per operation in ms
  yieldEveryNElements?: number        // Yield frequency for responsiveness
  batchSize?: number                  // Batch size for processing
  maxDepth?: number                   // Maximum tree depth (undefined = unlimited)
}
```

### UINode
```typescript
interface UINode {
  id?: string
  attributes: UIElementAttributes
  children: Array<UINode>  // Recursive structure
}
```

### UIElementAttributes
```typescript
interface UIElementAttributes {
  role: string
  name?: string
  label?: string
  value?: string
  description?: string
  properties: Record<string, string>
  isKeyboardFocusable?: boolean
  bounds?: Bounds
}
```

## Usage Examples

### Basic Automation
```javascript
const { Desktop } = require('terminator.js');

const desktop = new Desktop();

// Find and click a button
const buttonLocator = desktop.locator('role:button|Save');
const button = await buttonLocator.first();
await button.click();

// Type into a text field
const inputLocator = desktop.locator('role:textfield');
const input = await inputLocator.first();
input.typeText('Hello World');

// Open a URL
desktop.openUrl('https://example.com', 'Chrome');
```

### Checking Optional Elements
```javascript
// ✅ RECOMMENDED: Window-scoped search (most accurate)
// Check if optional dialog/button exists in specific window
try {
  // Scope to specific window first to avoid false positives
  const chromeWindow = await desktop.locator('role:Window && name:Chrome').first();
  // Then search within that window
  const leaveButton = await chromeWindow.locator('role:Button && name:Leave').first();

  console.log('Dialog found, clicking Leave button');
  await leaveButton.click();

  return JSON.stringify({
    dialog_exists: 'true'
  });
} catch (e) {
  console.log('No dialog present, continuing');
  return JSON.stringify({
    dialog_exists: 'false'
  });
}

// Alternative: Desktop-wide search (when element could be anywhere)
try {
  await desktop.locator('role:Button && name:Leave').first();
  // Element exists
} catch (e) {
  // Element doesn't exist
}

// Performance note: .first() with try/catch is ~8x faster than .all()
```

### Window Management
```javascript
// Get current window
const window = await desktop.getCurrentWindow();
console.log('Window:', window.name());

// Minimize/maximize
window.minimizeWindow();
window.maximizeWindow();

// Set transparency
window.setTransparency(80); // 80% opaque
```

### Error Handling
```javascript
const { Desktop, ElementNotFoundError } = require('terminator.js');

try {
  const locator = desktop.locator('role:button|NonExistent');
  const element = await locator.first();
} catch (error) {
  if (error instanceof ElementNotFoundError) {
    console.log('Button not found');
  }
}
```

### Browser Automation
```javascript
// Execute JavaScript in browser
const browser = await desktop.getCurrentBrowserWindow();
const result = await browser.executeBrowserScript(`
  document.title
`);
console.log('Page title:', result);
```

### OCR
```javascript
// OCR on screenshot
const screenshot = await desktop.captureMonitor(await desktop.getPrimaryMonitor());
const text = await desktop.ocrScreenshot(screenshot);
console.log('Extracted text:', text);
```

### UI Tree Inspection
```javascript
// Get tree for specific window
const app = desktop.application('Google Chrome');
const pid = app.processId();

// With performance tuning
const tree = desktop.getWindowTree(pid, 'New Tab', {
  propertyMode: PropertyLoadingMode.Fast,
  timeoutPerOperationMs: 50,
  maxDepth: 5  // Limit depth
});

// Traverse recursively
function printTree(node, depth = 0) {
  const indent = '  '.repeat(depth);
  console.log(`${indent}${node.attributes.role}: ${node.attributes.name || '(no name)'}`);
  for (const child of node.children) {
    printTree(child, depth + 1);
  }
}
printTree(tree);

// Get all app trees (expensive)
const allTrees = await desktop.getAllApplicationsTree();
console.log(`Found ${allTrees.length} applications`);

// Get subtree from specific element
const dialog = await desktop.locator('role:Dialog && name:Settings').first(5000);
const dialogTree = dialog.getTree(3);  // Limit to 3 levels deep
console.log(`Dialog has ${dialogTree.children.length} immediate children`);
```

## Platform Notes

- **Windows**: Uses UI Automation API
- **macOS**: Uses Accessibility API (requires permissions)
- **Linux**: Uses AT-SPI2

The native bindings are platform-specific (.node files) loaded automatically based on the platform.