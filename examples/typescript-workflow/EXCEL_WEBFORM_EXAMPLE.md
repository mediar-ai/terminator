# Excel to Web Form Data Entry Workflow

This example demonstrates a real-world TypeScript workflow that automates data entry from a CSV/Excel file to a web form.

## What It Does

1. **Reads CSV data** - Parses a CSV file with contact information
2. **Opens web browser** - Navigates to the target web form
3. **Fills form fields** - Automates data entry for each row
4. **Handles errors** - Recovers from form validation errors
5. **Generates report** - Creates a summary of successful/failed entries

## Target Form

**URL:** https://www.andrews.edu/~bidwell/examples/form.html

**Fields:**
- Title (radio buttons: Dr., Prof., Mr., Ms.)
- First Name
- Last Name
- Organization
- Phone
- Email
- Street Address
- City
- State
- Zip Code

## Usage

### Basic Usage (Process All Rows)

```bash
cd examples/typescript-workflow
npm install
tsx excel-to-webform-workflow.ts
```

### Process Specific Rows

```bash
# Process rows 1-3 only
tsx excel-to-webform-workflow.ts ./sample-data.csv 1 3

# Process starting from row 2
tsx excel-to-webform-workflow.ts ./sample-data.csv 2
```

### Custom CSV File

```bash
tsx excel-to-webform-workflow.ts /path/to/your/data.csv
```

## CSV Format

The CSV file should have these columns (header row required):

```csv
FirstName,LastName,Title,Organization,Phone,Email,Address1,City,State,Zip
John,Smith,Dr.,Acme Corp,(269) 555-0123,john.smith@example.com,123 Main St,Berrien Springs,MI,49103
```

**Included sample:** `sample-data.csv` (5 rows of test data)

## Input Parameters

```typescript
{
  csvFile: string;           // Path to CSV file (default: './sample-data.csv')
  webFormUrl: string;        // URL of web form (default: Andrews form)
  startRow: number;          // Starting row, 1-indexed (default: 1)
  endRow?: number;           // Ending row, optional (default: all rows)
  delayBetweenRows: number;  // Delay in ms between rows (default: 2000)
}
```

## Features Demonstrated

### 1. Type-Safe Inputs (Zod Schema)

```typescript
const InputSchema = z.object({
  csvFile: z.string().default('./sample-data.csv'),
  startRow: z.number().default(1).min(1),
  delayBetweenRows: z.number().default(2000),
});
```

### 2. Error Recovery

```typescript
onError: async ({ error, desktop, retry, attempt, logger }) => {
  if (attempt < 2) {
    logger.info('🔄 Retrying...');
    await desktop.delay(2000);
    return retry();
  }
  return { recoverable: false, reason: 'Max retries exceeded' };
}
```

### 3. Context Sharing Between Steps

```typescript
// Step 1: Store data in context
context.data.rows = filteredRows;

// Step 3: Access data from context
const rows = context.data.rows as FormData[];
```

### 4. Progress Tracking

```typescript
for (let i = 0; i < rows.length; i++) {
  const rowNumber = input.startRow + i;
  logger.info(`📝 Row ${rowNumber}: ${rowData.FirstName} ${rowData.LastName}`);
  // ... process row
}
```

### 5. Result Aggregation

```typescript
const results = {
  succeeded: 0,
  failed: 0,
  errors: [] as Array<{ row: number; error: string }>,
};
```

### 6. Summary Report Generation

Creates `data-entry-report.json`:

```json
{
  "timestamp": "2025-10-29T23:45:00.000Z",
  "csvFile": "./sample-data.csv",
  "webFormUrl": "https://www.andrews.edu/~bidwell/examples/form.html",
  "rowsProcessed": 5,
  "succeeded": 5,
  "failed": 0,
  "successRate": "100.0%",
  "errors": []
}
```

## Workflow Steps

### Step 1: Read CSV Data

- Reads and parses CSV file
- Validates headers
- Filters rows based on startRow/endRow
- Stores data in context

### Step 2: Open Web Form

- Launches Chrome browser
- Navigates to form URL
- Waits for page load
- Validates form is ready (checks for First Name field)
- Retries up to 2 times on failure

### Step 3: Fill Web Form

- Iterates through each CSV row
- Fills all form fields for current row
- Sets radio button for Title
- Fills text fields for all other data
- Resets form between rows
- Tracks success/failure for each row
- Continues processing even if a row fails

### Step 4: Generate Report

- Creates summary statistics
- Saves JSON report to disk
- Includes error details for failed rows

## Using with mediar-app

