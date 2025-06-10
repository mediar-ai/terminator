# Terminator Web Test Action

A GitHub Action for testing websites using the Terminator SDK with desktop automation capabilities. This action provides cross-platform web testing on both Ubuntu and Windows environments with full UI support.

## Features

- 🌐 **Cross-Platform Testing**: Works on Ubuntu (with virtual display) and Windows
- 🐍 **Multi-Language Support**: Python and TypeScript/Node.js
- 🌍 **Multi-Browser Support**: Chrome, Firefox, and Microsoft Edge
- 🖥️ **Desktop Automation**: Uses OS-level accessibility APIs for reliable automation
- 📸 **Failure Screenshots**: Automatically captures screenshots on test failures
- ⚡ **Fast Setup**: Automated environment configuration and browser installation
- 🔧 **Configurable**: Extensive customization options for different testing scenarios

## Quick Start

### Basic Usage

```yaml
name: Website Test
on: [push, pull_request]

jobs:
  test-website:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Test Google Search
        uses: ./.github/actions/terminator-web-test
        with:
          website-url: 'https://www.google.com'
          test-script: 'path/to/your/test_script.py'
          language: python
          browser: chrome
```

### Advanced Matrix Testing

```yaml
name: Cross-Platform Website Testing
on: [push]

jobs:
  test-matrix:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
        language: [python, typescript]
        browser: [chrome, firefox]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      
      - name: Run Website Tests
        uses: ./.github/actions/terminator-web-test
        with:
          website-url: 'https://example.com'
          test-script: ${{ matrix.language == 'python' && 'tests/test.py' || 'tests/test.ts' }}
          language: ${{ matrix.language }}
          browser: ${{ matrix.browser }}
          timeout: 300
```

## Input Parameters

| Parameter | Description | Required | Default |
|-----------|-------------|----------|---------|
| `website-url` | URL of the website to test | ✅ | - |
| `test-script` | Path to test script relative to repository root | ✅ | - |
| `language` | Language for automation (`python` or `typescript`) | ❌ | `python` |
| `browser` | Browser to use (`chrome`, `firefox`, `edge`) | ❌ | `chrome` |
| `headless` | Run tests in headless mode (`true`/`false`) | ❌ | `false` |
| `timeout` | Test timeout in seconds | ❌ | `300` |
| `python-version` | Python version (for Python tests) | ❌ | `3.11` |
| `node-version` | Node.js version (for TypeScript tests) | ❌ | `20` |

## Output Parameters

| Parameter | Description |
|-----------|-------------|
| `test-result` | Test execution result (`success` or `failure`) |
| `screenshot-path` | Path to failure screenshot (if available) |

## Environment Variables

Your test scripts can access these environment variables:

- `TERMINATOR_URL`: The website URL being tested
- `TERMINATOR_BROWSER`: The browser being used
- `TERMINATOR_HEADLESS`: Whether running in headless mode
- `TERMINATOR_TIMEOUT`: Test timeout value
- `SEARCH_TERM`: Custom search term (if set in workflow)

## Writing Test Scripts

### Python Example

```python
#!/usr/bin/env python3
import asyncio
import terminator
import logging
import os

async def test_website():
    url = os.getenv('TERMINATOR_URL', 'https://example.com')
    desktop = terminator.Desktop(log_level="info")
    
    try:
        # Open website
        desktop.open_url(url)
        await asyncio.sleep(3)
        
        # Find elements and interact
        window = desktop.locator('role:Window')
        document = window.locator('role:Document')
        
        # Your test logic here
        search_box = await document.locator('role:SearchBox').first()
        search_box.type_text("test query")
        await search_box.press_key("Return")
        
        # Verify results
        results = await document.locator('role:Main').first()
        if results:
            logging.info("✅ Test passed!")
            return True
        
    except Exception as e:
        logging.error(f"Test failed: {e}")
        return False

if __name__ == "__main__":
    asyncio.run(test_website())
```

### TypeScript Example

```typescript
const terminator = require('terminator.js');

async function testWebsite(): Promise<boolean> {
    const url = process.env.TERMINATOR_URL || 'https://example.com';
    const desktop = new terminator.Desktop(undefined, undefined, 'info');
    
    try {
        // Open website
        desktop.openUrl(url);
        await new Promise(resolve => setTimeout(resolve, 3000));
        
        // Find elements and interact
        const window = desktop.locator('role:Window');
        const document = window.locator('role:Document');
        
        // Your test logic here
        const searchBox = await document.locator('role:SearchBox').first();
        await searchBox.typeText("test query");
        await searchBox.pressKey("Return");
        
        // Verify results
        const results = await document.locator('role:Main').first();
        if (results) {
            console.log("✅ Test passed!");
            return true;
        }
        
    } catch (error) {
        console.error(`Test failed: ${error}`);
        return false;
    }
}

testWebsite();
```

