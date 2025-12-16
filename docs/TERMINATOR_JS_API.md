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
  WindowManager,
  // Enums
  ClickType,
  VisionType,
  TreeOutputFormat,
  ElementSource,
  OverlayDisplayMode,
  PropertyLoadingMode,
  TextPosition,
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

## Breaking Changes (v2.0+)

> **Important:** These changes align the SDK with MCP tool behavior.

1. **`Element.click()` is now async** - Returns `Promise<ClickResult>` instead of `ClickResult`
2. **`Locator.first(timeoutMs)` requires timeout** - Timeout in milliseconds is now mandatory
3. **`Locator.all(timeoutMs)` requires timeout** - Timeout in milliseconds is now mandatory
4. **`typeText()` uses `TypeTextOptions`** - New options object with required `clearBeforeTyping` field

```javascript
// Before (deprecated)
element.click();
await locator.first();
element.typeText('hello');

// After (current)
await element.click();
await locator.first(5000);  // 5 second timeout
element.typeText('hello', { clearBeforeTyping: true });
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
- `openApplication(name: string, includeWindowScreenshot?, includeMonitorScreenshots?): Element` - Open application
- `activateApplication(name: string): void` - Activate/focus application
- `windowsForApplication(name: string): Promise<Array<Element>>` - Get all windows for an app
- `getAllApplicationsTree(): Promise<Array<UINode>>` - Get UI trees for all apps

### Window & Tree Management
- `getCurrentWindow(): Promise<Element>` - Get currently focused window
- `getCurrentApplication(): Promise<Element>` - Get currently focused application
- `getCurrentBrowserWindow(): Promise<Element>` - Get current browser window
- `activateBrowserWindowByTitle(title: string): void` - Activate browser by title
- `getWindowTree(process: string, title?: string, config?: TreeBuildConfig): UINode` - Get UI tree for window
- `getWindowTreeResult(process: string, title?: string, config?: TreeBuildConfig): WindowTreeResult` - **Recommended:** Get full result with formatted output and `indexToBounds` mapping
- `getWindowTreeResultAsync(process: string, title?: string, config?: TreeBuildConfig): Promise<WindowTreeResult>` - Async version supporting `treeFromSelector`

### Element Location
- `locator(selector: string | Selector): Locator` - Create element locator
  - **Important:** `desktop.locator()` searches ALL windows/applications
  - For window-specific searches, use `element.locator()` on a window element
- `locatorForProcess(process: string, selector: string | Selector, windowSelector?: string): Locator` - **Recommended:** Create process-scoped locator
- `focusedElement(): Element` - Get currently focused element

### Index-Based Click Targeting
After calling `getWindowTreeResult()` or vision methods, use these to click by index:
- `clickByIndex(index: number, visionType?: VisionType, xPercentage?, yPercentage?, clickType?): ClickResult` - Click element by its index from tree/vision output
- `clickAtBounds(x, y, width, height, xPercentage?, yPercentage?, clickType?): ClickResult` - Click within specific bounds

### Browser & File Operations
- `openUrl(url: string, browser?: string, includeWindowScreenshot?, includeMonitorScreenshots?): Element` - Open URL in browser
  - Browser options: "Default", "Chrome", "Firefox", "Edge", "Brave", "Opera", "Vivaldi", or custom path
- `navigateBrowser(url: string, browser?: string, includeWindowScreenshot?, includeMonitorScreenshots?): Element` - **Recommended:** Navigate browser reliably
- `openFile(filePath: string): void` - Open file with default app
- `executeBrowserScript(script: string): Promise<string>` - Execute JavaScript in current browser tab

### Command Execution
- `runCommand(windowsCommand?: string, unixCommand?: string): Promise<CommandOutput>` - Run shell command
- `run(command: string, shell?: string, workingDirectory?: string): Promise<CommandOutput>` - GitHub Actions-style command
- `delay(delayMs: number): Promise<void>` - Delay execution for specified milliseconds

### OCR (Optical Character Recognition)
- `ocrImagePath(imagePath: string): Promise<string>` - OCR on image file (returns text)
- `ocrScreenshot(screenshot: ScreenshotResult): Promise<string>` - OCR on screenshot (returns text)
- `performOcrForProcess(process: string, formatOutput?: boolean): Promise<OcrResult>` - **Recommended:** OCR with structured results and `indexToBounds` for click targeting

### Vision AI Detection
- `captureBrowserDom(maxElements?: number, formatOutput?: boolean): Promise<BrowserDomResult>` - Capture DOM elements with bounds
- `getClusteredTree(process: string, maxDomElements?, includeOmniparser?, includeGeminiVision?): Promise<ClusteredFormattingResult>` - Combine multiple sources (UIA, DOM, OCR, etc.) with spatial clustering
- `performGeminiVisionForProcess(process: string, formatOutput?: boolean): Promise<GeminiVisionResult>` - Gemini AI vision detection
- `performOmniparserForProcess(process: string, imgsz?: number, formatOutput?: boolean): Promise<OmniparserResult>` - Omniparser icon/field detection

### Monitor/Display Management
- `listMonitors(): Promise<Array<Monitor>>` - List all monitors
- `getPrimaryMonitor(): Promise<Monitor>` - Get primary monitor
- `getActiveMonitor(): Promise<Monitor>` - Get monitor with focused window
- `getMonitorById(id: string): Promise<Monitor>` - Get monitor by ID
- `getMonitorByName(name: string): Promise<Monitor>` - Get monitor by name
- `captureMonitor(monitor: Monitor): Promise<ScreenshotResult>` - Capture specific monitor
- `captureAllMonitors(): Promise<Array<MonitorScreenshotPair>>` - Capture all monitors
- `captureWindowByProcess(process: string): ScreenshotResult` - Capture window by process name

### Screenshot Utilities
- `screenshotToPng(screenshot: ScreenshotResult): Array<number>` - Convert to PNG bytes
- `screenshotToPngResized(screenshot: ScreenshotResult, maxDimension?: number): Array<number>` - Convert with resize
- `screenshotToBase64Png(screenshot: ScreenshotResult): string` - Convert to base64 PNG
- `screenshotToBase64PngResized(screenshot: ScreenshotResult, maxDimension?: number): string` - Convert to base64 with resize
- `screenshotResizedDimensions(screenshot: ScreenshotResult, maxDimension: number): ResizedDimensions` - Get dimensions after resize

### Global Input
- `pressKey(key: string): Promise<void>` - Press key globally (e.g., "Enter", "Ctrl+C", "F1")
- `setZoom(percentage: number): Promise<void>` - Set zoom percentage

### Verification (Post-Action Checks)
- `verifyElementExists(scopeElement: Element, selector: string, timeoutMs?: number): Promise<Element>` - Verify element appeared after action
- `verifyElementNotExists(scopeElement: Element, selector: string, timeoutMs?: number): Promise<void>` - Verify element disappeared after action

### Visual Debugging
- `showInspectOverlay(elements: InspectElement[], windowBounds: Bounds, displayMode?: OverlayDisplayMode): void` - Show indexed overlay
- `hideInspectOverlay(): void` - Hide overlay
- `stopHighlighting(): number` - Stop all active highlights

### Agentic Automation
- `geminiComputerUse(process: string, goal: string, maxSteps?: number, onStep?: callback): Promise<ComputerUseResult>` - Run Gemini computer use agent

### Execution Control
- `stopExecution(): void` - Cancel all running operations
- `isCancelled(): boolean` - Check if execution was cancelled

## Element Class

Represents a UI element in the accessibility tree.

### Properties & Attributes
- `id(): string | null` - Get element ID
- `role(): string` - Get element role (e.g., "button", "textfield")
- `name(): string | null` - Get element name
- `attributes(): UIElementAttributes` - Get all attributes
- `bounds(): Bounds` - Get bounds {x, y, width, height}
- `processId(): number` - Get process ID of containing app
- `processName(): string` - Get process name (e.g., "chrome", "notepad")

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
- `isSelected(): boolean` - Check if selected (list items, tabs)

### Mouse Interactions
All mouse methods accept optional `ActionOptions` for screenshot capture and highlighting.

- `click(options?: ActionOptions): Promise<ClickResult>` - **Async** click element
- `doubleClick(options?: ActionOptions): ClickResult` - Double click
- `rightClick(options?: ActionOptions): void` - Right click
- `hover(options?: ActionOptions): void` - Hover over element
- `mouseDrag(startX, startY, endX, endY): void` - Drag from start to end
- `mouseClickAndHold(x, y): void` - Press and hold at coordinates
- `mouseMove(x, y): void` - Move mouse to coordinates
- `mouseRelease(): void` - Release mouse button

### Keyboard Interactions
- `focus(): void` - Focus element
- `typeText(text: string, options?: TypeTextOptions): ActionResult` - Type text with options
- `pressKey(key: string, options?: ActionOptions): ActionResult` - Press key while focused
- `setValue(value: string, options?: ActionOptions): ActionResult` - Set element value directly

### Text & Value Operations
- `text(maxDepth?: number): string` - Get text content
- `getValue(): string | null` - Get value attribute

### Control Operations
- `performAction(action: string): void` - Perform named action
- `invoke(options?: ActionOptions): ActionResult` - Trigger default action (more reliable than click for some controls)
- `setToggled(state: boolean): void` - Set toggle state
- `setSelected(state: boolean, options?: ActionOptions): void` - Set selection state

### Dropdown/List Operations
- `selectOption(optionName: string, options?: ActionOptions): void` - Select dropdown option
- `listOptions(): Array<string>` - List available options

### Range Controls (Sliders)
- `getRangeValue(): number` - Get current slider/progress value
- `setRangeValue(value: number): void` - Set slider value

### Scrolling
- `scroll(direction: string, amount: number, options?: ActionOptions): ActionResult` - Scroll element
- `scrollIntoView(): void` - Scroll element into view within its window

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

### Options Types for Element Methods

```typescript
interface ActionOptions {
  highlightBeforeAction?: boolean    // Highlight element before action (default: false)
  includeWindowScreenshot?: boolean  // Capture window screenshot after (default: true)
  includeMonitorScreenshots?: boolean // Capture all monitors after (default: false)
  tryFocusBefore?: boolean           // Try focusing before action (default: true)
  tryClickBefore?: boolean           // Try clicking if focus fails (default: true)
}