### 1. Load Workflow

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "url": "file://./examples/typescript-workflow/excel-to-webform-workflow.ts",
    "inputs": {
      "csvFile": "./sample-data.csv",
      "startRow": 1,
      "endRow": 3
    }
  }
}
```

### 2. Start/Stop at Specific Steps (Debugging)

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "url": "file://./examples/typescript-workflow/excel-to-webform-workflow.ts",
    "start_from_step": "fill-web-form",
    "end_at_step": "fill-web-form",
    "inputs": {
      "csvFile": "./sample-data.csv"
    }
  }
}
```

### 3. Resume from Failure

If workflow fails at step 3, state is automatically saved. Fix the issue and resume:

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "url": "file://./examples/typescript-workflow/excel-to-webform-workflow.ts",
    "start_from_step": "fill-web-form"
  }
}
```

State is automatically restored from `.workflow_state/excel-to-webform-workflow.json`

## Visualization in mediar-app

The workflow returns metadata for UI visualization:

```json
{
  "metadata": {
    "name": "Excel to Web Form Data Entry",
    "description": "Automate data entry from CSV/Excel to web form",
    "version": "1.0.0",
    "input": {
      "type": "object",
      "properties": {
        "csvFile": {
          "type": "string",
          "default": "./sample-data.csv",
          "description": "Path to CSV file with data"
        },
        "startRow": {
          "type": "number",
          "default": 1,
          "description": "Row to start from (1-indexed)"
        }
      }
    },
    "steps": [
      {
        "id": "read-csv",
        "name": "Read CSV Data",
        "description": "Read and parse CSV file with form data"
      },
      {
        "id": "open-web-form",
        "name": "Open Web Form",
        "description": "Open browser and navigate to the web form"
      },
      {
        "id": "fill-web-form",
        "name": "Fill Web Form from Data",
        "description": "Iterate through CSV rows and fill web form"
      },
      {
        "id": "generate-report",
        "name": "Generate Summary Report",
        "description": "Create a summary report of the process"
      }
    ]
  }
}
```

**UI can render:**
- Input form with all parameters (auto-generated from Zod schema)
- Step list with progress indicators
- Current step being executed
- Error messages per step
- Success/failure counts

## Error Scenarios Handled

1. **CSV file not found** - Fails immediately with clear error
2. **Invalid CSV format** - Fails with parsing error details
3. **Web page fails to load** - Retries up to 2 times with 2s delay
4. **Form field not found** - Continues to next row, logs error
5. **Network issues** - Retry logic with exponential backoff

## Extending the Example

### Add Excel Support

Install xlsx library:

```bash
npm install xlsx
```

Update read-csv step:

```typescript
import * as XLSX from 'xlsx';

const workbook = XLSX.readFile(input.csvFile);
const sheetName = workbook.SheetNames[0];
const rows = XLSX.utils.sheet_to_json(workbook.Sheets[sheetName]);
```

### Add Screenshot on Error

```typescript
onError: async ({ error, desktop, logger }) => {
  // Take screenshot
  const screenshot = await desktop.captureScreen();
  await fs.writeFile(`error-${Date.now()}.png`, screenshot);
  logger.error('Screenshot saved');
}
```

### Add Form Submission

```typescript
// After filling all fields
const submitButton = await chromeWindow.locator('role:button|name:Submit').first(2000);
await submitButton.click();

// Wait for confirmation
await desktop.delay(2000);
```

### Process Multiple Forms

Add a loop to process different form URLs:

```typescript
const forms = [
  'https://example.com/form1',
  'https://example.com/form2',
];

for (const formUrl of forms) {
  await desktop.navigateBrowser(formUrl);
  // ... fill form
}
```

## Comparison with YAML Approach

### YAML Approach (Old)

**Problems:**
- Would require 40+ steps (one per field per row)
- No type safety for CSV data
- Hard to handle dynamic row counts
- No loop support (or very complex jump/goto logic)
- Error recovery requires manual state management

### TypeScript Approach (New)

**Benefits:**
- ✅ Single file, ~350 lines
- ✅ Full type safety (FormData interface)
- ✅ Natural for loops in code
- ✅ Easy error handling with try/catch
- ✅ Automatic state management
- ✅ IDE autocomplete and linting

## Next Steps

1. **Add your own CSV file** with real data
2. **Test with different web forms** (change `webFormUrl`)
3. **Add validation** for form submission
4. **Extend error recovery** for specific error types
5. **Add email notifications** on completion

## Questions?

See main documentation:
- `TYPESCRIPT_WORKFLOWS.md` - Overall approach
- `examples/typescript-workflow/README.md` - Getting started
- `REQUIREMENTS_COVERAGE.md` - Features and coverage
