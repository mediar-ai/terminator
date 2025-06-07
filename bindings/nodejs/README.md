# terminator.js

Node.js/TypeScript bindings for the Terminator Rust library - AI-native GUI automation for Windows, macOS, and Linux.

## Installation

```bash
npm install terminator.js
# or
bun install terminator.js
# or
yarn add terminator.js
```

## Quick Start

```javascript
const { Desktop } = require('terminator.js');

async function main() {
  const desktop = new Desktop();
  
  // Get root element
  const root = desktop.root();
  console.log('Root element:', root.role(), root.name());
  
  // Find and click a button
  const locator = desktop.locator('role:button');
  try {
    const button = await locator.first();
    console.log('Found button:', button.name());
    await button.click();
  } catch (error) {
    console.log('Button not found:', error.message);
  }
  
  // Take a screenshot
  const screenshot = await desktop.captureScreen();
  console.log(`Screenshot: ${screenshot.width}x${screenshot.height}`);
  
  // Run a command
  const result = await desktop.runCommand('echo hello', 'echo hello');
  console.log('Command output:', result.stdout);
}

main().catch(console.error);
```

## TypeScript Support

This package includes TypeScript definitions out of the box:

```typescript
import { Desktop, ElementNotFoundError } from 'terminator.js';

const desktop = new Desktop();
const root = desktop.root();
```

## Error Handling

The library provides specific error types for better error handling:

```javascript
const { 
  Desktop, 
  ElementNotFoundError, 
  TimeoutError, 
  PermissionDeniedError 
} = require('terminator.js');

try {
  const button = await desktop.locator('role:button').first();
  await button.click();
} catch (error) {
  if (error instanceof ElementNotFoundError) {
    console.log('Element not found');
  } else if (error instanceof TimeoutError) {
    console.log('Operation timed out');
  } else if (error instanceof PermissionDeniedError) {
    console.log('Permission denied');
  }
}
```

## Platform Support

- ✅ Windows (x64)

## API Reference

### Desktop

- `new Desktop()` - Create a new desktop automation instance
- `root()` - Get the root element
- `applications()` - List all applications
- `locator(selector)` - Create a locator for finding elements
- `captureScreen()` - Take a screenshot
- `runCommand(windowsCmd, unixCmd)` - Run a system command

### Element

- `click()` - Click the element
- `type(text)` - Type text into the element
- `role()` - Get element role
- `name()` - Get element name
- `children()` - Get child elements

### Locator

- `first()` - Get the first matching element
- `all()` - Get all matching elements
- `count()` - Count matching elements

## Examples

See the [examples directory](https://github.com/mediar-ai/terminator/tree/main/examples) for more usage examples.

## Repository

https://github.com/mediar-ai/terminator 