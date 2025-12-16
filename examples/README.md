# Terminator Examples

This directory contains example scripts demonstrating various capabilities of the Terminator automation framework.

## Examples Table

| Example | Path | Description |
|---------|------|-------------|
| **Python - Windows Applications** | | |
| Windows Calculator | `win_calculator.py` | Automates Windows Calculator with arithmetic operations |
| Notepad Automation | `notepad.py` | Automates basic interactions with Windows Notepad |
| MS Paint Automation | `mspaint.py` | Automates drawing shapes and saving images in MS Paint |
| Snipping Tool | `snipping_tool.py` | Automates the Windows Snipping Tool for screenshots |
| **Python - Cross-Platform** | | |
| Monitor Example | `monitor_example.py` | Retrieves monitor information for windows and UI elements |
| Element Screenshot | `element_screenshot.py` | Captures screenshots of UI elements and performs OCR |
| VLC Auto Player | `vlc_auto_player.py` | Controls VLC media player to automatically play media |
| **Python - Platform-Specific** | | |
| GNOME Calculator | `gnome-calculator.py` | Demonstrates automation of GNOME Calculator on Linux |
| macOS Calculator | `macos_calculator.py` | Demonstrates automation of Calculator on macOS |
| **Python - Web Automation** | | |
| Gmail Automation | `gmail_automation.py` | Automates common tasks within the Gmail web interface |
| **YAML Workflows** | | |
| I-94 Automation | `i94_automation.yml` | Declarative workflow for U.S. I-94 travel form |
| Browser DOM Extraction | `browser_dom_extraction.yml` | Extract data from browser DOM |
| Comprehensive UI Test | `comprehensive_ui_test.yml` | Full UI automation test suite |
| Simple Browser Test | `simple_browser_test.yml` | Basic browser automation workflow |
| GitHub Actions Commands | `github_actions_style_commands.yml` | GitHub Actions-style command syntax |
| Website Search Parser | `website_search_parser.yml` | Parse and extract website search results |
| Web Monitor with Skip | `web_monitor_with_skip.yml` | Monitor web pages with skip conditions |
| Enable Trailing Cursor | `enable_trailing_cursor.yml` | Enable trailing cursor accessibility feature |
| **TypeScript Workflows** | | |
| Simple Notepad Workflow | `simple_notepad_workflow/` | Complete TypeScript workflow example |
| **Project Examples** | | |
| PDF to Form | `pdf-to-form/` | Converts PDF data into web forms using Terminator |
| reCAPTCHA Resolver | `recaptcha-resolver/` | Automated reCAPTCHA solving |
| AI Explorer | `ai-explorer/` | AI-powered UI exploration |
| Next.js Workflows | `nextjs-workflows/` | Next.js integration examples |

## Platform Compatibility

| Example Type | Windows | Linux | macOS |
|--------------|---------|-------|-------|
| Windows Apps (notepad, mspaint, etc.) | ✓ | ✗ | ✗ |
| GNOME Calculator | ✗ | ✓ | ✗ |
| macOS Calculator | ✗ | ✗ | ✓ |
| Web Automation | ✓ | ✓ | ✓ |
| Monitor/Screenshot | ✓ | ✓ | ✓ |
| YAML Workflows | ✓ | ✓ | ✓ |
| TypeScript Workflows | ✓ | ✓ | ✓ |

## Troubleshooting

1. **"Application not found"** - Ensure the target application is installed
2. **"Element not found"** - UI selectors may vary between OS versions
3. **"Module not found"** - Install required dependencies
4. **Encoding errors** - Fixed for cross-platform compatibility 
