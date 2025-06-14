# ClipboardTriggerBot

A powerful desktop utility built with Tauri that automatically fills forms using data from your clipboard, controlled by customizable keyboard shortcuts.

## Overview

ClipboardTriggerBot streamlines data entry by allowing you to copy structured information to your clipboard and then use a global shortcut (Ctrl+Shift+F or Cmd+Shift+F on macOS) to automatically fill form fields in any application. It uses simulated keystrokes to navigate between fields and input data, eliminating tedious manual entry.

## Features

- Global shortcut (Ctrl+Shift+F / Cmd+Shift+F) to trigger form filling from clipboard content
- Support for multiple clipboard data formats (JSON, key-value pairs, plain text)
- Customizable hotkeys for navigating between form fields
- Real-time activity logging
- System tray integration
- Cross-platform compatibility (Windows, macOS, Linux)

## Installation

### Prerequisites

- [Node.js](https://nodejs.org/) (v16 or higher)
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- Operating system dependencies for Tauri:
  - **Windows**: Microsoft Visual Studio C++ Build Tools
  - **macOS**: Xcode Command Line Tools
  - **Linux**: `libwebkit2gtk-4.0-dev`, `build-essential`, `curl`, `wget`, `libssl-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`

### Setup Instructions

1. Clone the repository:

   ```bash
   git clone https://github.com/yourusername/clipboard-trigger-bot.git
   cd clipboard-trigger-bot

   ```

2. Install dependencies:

   ```bash
   npm install
   ```

3. Build and run the application:
   ```bash
   npm run tauri dev
   ```
4. For production build:
   ```bash
   npm run tauri build
   ```
   The built application will be available in src-tauri/target/release

### macOS-Specific Setup

After installing on macOS, you must grant Accessibility permissions:

1. Open System Preferences
2. Navigate to Security & Privacy → Privacy → Accessibility
3. Click the lock icon to make changes (you'll need to enter your password)
4. Add ClipboardTriggerBot to the list of allowed applications
5. Make sure the checkbox next to it is checked

Without these permissions, the app won't be able to simulate keyboard input on macOS.

## Usage Guide

### Basic Usage

1. Launch ClipboardTriggerBot
2. Configure hotkeys for each field you want to fill
3. Focus on the target application where you need to fill a form
4. Copy the data you want to fill into the clipboard
5. Press `Ctrl+Shift+F` (or `Cmd+Shift+F` on macOS) to trigger automatic form filling

### Configuring Field Hotkeys

Each field in ClipboardTriggerBot requires a hotkey that will be used to navigate to that field:

1. `Field Name`: The identifier that will be matched with your clipboard data
2. `Hotkey`: The key or key combination to press before filling the field
3. `Value Preview`: Shows what will be filled (updates when you trigger the action)

Common hotkey examples:

- `Tab` - Move to the next field (most common)
- `Shift+Tab` - Move to the previous field
- `Alt+1`, `Alt+2`, etc. - Access numbered fields
- `Ctrl+A` - Select all in the current field (useful before overwriting)

### Supported Clipboard Formats

ClipboardTriggerBot can parse several data formats from your clipboard:

#### 1. JSON

```JSON
{
  "Field 1": "John Smith",
  "Field 2": "john.smith@example.com",
  "Field 3": "123-456-7890"
}
```

#### 2. Key-Value Pairs

```Code
Field 1: John Smith
Field 2: john.smith@example.com
Field 3: 123-456-7890
```

#### 3. Plain Text (by line position)

```Code
John Smith
john.smith@example.com
123-456-7890
```

## Examples

### Example 1: Contact Form

Setup:

1. Configure field mappings:
   - Field 1: "Name" with hotkey "Tab"
   - Field 2: "Email" with hotkey "Tab"
   - Field 3: "Phone" with hotkey "Tab"

Clipboard Data (JSON):

```JSON
{
"Name": "Jane Doe",
"Email": "jane.doe@example.com",
"Phone": "555-123-4567"
}
```

Result: When you press `Ctrl+Shift+F` while focused on the form:

1. The app will press Tab to navigate to the Name field and type "Jane Doe"
2. Press Tab again to move to the Email field and type "jane.doe@example.com"
3. Press Tab again to move to the Phone field and type "555-123-4567"

### Example 2: Login Form with Special Navigation

Setup:

1. Configure field mappings:
   - Field 1: "Username" with hotkey "Tab"
   - Field 2: "Password" with hotkey "Tab"
   - Field 3: "Submit" with hotkey "Enter"

Clipboard Data (Key-Value):

```Code
Username: adminuser
Password: securepassword123
Submit: true
```

Result: When you press Ctrl+Shift+F while focused on the login form:

1. The app will press Tab to navigate to the Username field and type "adminuser"
2. Press Tab again to move to the Password field and type "securepassword123"
3. Press Enter to submit the form

### Example 3: Complex Form with Alt-Key Navigation

Setup:

1. Configure field mappings:
   - Field 1: "Title" with hotkey "Alt+t"
   - Field 2: "Description" with hotkey "Alt+d"
   - Field 3: "Category" with hotkey "Alt+c"

Clipboard Data (Plain Text):

```Code
Project Proposal
A detailed description of the project scope and deliverables.
Technology
```

Result: When you press Ctrl+Shift+F:

1. The app will press Alt+t to focus the Title field and type "Project Proposal"
2. Press Alt+d to focus the Description field and type "A detailed description of the project scope and deliverables."
3. Press Alt+c to focus the Category field and type "Technology"