interface TypeTextOptions extends ActionOptions {
  clearBeforeTyping: boolean  // REQUIRED: Clear existing text first
  useClipboard?: boolean      // Use clipboard paste (default: false)
}
```

## Locator Class

For finding UI elements by selector.

### Methods

**IMPORTANT:** `first()` and `all()` now require a timeout parameter:

- `first(timeoutMs: number): Promise<Element>` - Get first matching element (timeout REQUIRED)
- `all(timeoutMs: number, depth?: number): Promise<Array<Element>>` - Get all matches (timeout REQUIRED)
- `validate(timeoutMs: number): Promise<ValidationResult>` - Check element existence without throwing
- `waitFor(condition: string, timeoutMs: number): Promise<Element>` - Wait for condition ('exists', 'visible', 'enabled', 'focused')
- `timeout(timeoutMs: number): Locator` - Set default timeout for chained calls
- `within(element: Element): Locator` - Scope search to element subtree
- `locator(selector: string | Selector): Locator` - Chain another selector

### ValidationResult Type

```typescript
interface ValidationResult {
  exists: boolean        // Whether element was found
  element?: Element      // The element if found
  error?: string         // Error message if validation failed (not for "not found")
}
```

### Example Usage

```typescript
// Required timeout - use 0 for immediate search
const button = await desktop.locator('role:Button|name:Submit').first(5000);

