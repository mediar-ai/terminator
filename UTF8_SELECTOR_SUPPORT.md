# UTF-8 Selector Support in Terminator

## Status: ✅ Supported (with caveats)

This document describes the state of UTF-8/Unicode support in Terminator selectors, particularly for non-ASCII characters like Chinese, Japanese, Korean, Arabic, etc.

## Summary

**Rust Core**: ✅ Fully supports UTF-8 selectors
- Selector parsing correctly handles multi-byte UTF-8 characters
- Comprehensive tests added in `terminator/src/tests/utf8_selector_tests.rs`
- Tests cover Chinese (中文), Japanese (日本語), Korean (한국어), Arabic (العربية), Cyrillic (Русский), and emoji

**Python Bindings**: ✅ Should work automatically
- PyO3 handles UTF-8 string conversion from Python to Rust automatically
- Python 3.x uses UTF-8 as default string encoding

**Potential Issues**:
1. **Windows UI Automation API**: The underlying `uiautomation` crate (v0.22.0) passes strings to Windows UI Automation
   - Windows may use UTF-16 internally, but the Rust `windows` crate handles conversion
   - Element name matching depends on how the target application exposes element names
   - Some applications may not correctly expose localized element names

2. **Locale-specific behavior**: Element names may differ based on:
   - Windows display language
   - Application language settings
   - Regional settings

## Testing

### Rust Unit Tests
Run the comprehensive UTF-8 selector tests:
```bash
cargo test utf8_selector_tests
```

These tests verify that selector parsing works correctly with:
- Chinese characters (e.g., `role:Button|name:提交`)
- Japanese hiragana/katakana (e.g., `name:こんにちは`)
- Korean hangul (e.g., `text:안녕하세요`)
- Arabic RTL text (e.g., `name:مرحبا`)
- Cyrillic (e.g., `role:Button|name:Привет`)
- Mixed scripts (e.g., `role:Window|name:Settings 设置`)
- Emoji (e.g., `name:保存 💾`)

### Python Live Tests
Use the provided test script on a system with localized applications:

```bash
# Install the Python package first
cd bindings/python
pip install -e .

# Run the test
python ../../test_chinese_selectors.py
```

This script:
1. Opens Calculator
2. Lists all button names with their UTF-8 byte representations
3. Tests various selector encoding approaches
4. Verifies selector string passthrough from Python to Rust

## Known Limitations

1. **Application-dependent**: Whether UTF-8 selectors work depends on how the target application exposes element names
   - Some apps may only expose English names even on localized Windows
   - Some apps may expose localized names that match the Windows display language

2. **Windows UI Automation quirks**:
   - Element names are retrieved via `IUIAutomationElement::get_CurrentName()`
   - The returned string is in UTF-16, converted to UTF-8 by the `windows` crate
   - Matching is case-insensitive and uses substring matching (via `contains_name`)

3. **Testing challenges**:
   - Comprehensive testing requires access to Windows systems with different display languages
   - Calculator button names vary by Windows version and language

## Recommendations for Users

### For Chinese/CJK Systems:

1. **First, inspect actual element names**:
```python
import asyncio
import terminator

async def inspect_elements():
    desktop = terminator.Desktop()
    window = desktop.open_application("calc.exe")
    await asyncio.sleep(2)

    # Get all buttons to see their actual names
    buttons = await window.locator("role:Button").all(timeout_ms=5000, depth=10)
    for btn in buttons[:10]:
        print(f"Button name: '{btn.name()}'")

asyncio.run(inspect_elements())
```

2. **Use the actual names you observe**:
```python
# If you see the button is named "显示为"
element = await window.locator("role:Button|name:显示为").first()
```

3. **Fallback to role + NativeId when names are not reliable**:
```python
# Use AutomationId which is usually language-independent
element = await window.locator("nativeid:CalculatorResults").first()
```

### For Workflow Authors:

Prefer language-independent selectors when possible:
- `nativeid:` - Uses AutomationId (language-independent)
- `classname:` - Uses class names (language-independent)
- `#id` - Numeric IDs (generated, language-independent)

Use localized names only when necessary, and document:
```yaml
steps:
  - tool_name: click_element
    arguments:
      # Note: This selector is for Chinese Windows
      # English Windows users should use: role:Button|name:Display
      selector: "role:Button|name:显示为"
```

## Implementation Details

### Selector Parsing (Rust)
File: `terminator/src/selector.rs`

The selector parser uses standard Rust string methods:
- `split()` - Safe for UTF-8 (splits at character boundaries)
- `trim()` - Safe for UTF-8
- `strip_prefix()` - Safe for UTF-8

**Important**: Byte indexing (e.g., `s[5..]`) is safe when the prefix is ASCII (like `"role:"`, `"name:"`), as the index will always be at a UTF-8 character boundary.

### Windows Platform (Rust)
File: `terminator/src/platforms/windows/engine.rs`

String matching uses the `uiautomation` crate's `contains_name()` method:
```rust
matcher_builder = matcher_builder.contains_name(name);
```

This passes the UTF-8 string to the `uiautomation` crate, which converts it to UTF-16 for Windows APIs.

### Python Bindings
File: `bindings/python/src/desktop.rs`, `bindings/python/src/locator.rs`

PyO3 handles conversion automatically:
```rust
pub fn locator(&self, selector: &str) -> PyResult<Locator> {
    let locator = self.inner.locator(selector);
    Ok(Locator { inner: locator })
}
```

The `&str` parameter in Rust is UTF-8, and PyO3 automatically converts Python's UTF-8 strings to Rust's UTF-8 `&str`.

## Related Issues

- Issue #299: Chinese character support in selectors
- The comprehensive tests were added in response to this issue

## Future Improvements

1. Add integration tests with real localized applications
2. Test on Windows systems with different display languages
3. Document known application-specific quirks
4. Consider adding a helper to detect system locale and suggest appropriate selectors

## Contributing

If you encounter issues with UTF-8 selectors:
1. Run `test_chinese_selectors.py` and share the output
2. Report which application and Windows version you're using
3. Include the actual element names from the UI tree inspection
4. Specify your Windows display language setting
