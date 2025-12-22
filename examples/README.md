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

## Troubleshooting

1. **"Application not found"** - Ensure the target application is installed
2. **"Element not found"** - UI selectors may vary between OS versions
3. **"Module not found"** - Install required dependencies
4. **Encoding errors** - Fixed for cross-platform compatibility 