// Validate without throwing
const result = await desktop.locator('role:Dialog').validate(1000);
if (result.exists) {
  await result.element!.click();
}

// Wait for specific condition
const input = await desktop.locator('role:Edit').waitFor('enabled', 10000);
```

## Selector Class

Typed selector API (alternative to string selectors).

### Static Factory Methods

**Scoping (NEW):**
- `Selector.process(processName: string): Selector` - Scope search to specific process
- `Selector.window(title: string): Selector` - Scope to window within process

**Element Matching:**
- `Selector.name(name: string): Selector` - Match by name
- `Selector.role(role: string, name?: string): Selector` - Match by role
- `Selector.id(id: string): Selector` - Match by ID
- `Selector.text(text: string): Selector` - Match by text content
- `Selector.path(path: string): Selector` - XPath-like path
- `Selector.nativeId(id: string): Selector` - Native automation ID
- `Selector.className(name: string): Selector` - Match by class
- `Selector.attributes(attributes: Record<string, string>): Selector` - Match by attributes

**Navigation & Filtering:**
- `Selector.nth(index: number): Selector` - Select nth element (0-based, negative from end)
- `Selector.has(innerSelector: Selector): Selector` - Has descendant matching selector
- `Selector.parent(): Selector` - Navigate to parent

### Instance Methods
- `chain(other: Selector): Selector` - Chain another selector
- `visible(isVisible: boolean): Selector` - Filter by visibility

### Example Usage

```typescript
// Scope to process and window
const selector = Selector.process("notepad")
  .chain(Selector.window("Untitled"))
  .chain(Selector.role("Edit"));

