# Excel Copilot

This is an example application demonstrating the Terminator SDK, showcasing how to automate Microsoft Excel using UI automation. It also integrates Google Gemini AI for enhanced data analysis and **Microsoft Excel Copilot** for advanced Excel features. Built with Tauri, React, TypeScript, and Rust.

## Overview

Excel Copilot enables AI-powered Excel automation through direct window manipulation. The application uses the Terminator library for precise UI automation, Google Gemini for intelligent data analysis and formula generation, and **can interact with Microsoft Excel Copilot** for advanced formatting, chart creation, and data analysis.

## Features

- **Excel Automation**: Read/write cells, set formulas, read ranges via UI automation
- **AI Integration**: Chat with Gemini to analyze and manipulate Excel data
- **Microsoft Excel Copilot Integration**: Advanced formatting, charts, conditional formatting
- **TSV Batch Operations**: Efficient bulk data import and processing
- **PDF Processing**: Attach PDF files for data extraction and analysis
- **Real-time Operations**: Direct interaction with Excel window
- **UI Context Awareness**: Precise targeting of Excel UI elements

## Prerequisites

- Windows 10/11
- Node.js 18+
- Rust (latest stable)
- Microsoft Excel (Copilot features are optional; requires Microsoft 365 subscription if you wish to use Copilot)
- Google Gemini API key
- **OneDrive account** (only required if using Copilot features)

## Installation

0. Install [webview2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/?form=MA13LH)
1. Clone the repository
2. Install dependencies:
   ```bash
   cd examples/excel-copilot
   npm install
   ```
3. Run the application:
   ```bash
   npm run tauri:dev
   ```

## Configuration

1. Obtain a Gemini API key from [Google AI Studio](https://makersuite.google.com/app/apikey)
2. Launch the application
3. Click the settings button and enter your API key
4. Ensure that the status state is "ready"
5. Click "new" to create a new file or "open" to use an existing file
6. Start chatting with Excel data

## Usage

### Basic Commands
- "Read cell A1"
- "Write 'Sales' to cell B1"
- "Set formula =SUM(A1:A10) in cell C1"
- "Read range A1:C5"

### Data Analysis
- "Analyze the data in columns A through D"
- "Find the maximum value in this sheet"
- "Create a summary of this data"

### PDF Integration
- Attach PDF files to extract data into Excel
- "Extract table data from this PDF"
- "Create Excel sheet from this invoice"

## Microsoft Excel Copilot Requirements

**⚠️ IMPORTANT: Microsoft Excel Copilot has specific requirements that must be met for it to function properly.**

### Essential Requirements

1. **Microsoft 365 Subscription**: Excel Copilot requires an active Microsoft 365 subscription with Copilot features enabled.

2. **OneDrive Storage**: 
   - Files **MUST** be saved in OneDrive (not local storage)
   - Use **"Open"** button to select existing OneDrive files (not "New")
   - Check file path contains "OneDrive" to verify

3. **Auto-Save Enabled**:
   - Enable auto-save in Excel for best results
   - Ensures changes are immediately synced to OneDrive

4. **Data Structure Requirements**:
   - Data range must have **at least 3 rows and 2 columns**
   - **Headers are required** in the first row
   - Headers must have different names (no duplicates)
   - Data should be properly structured in table format

### Copilot Features Available

When requirements are met, you can use:

- **Advanced Formatting**: "Format this data with bold headers, currency format for values, and alternating row colors"
- **Chart Creation**: "Create a column chart showing sales by month with title 'Monthly Sales Report'"
- **Conditional Formatting**: "Highlight values greater than 1000 in red and less than 500 in yellow"
- **Data Analysis**: "Create a summary table showing totals, averages, and trends from this data"

### Enabling/Disabling Copilot

- Use the toggle switch in the application to enable/disable Copilot features
- When disabled, only basic Excel automation tools are available
- When enabled, ensure all requirements above are met

### Troubleshooting Copilot Issues

- **"Copilot not working"** → Check if file is saved in OneDrive
- **"No Copilot button"** → Verify Microsoft 365 subscription and Excel version
- **"Insufficient data" error from copilot** → Ensure at least 3 rows and 2 columns with headers
- **"Auto-save required"** → Enable auto-save in Excel settings

## Technical Details

### Architecture
- **Frontend**: React + TypeScript for the user interface
- **Backend**: Rust + Tauri for system integration and Excel automation
- **Automation**: Terminator library for Windows UI automation
- **AI**: Google Gemini API with function calling capabilities

### Available Functions
- `excel_read_cell` - Read value from specific cell
- `excel_write_cell` - Write value to specific cell
- `excel_read_range` - Read data from cell range
- `excel_set_formula` - Set Excel formula with validation
- `get_excel_sheet_overview` - Get complete sheet status
- `get_excel_ui_context` - Get UI element tree for precise targeting
- `paste_tsv_batch_data` - Efficient bulk data import via TSV format
- `send_request_to_excel_copilot` - Send requests to Microsoft Excel Copilot
- `format_cells_with_copilot` - Advanced cell formatting via Copilot
- `create_chart_with_copilot` - Chart creation via Copilot
- `apply_conditional_formatting_with_copilot` - Conditional formatting via Copilot

## Build

Development:
```bash
npm run tauri:dev
```

Production:
```bash
npm run tauri:build
```

## Requirements

- Windows operating system (required for Excel automation)
- Microsoft Excel installed and running
- Valid Google Gemini API key
- Node.js and Rust development environment
- WebView2 : https://developer.microsoft.com/en-us/microsoft-edge/webview2/?form=MA13LH
