# Excel Copilot

This is an example application demonstrating the Terminator SDK, showcasing how to automate Microsoft Excel using UI automation. It also integrates Google Gemini AI for enhanced data analysis. Built with Tauri, React, TypeScript, and Rust.

## Overview

Excel Copilot enables AI-powered Excel automation through direct window manipulation. The application uses the Terminator library for precise UI automation and Google Gemini for intelligent data analysis and formula generation.

## Features

- **Excel Automation**: Read/write cells, set formulas, read ranges via UI automation
- **AI Integration**: Chat with Gemini to analyze and manipulate Excel data
- **PDF Processing**: Attach PDF files for data extraction and analysis
- **Real-time Operations**: Direct interaction with Excel window

## Prerequisites

- Windows 10/11
- Node.js 18+
- Rust (latest stable)
- Microsoft Excel
- Google Gemini API key

## Installation

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

