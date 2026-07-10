# Terminator Examples

This directory contains example scripts demonstrating the [Terminator](https://github.com/mediar-ai/terminator) desktop-automation framework. Each script drives a real application (Calculator, Notepad, VLC, Chrome, …) through Terminator's accessibility-based UI automation — finding elements by role/name/AutomationId, clicking, typing, capturing screenshots, and running OCR.

## Prerequisites

- **Python 3.9+**
- The `terminator.py` package:
  ```bash
  pip install terminator.py
  ```
- **Pillow** (only needed for the screenshot/OCR examples — `element_screenshot.py`, `gmail_automation.py`):
  ```bash
  pip install Pillow
  ```
- **VLC** must be installed for `vlc_auto_player.py`. Streaming a YouTube link additionally needs:
  ```bash
  pip install yt-dlp        # resolves the page URL to a direct stream
  winget install Gyan.FFmpeg   # trims + re-streams the clip over HTTP
  ```
- **Google Chrome** for `gmail_automation.py`.

Most scripts open a real window and take over the mouse/keyboard while they run, so let them finish before touching the machine.

## Running the examples

Run any single example directly:

```bash
python examples/win_calculator.py
python examples/vlc_auto_player.py --youtube-link "https://www.youtube.com/watch?v=YQHsXMglC9A"
python examples/vlc_auto_player.py --file "my_video.mp4"
```

### Run them all

Two convenience runners execute every example in this directory (skipping the
other-OS demos `gnome-*`/`macos-*` and the `_*`-prefixed scratch scripts, and
running `vlc_auto_player.py` last because it streams for a while):

```bash
# Windows (PowerShell) — passes a default YouTube link to the VLC example
./examples/run_all.ps1

# Git Bash / Linux / macOS
./examples/run_all.sh
```

Both runners prefer a project virtualenv at `.venv/` if one exists, otherwise
they fall back to `python` on `PATH`.

## What each example does

### Windows applications

| Example | What it does |
|---------|--------------|
| [win_calculator.py](win_calculator.py) | Opens the Windows Calculator (by AppUserModelID) and computes `1 + 2`, reading the result back from the display. Uses `nativeid:` (AutomationId) selectors so `1`/`2` aren't confused with Scientific-mode exponent buttons. |
| [notepad.py](notepad.py) | Opens Notepad, highlights UI elements, and types text (handles both Windows 11 and the classic Notepad). |
| [mspaint.py](mspaint.py) | Opens MS Paint, selects shapes/tools, draws on the canvas, and saves the image. |
| [snipping_tool.py](snipping_tool.py) | Drives the Windows Snipping Tool: selects a snip mode, starts a new full-screen screenshot, and moves the mouse in a near-circle while the button is held. |

### Cross-platform

| Example | What it does |
|---------|--------------|
| [monitor_example.py](monitor_example.py) | Enumerates open windows and prints the monitor (name, id, geometry) each one lives on. |
| [element_screenshot.py](element_screenshot.py) | Launches Notepad, types multi-line text, captures a screenshot of the editor body, frames it with a border, saves `screenshot.png`, and runs OCR to read the text back. |
| [vlc_auto_player.py](vlc_auto_player.py) | Automates VLC to play a local file (`--file`) or a network/YouTube stream (`--youtube-link`). For YouTube it resolves the direct URL with yt-dlp and re-streams the first minute over local HTTP with ffmpeg (VLC's built-in YouTube extractor is unreliable). |

### Web automation

| Example | What it does |
|---------|--------------|
| [gmail_automation.py](gmail_automation.py) | Launches Chrome with a **dedicated automation profile** (`--user-data-dir` + `--force-renderer-accessibility`), opens Gmail, composes and sends an email, then opens it from the Sent folder — saving a numbered screenshot at each step. Edit the `recipient`/`subject`/`body` in `main()` before running; sign in once in the automation-profile window and it stays signed in. |

### Platform-specific (non-Windows)

| Example | What it does |
|---------|--------------|
| [gnome-calculator.py](gnome-calculator.py) | Automates the GNOME Calculator on Linux (adjusts selectors by detected version). |
| [macos_calculator.py](macos_calculator.py) | Automates the Calculator app on macOS. |

### Scratch / diagnostic scripts

`_`-prefixed files are ad-hoc diagnostics, not polished demos, and are skipped by the `run_all` scripts:

| File | What it does |
|------|--------------|
| [_diag_vlc.py](_diag_vlc.py) | Probes whether VLC's `Ctrl+N` (Open Media) dialog appears with and without activating the window first — used to debug focus/activation behavior. |

## Platform compatibility

| Example type | Windows | Linux | macOS |
|--------------|:-------:|:-----:|:-----:|
| Windows apps (calculator, notepad, mspaint, snipping tool) | ✓ | ✗ | ✗ |
| GNOME Calculator | ✗ | ✓ | ✗ |
| macOS Calculator | ✗ | ✗ | ✓ |
| Web automation (Gmail) | ✓ | ✓ | ✓ |
| Monitor / screenshot / OCR | ✓ | ✓ | ✓ |

## Troubleshooting

1. **"Application not found"** — Ensure the target application is installed and, on Windows, launchable by the identifier used in the script.
2. **"Element not found"** — UI selectors vary between OS/app versions. Many scripts branch on Windows 10 vs 11; adjust `role:`/`name:`/`nativeid:` selectors if your version differs.
3. **"Module not found"** — Install the dependency listed under [Prerequisites](#prerequisites) (`terminator.py`, `Pillow`, `yt-dlp`).
4. **Gmail shows a blank/empty page to automation** — Chrome must be launched with `--force-renderer-accessibility` (the example does this automatically); a normal Chrome window won't expose page content to the accessibility tree.
5. **VLC won't play a YouTube link** — Install `yt-dlp` and `ffmpeg`; VLC's bundled extractor can no longer decipher YouTube stream signatures on its own.
