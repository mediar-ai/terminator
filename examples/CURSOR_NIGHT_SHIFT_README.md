# 🌙 Cursor Night Shift Agent

Automate Cursor IDE with AI prompts while you sleep! This example demonstrates how to use Terminator to send prompts to Cursor at regular intervals, perfect for running background tasks while you're away from your keyboard.

## Overview

The Night Shift Agent automatically:
- 🎯 Finds and focuses the Cursor application window
- 💬 Sends prompts from a configurable list to Cursor's chat interface  
- ⏰ Waits for customizable intervals between prompts
- 🔄 Loops through prompts continuously
- 🛡️ Handles errors gracefully with automatic recovery
- 🎨 Provides visual feedback by highlighting UI elements

## Files

- `cursor_night_shift_agent.py` - Python implementation
- `cursor_night_shift_agent.ts` - TypeScript implementation
- `CURSOR_NIGHT_SHIFT_README.md` - This documentation

## Features

### 🤖 Intelligent Window Detection
- Automatically finds Cursor windows using multiple selectors
- Falls back to launching Cursor if not found
- Robust window focusing and management

### 💬 Smart Chat Interface Detection  
- Tries multiple methods to find chat input areas
- Uses keyboard shortcuts to open chat if needed
- Supports various Cursor UI layouts and versions

### 🔧 Configurable Automation
- Customizable prompt lists
- Adjustable intervals between prompts
- Configurable retry logic and error handling

### 🛡️ Error Recovery
- Graceful handling of UI changes
- Automatic retry with exponential backoff
- Continues running despite individual failures

## Requirements

### Python Version
- Python 3.7+
- Terminator Python bindings
- Cursor IDE installed

### TypeScript Version  
- Node.js 16+
- TypeScript or tsx for execution
- Terminator Node.js bindings
- Cursor IDE installed

## Installation

### For Python
```bash
# Install terminator python bindings (if not already installed)
pip install terminator

# Make the script executable (optional)
chmod +x cursor_night_shift_agent.py
```

### For TypeScript
```bash
# Navigate to examples directory
cd examples

# Install dependencies
npm install

# Install tsx for TypeScript execution (optional)
npm install -g tsx
```

## Configuration

Both versions can be configured by modifying the constants at the bottom of the files:

### Prompts
Customize the `PROMPTS` list with your desired automation tasks:

```python
# Python
PROMPTS = [
    "Review the codebase and suggest any potential improvements to error handling",
    "Check for any unused imports or variables in the current project", 
    "Look for opportunities to add helpful comments or documentation",
    # Add your custom prompts here...
]
```

```typescript
// TypeScript
const PROMPTS = [
    "Review the codebase and suggest any potential improvements to error handling",
    "Check for any unused imports or variables in the current project",
    "Look for opportunities to add helpful comments or documentation",
    // Add your custom prompts here...
];
```

### Timing
Adjust the interval between prompts:

```python
# Python - Time between prompts in seconds
INTERVAL_SECONDS = 300  # 5 minutes
```

```typescript
// TypeScript - Time between prompts in seconds  
const INTERVAL_SECONDS = 300; // 5 minutes
```

### Advanced Configuration
You can also modify the agent constructor parameters:

```python
# Python
agent = CursorNightShiftAgent(
    prompts=PROMPTS,
    interval_seconds=INTERVAL_SECONDS,
    max_retries=3  # Number of retries per operation
)
```

## Usage

### Running the Python Version

```bash
# Basic usage
python cursor_night_shift_agent.py

# Or if made executable
./cursor_night_shift_agent.py
```

### Running the TypeScript Version

```bash
# Using tsx (recommended)
npx tsx cursor_night_shift_agent.ts

# Or compile and run with node
tsc cursor_night_shift_agent.ts
node cursor_night_shift_agent.js
```

## How It Works

### 1. Window Detection
The agent uses multiple strategies to find Cursor:
- Searches for windows with names containing "Cursor" 
- Tries different accessibility selectors
- Attempts to launch Cursor if not found

### 2. Chat Interface Detection
Once Cursor is found, the agent locates the chat input:
- Searches for text input elements using accessibility roles
- Tries common selectors for chat interfaces
- Falls back to keyboard shortcuts (Ctrl+L, Ctrl+K, etc.)

### 3. Prompt Sending
For each prompt cycle:
- Focuses the chat input area
- Clears any existing text
- Types the prompt text
- Sends using Enter, Ctrl+Enter, or Shift+Enter
- Waits for the configured interval

### 4. Error Handling
The agent includes robust error handling:
- Retries failed operations
- Longer waits after consecutive failures
- Continues running despite individual errors
- Graceful shutdown on Ctrl+C

## Example Prompt Ideas

### Code Review Tasks
```
"Review the current file for potential bugs or issues"
"Suggest improvements to code readability and maintainability" 
"Check for security vulnerabilities in the codebase"
"Look for performance optimization opportunities"
```

### Documentation Tasks
```
"Generate JSDoc comments for functions missing documentation"
"Create a README section for the current module"
"Suggest improvements to existing code comments"
"Generate usage examples for the current API"
```

### Refactoring Tasks  
```
"Identify code duplication and suggest refactoring"
"Look for opportunities to extract utility functions"
"Suggest better variable and function names"
"Check for unused imports and variables"
```

### Testing Tasks
```
"Generate unit tests for the current module"
"Suggest edge cases that should be tested"
"Review existing tests for completeness"
"Generate integration test scenarios"
```

## Troubleshooting

### Common Issues

#### "Could not find Cursor window"
- Make sure Cursor is running before starting the agent
- Try manually focusing Cursor and running again
- Check if Cursor window title contains "Cursor"

#### "Could not find chat input"
- Ensure Cursor's chat interface is accessible
- Try manually opening chat (Ctrl+L) before running
- Update selectors if Cursor UI has changed

#### "All send methods failed"
- Check if chat input is accepting text
- Verify Cursor permissions and focus
- Try running with shorter prompts first

#### TypeScript compilation errors
- Ensure @types/node is installed: `npm install --save-dev @types/node`
- Use tsx for direct TypeScript execution
- Check Node.js version compatibility

### Debug Mode
Enable debug logging by changing the log level:

```python
# Python - Change log_level to "debug"
desktop = terminator.Desktop(log_level="debug")
```

```typescript
// TypeScript - Change log level to 'debug'
const desktop = new Desktop(undefined, undefined, 'debug');
```

## Safety Considerations

⚠️ **Important Safety Notes:**

- **Test First**: Always test with a small number of cycles before running overnight
- **Rate Limiting**: Don't set intervals too short to avoid overwhelming Cursor or the AI service
- **Monitor Usage**: Keep an eye on AI service usage and costs
- **Backup Work**: Ensure your work is saved/committed before running automation
- **Interruption**: The agent can be stopped anytime with Ctrl+C

## Contributing

This example demonstrates key Terminator automation patterns:
- Window detection and focusing
- UI element discovery with fallbacks  
- Text input automation
- Error handling and recovery
- Cross-platform compatibility

Feel free to extend this example for other automation scenarios!

## Related Examples

- `notepad.py` - Basic text input automation
- `win_calculator.py` - UI element interaction
- `gmail_automation.py` - Web application automation

## License

This example is part of the Terminator project and follows the same license.