// Find element using typed selector
const editor = await desktop.locator(selector).first(5000);
```

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

## Enums

### VisionType
Detection source for index-based clicking:
```typescript
enum VisionType {
  UiTree = 'UiTree',      // Standard UI automation tree
  Ocr = 'Ocr',            // Optical character recognition
  Omniparser = 'Omniparser', // Icon/UI field detection
  Gemini = 'Gemini',      // Gemini vision model
  Dom = 'Dom'             // Browser DOM elements
}
```

### TreeOutputFormat
Format for UI tree output:
```typescript
enum TreeOutputFormat {
  VerboseJson = 'verbose_json',  // Full JSON with all fields
  CompactYaml = 'compact_yaml',  // Minimal: [ROLE] name #id
  ClusteredYaml = 'clustered_yaml' // Spatial clustering with prefixed indices (#u1, #d2, #o3)
}
```

### TextPosition
Position for text overlay on highlights:
```typescript
enum TextPosition {
  Top = 'Top',
  TopRight = 'TopRight',
  Right = 'Right',
  BottomRight = 'BottomRight',
  Bottom = 'Bottom',
  BottomLeft = 'BottomLeft',
  Left = 'Left',
  TopLeft = 'TopLeft',
  Inside = 'Inside'
}
```

### PropertyLoadingMode
Performance mode for tree building:
```typescript
enum PropertyLoadingMode {
  Fast = 'Fast',       // Skip expensive properties
  Complete = 'Complete', // Load all properties
  Smart = 'Smart'      // Adaptive based on depth
}
```

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

### Coordinates
```typescript
interface Coordinates {
  x: number
  y: number
}
```

### ClickResult
```typescript
interface ClickResult {
  method: string              // Click method used
  coordinates?: Coordinates   // Click coordinates
  details: string             // Additional details
  windowScreenshotPath?: string      // Path to captured screenshot
  monitorScreenshotPaths?: Array<string>  // Monitor screenshot paths
  uiDiff?: UiDiffResult       // UI diff if enabled
}
```

### ActionResult
```typescript
interface ActionResult {
  success: boolean            // Whether action succeeded
  windowScreenshotPath?: string      // Path to captured screenshot
  monitorScreenshotPaths?: Array<string>  // Monitor screenshot paths
  uiDiff?: UiDiffResult       // UI diff if enabled
}
```

### UiDiffResult
```typescript
interface UiDiffResult {
  diff: string               // Diff showing changes (+ or - lines)
  treeBefore?: string        // Full tree before action (if include_full_trees)
  treeAfter?: string         // Full tree after action (if include_full_trees)
  hasChanges: boolean        // Whether any UI changes detected
}
```

### WindowInfo
```typescript
interface WindowInfo {
  hwnd: number              // Window handle
  processName: string       // Process name (e.g., "notepad.exe")
  processId: number         // Process ID
  zOrder: number            // Z-order position (0 = topmost)
  isMinimized: boolean      // Whether window is minimized
  isMaximized: boolean      // Whether window is maximized
  isAlwaysOnTop: boolean    // Whether has WS_EX_TOPMOST style
  title: string             // Window title
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

## WindowManager Class

Controls window states with z-order tracking. Useful for workflows that need to manage multiple windows.

### Constructor
```typescript
const wm = new WindowManager();
```

### Methods

**Cache Management:**
- `updateWindowCache(): Promise<void>` - Refresh window information cache

**Window Queries:**
- `getTopmostWindowForProcess(process: string): Promise<WindowInfo | null>` - Get top window by process name
- `getTopmostWindowForPid(pid: number): Promise<WindowInfo | null>` - Get top window by PID
- `getAlwaysOnTopWindows(): Promise<Array<WindowInfo>>` - Get all always-on-top windows

**Window Control:**
- `bringWindowToFront(hwnd: number): Promise<boolean>` - Bring to front (bypasses Windows focus prevention)
- `minimizeIfNeeded(hwnd: number): Promise<boolean>` - Minimize if not already
- `maximizeIfNeeded(hwnd: number): Promise<boolean>` - Maximize if not already
- `minimizeAlwaysOnTopWindows(targetHwnd: number): Promise<number>` - Minimize always-on-top windows (except target)
- `minimizeAllExcept(targetHwnd: number): Promise<number>` - Minimize all except target

**State Management:**
- `captureInitialState(): Promise<void>` - Snapshot window states before workflow
- `restoreAllWindows(): Promise<number>` - Restore windows to original state
- `clearCapturedState(): Promise<void>` - Clear captured state
- `setTargetWindow(hwnd: number): Promise<void>` - Track target for restoration

**Utilities:**
- `isUwpApp(pid: number): Promise<boolean>` - Check if process is UWP/Modern app

### Example Usage
```typescript
const { WindowManager } = require('terminator.js');

const wm = new WindowManager();

// Get topmost Chrome window
await wm.updateWindowCache();
const chromeWindow = await wm.getTopmostWindowForProcess('chrome');
if (chromeWindow) {
  console.log(`Chrome: ${chromeWindow.title} (hwnd: ${chromeWindow.hwnd})`);

  // Bring to front
  await wm.bringWindowToFront(chromeWindow.hwnd);
}

// Workflow pattern: capture state, do work, restore
await wm.captureInitialState();
try {
  // ... perform automation
} finally {
  await wm.restoreAllWindows();
}
```

## Usage Examples

### Basic Automation
```javascript
const { Desktop } = require('terminator.js');

const desktop = new Desktop();

// Find and click a button (timeout required)
const buttonLocator = desktop.locator('role:button|Save');
const button = await buttonLocator.first(5000);  // 5 second timeout
await button.click();

// Type into a text field
const inputLocator = desktop.locator('role:textfield');
const input = await inputLocator.first(5000);
await input.typeText('Hello World', { clearBeforeTyping: true });

// Open a URL
desktop.openUrl('https://example.com', 'Chrome');
```

### Checking Optional Elements
```javascript
// ✅ RECOMMENDED: Use validate() for optional elements (doesn't throw)
const result = await desktop.locator('role:Dialog|name:Confirm').validate(1000);
if (result.exists) {
  const leaveButton = await result.element.locator('role:Button|name:Leave').first(1000);
  await leaveButton.click();
}

// Alternative: Window-scoped try/catch
try {
  const chromeWindow = await desktop.locator('role:Window && name:Chrome').first(2000);
  const leaveButton = await chromeWindow.locator('role:Button && name:Leave').first(1000);
  await leaveButton.click();
  console.log('Dialog found and handled');
} catch (e) {
  console.log('No dialog present, continuing');
}

// Performance note: validate() and .first() with try/catch are ~8x faster than .all()
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
  const element = await locator.first(5000);  // Timeout required
} catch (error) {
  if (error instanceof ElementNotFoundError) {
    console.log('Button not found');
  }
}

// Better approach: use validate() to avoid exceptions
const result = await desktop.locator('role:button|NonExistent').validate(1000);
if (!result.exists) {
  console.log('Button not found');
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