## Supported Platforms

### Ubuntu (Linux)

- ✅ **Virtual Display**: Automatic Xvfb setup with fluxbox window manager
- ✅ **Browsers**: Chrome, Firefox, Microsoft Edge
- ✅ **Desktop Environment**: Full GUI support through virtual display
- ✅ **Screenshots**: Automatic failure screenshot capture

### Windows

- ✅ **Native Display**: Uses Windows desktop environment
- ✅ **Browsers**: Chrome, Firefox, Microsoft Edge (pre-installed)
- ✅ **Desktop Environment**: Native Windows GUI support
- ✅ **Screenshots**: Automatic failure screenshot capture

## Browser Support

| Browser | Ubuntu | Windows | Notes |
|---------|---------|---------|-------|
| Chrome | ✅ | ✅ | Automatically installed |
| Firefox | ✅ | ✅ | Automatically installed |
| Edge | ✅ | ✅ | Pre-installed on Windows |

## Terminator SDK Features

The action leverages Terminator's powerful automation capabilities:

- **Accessibility-Based**: Uses OS accessibility APIs (not vision-based)
- **Fast Performance**: 80ms UI scans with optimized workflows
- **Cross-Platform**: Works across Windows, macOS, and Linux
- **Element Highlighting**: Visual feedback during test execution
- **Robust Locators**: Multiple strategies for finding UI elements
- **Error Handling**: Comprehensive error reporting and recovery

## Locator Strategies

Terminator supports various locator strategies:

```python
# By name/label
element = desktop.locator('name:Submit Button')

# By role (accessibility role)
element = desktop.locator('role:Button')

# By native ID
element = desktop.locator('nativeid:submit-btn')

# By window title
window = desktop.locator('window:Google Chrome')
```

## Troubleshooting

### Common Issues

1. **Element Not Found**
   - Increase wait times for page loading
   - Try alternative locator strategies
   - Check if element is in correct window/frame

2. **Test Timeouts**
   - Increase the `timeout` parameter
   - Add explicit waits in your test script
   - Check network connectivity in CI environment

3. **Browser Issues**
   - Try different browsers
   - Check if browser installation succeeded
   - Verify virtual display is working (Ubuntu)

4. **Virtual Display Issues (Ubuntu)**
   - Check Xvfb logs
   - Verify display environment variable
   - Ensure window manager is running

### Debug Mode

Enable debug logging in your test scripts:

```python
# Python
desktop = terminator.Desktop(log_level="debug")

# TypeScript
const desktop = new terminator.Desktop(undefined, undefined, 'debug');
```

### Screenshots

The action automatically captures screenshots on failures. Access them via:

```yaml
- name: Upload Screenshots
  if: failure()
  uses: actions/upload-artifact@v4
  with:
    name: test-screenshots
    path: test-outputs/
```

## Performance Optimization

- **Caching**: Rust dependencies are cached between runs
- **Parallel Execution**: Use matrix strategies for concurrent testing
- **Selective Testing**: Use path filters to run tests only when needed
- **Timeout Management**: Set appropriate timeouts for your test complexity

## Security Considerations

- Test scripts run with standard GitHub Actions permissions
- No sensitive data is logged by default
- Screenshots may contain sensitive information - review before sharing
- Browser automation has access to the virtual/desktop environment

## Examples Repository

Check the `examples/website-tests/` directory for complete working examples:

- `test_google_search.py` - Python Google search automation
- `test_wikipedia_search.ts` - TypeScript Wikipedia search automation

## Contributing

When contributing test scripts or improvements:

1. Follow the existing code patterns
2. Add comprehensive error handling
3. Include logging for debugging
4. Test on both Ubuntu and Windows when possible
5. Update documentation for new features

## Support

For issues and questions:

- 🐛 **Bugs**: Open an issue with reproduction steps
- 💡 **Feature Requests**: Describe your use case and requirements
- 📖 **Documentation**: Improve this README with your learnings
- 🤝 **Community**: Share your automation patterns and solutions