# Website Testing Examples

This directory contains example test scripts that demonstrate how to use the Terminator SDK for automated website testing. These examples are designed to work with the GitHub Action for Terminator web testing.

## Available Tests

### `test_google_search.py` (Python)

Tests Google search functionality by:
1. Opening Google homepage
2. Finding the search input field
3. Typing a search term
4. Submitting the search
5. Verifying search results appear

**Usage:**
```bash
# Run directly
python test_google_search.py

# With custom environment variables
TERMINATOR_URL="https://www.google.com" SEARCH_TERM="AI automation" python test_google_search.py

# Via npm script
npm run test:google
```

### `test_wikipedia_search.ts` (TypeScript)

Tests Wikipedia search functionality by:
1. Opening Wikipedia homepage
2. Finding the search input field
3. Typing a search term
4. Submitting the search
5. Verifying search results or article content appears

**Usage:**
```bash
# Run directly (requires ts-node)
ts-node test_wikipedia_search.ts

# With custom environment variables
TERMINATOR_URL="https://en.wikipedia.org" SEARCH_TERM="Machine Learning" ts-node test_wikipedia_search.ts

# Via npm script
npm run test:wikipedia
```

## Environment Variables

Both test scripts support these environment variables:

- `TERMINATOR_URL`: The website URL to test
- `TERMINATOR_BROWSER`: Browser to use (automatically set by GitHub Action)
- `TERMINATOR_HEADLESS`: Whether to run in headless mode
- `TERMINATOR_TIMEOUT`: Test timeout in seconds
- `SEARCH_TERM`: Custom search term to use in tests

## Setup for Local Development

1. **Install Python Dependencies** (for Python tests):
   ```bash
   pip install terminator asyncio
   ```

2. **Install Node.js Dependencies** (for TypeScript tests):
   ```bash
   npm install
   ```

3. **Build Terminator SDK** (if running locally):
   ```bash
   # From repository root
   cd bindings/python && maturin develop
   cd ../nodejs && npm run build && npm link
   ```

## Running Tests Locally

### Prerequisites

- Desktop environment (GUI) available
- Browser installed (Chrome, Firefox, or Edge)
- Terminator SDK built and installed

### Quick Test

```bash
# Test Google with Python
TERMINATOR_URL="https://www.google.com" python test_google_search.py

# Test Wikipedia with TypeScript  
TERMINATOR_URL="https://en.wikipedia.org" ts-node test_wikipedia_search.ts
```

## GitHub Action Integration

These tests are designed to work seamlessly with the GitHub Action:

```yaml
- name: Test Google Search
  uses: ./.github/actions/terminator-web-test
  with:
    website-url: 'https://www.google.com'
    test-script: 'examples/website-tests/test_google_search.py'
    language: python
    browser: chrome

- name: Test Wikipedia Search  
  uses: ./.github/actions/terminator-web-test
  with:
    website-url: 'https://en.wikipedia.org'
    test-script: 'examples/website-tests/test_wikipedia_search.ts'
    language: typescript
    browser: firefox
```

## Writing Your Own Tests

Use these examples as templates for your own website tests:

1. **Copy an existing test** that matches your preferred language
2. **Modify the URL and test logic** for your specific website
3. **Update the locator strategies** to match your target elements
4. **Add appropriate assertions** to verify expected behavior
5. **Test locally** before integrating with GitHub Actions

### Key Patterns

**Element Location:**
```python
# Python
search_box = await document.locator('role:SearchBox').first()
button = await document.locator('name:Submit').first()

# TypeScript
const searchBox = await document.locator('role:SearchBox').first();
const button = await document.locator('name:Submit').first();
```

**Element Interaction:**
```python
# Python
await search_box.type_text("search term")
await search_box.press_key("Return")
button.click()

# TypeScript
await searchBox.typeText("search term");
await searchBox.pressKey("Return");
await button.click();
```

**Verification:**
```python
# Python
results = await document.locator('role:Main').first()
if results:
    logging.info("✅ Test passed!")

# TypeScript
const results = await document.locator('role:Main').first();
if (results) {
    console.log("✅ Test passed!");
}
```

## Troubleshooting

### Common Issues

1. **Element not found**: 
   - Check if page has loaded completely
   - Try different locator strategies
   - Add debug logging to see available elements

2. **Test timeouts**:
   - Increase wait times between actions
   - Check network connectivity
   - Verify browser is launching correctly

3. **Locator failures**:
   - Use element highlighting to debug
   - Try multiple fallback strategies
   - Check browser developer tools for accessibility properties

### Debug Mode

Enable debug logging:

```python
# Python
desktop = terminator.Desktop(log_level="debug")

# TypeScript
const desktop = new terminator.Desktop(undefined, undefined, 'debug');
```

### Element Highlighting

Use highlighting to visualize what elements are being found:

```python
# Python
element.highlight(color=0x00FF00, duration_ms=2000)  # Green highlight

# TypeScript
element.highlight(0x00FF00, 2000);  // Green highlight
```

## Contributing

When adding new test examples:

1. Follow the existing code structure and naming conventions
2. Include comprehensive error handling and logging
3. Add environment variable support for customization
4. Test on multiple browsers and platforms
5. Update this README with your new example
6. Consider creating both Python and TypeScript versions for consistency

## Support

For questions about these examples or writing your own tests:

- Check the main GitHub Action documentation
- Review the Terminator SDK documentation
- Open an issue with your specific use case
- Share your own test patterns with the community