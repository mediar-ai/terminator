#!/usr/bin/env tsx
/**
 * Excel to Web Form Data Entry Workflow
 *
 * This workflow demonstrates:
 * - Reading CSV/Excel data
 * - Opening Excel (or using CSV directly)
 * - Navigating to web form
 * - Automating data entry from spreadsheet to web form
 * - Error recovery for web form validation
 * - Processing multiple rows
 */

import { createStep, createWorkflow, z, type Desktop } from '../../packages/terminator-workflow/src';
import fs from 'fs/promises';
import path from 'path';

// ============================================================================
// Input Schema
// ============================================================================

const InputSchema = z.object({
  csvFile: z
    .string()
    .default('./sample-data.csv')
    .describe('Path to CSV file with data'),

  webFormUrl: z
    .string()
    .default('https://www.andrews.edu/~bidwell/examples/form.html')
    .describe('URL of the web form to fill'),

  startRow: z
    .number()
    .default(1)
    .min(1)
    .describe('Row to start from (1-indexed, skips header)'),

  endRow: z
    .number()
    .optional()
    .describe('Row to end at (optional, processes all if not specified)'),

  delayBetweenRows: z
    .number()
    .default(2000)
    .describe('Delay in ms between processing each row'),
});

type Input = z.infer<typeof InputSchema>;

interface FormData {
  FirstName: string;
  LastName: string;
  Title: string;
  Organization: string;
  Phone: string;
  Email: string;
  Address1: string;
  City: string;
  State: string;
  Zip: string;
}

// ============================================================================
// Step 1: Read and Parse CSV Data
// ============================================================================

const readCsvData = createStep({
  id: 'read-csv',
  name: 'Read CSV Data',
  description: 'Read and parse CSV file with form data',

  execute: async ({ input, context, logger }) => {
    logger.info(`📄 Reading CSV file: ${input.csvFile}`);

    const csvPath = path.resolve(input.csvFile);
    const content = await fs.readFile(csvPath, 'utf-8');

    // Parse CSV (simple parser - assumes no commas in values)
    const lines = content.trim().split('\n');
    const headers = lines[0].split(',').map(h => h.trim());

    logger.info(`📊 Headers: ${headers.join(', ')}`);

    const rows: FormData[] = [];
    for (let i = 1; i < lines.length; i++) {
      const values = lines[i].split(',').map(v => v.trim());
      const row: any = {};

      headers.forEach((header, index) => {
        row[header] = values[index] || '';
      });

      rows.push(row as FormData);
    }

    // Apply row filtering
    const startIdx = input.startRow - 1; // Convert to 0-indexed
    const endIdx = input.endRow ? input.endRow - 1 : rows.length - 1;

    const filteredRows = rows.slice(startIdx, endIdx + 1);

    logger.success(`✅ Loaded ${rows.length} total rows`);
    logger.info(`📍 Processing rows ${input.startRow} to ${endIdx + 1} (${filteredRows.length} rows)`);

    // Store in context
    context.data.rows = filteredRows;
    context.data.currentRowIndex = 0;

    return {
      totalRows: rows.length,
      rowsToProcess: filteredRows.length,
    };
  },

  onError: async ({ error, logger }) => {
    logger.error(`❌ Failed to read CSV: ${error.message}`);
    return { recoverable: false, reason: 'CSV file not found or invalid format' };
  },
});

// ============================================================================
// Step 2: Open Web Browser and Navigate to Form
// ============================================================================

const openWebForm = createStep({
  id: 'open-web-form',
  name: 'Open Web Form',
  description: 'Open browser and navigate to the web form',

  execute: async ({ desktop, input, logger }) => {
    logger.info(`🌐 Opening web form: ${input.webFormUrl}`);

    // Navigate to the form URL
    await desktop.navigateBrowser(input.webFormUrl, 'Chrome');

    logger.info('⏳ Waiting for page to load...');
    await desktop.delay(3000);

    // Wait for form to be ready (check for FirstName field)
    const formReady = await desktop
      .locator('role:Window|name:Chrome')
      .locator('role:textbox|name:First')
      .validate(5000);

    if (!formReady.exists) {
      throw new Error('Form not loaded - First Name field not found');
    }

    logger.success('✅ Web form loaded successfully');
  },

  onError: async ({ error, desktop, retry, attempt, logger }) => {
    logger.warn(`⚠️ Error opening form: ${error.message}`);

    // Retry up to 2 times
    if (attempt < 2) {
      logger.info('🔄 Retrying...');
      await desktop.delay(2000);
      return retry();
    }

    return { recoverable: false, reason: 'Failed to load web form after retries' };
  },
});

// ============================================================================
// Step 3: Process Each Row - Fill Web Form
// ============================================================================

const fillWebFormFromExcel = createStep({
  id: 'fill-web-form',
  name: 'Fill Web Form from Data',
  description: 'Iterate through CSV rows and fill web form for each',

  execute: async ({ desktop, input, context, logger }) => {
    const rows = context.data.rows as FormData[];
    const totalRows = rows.length;

    logger.info(`🔄 Processing ${totalRows} rows...`);

    const results = {
      succeeded: 0,
      failed: 0,
      errors: [] as Array<{ row: number; error: string }>,
    };

    for (let i = 0; i < rows.length; i++) {
      const rowData = rows[i];
      const rowNumber = input.startRow + i;

      logger.info(`\n📝 Row ${rowNumber}/${input.startRow + totalRows - 1}: ${rowData.FirstName} ${rowData.LastName}`);

      try {
        // Get the Chrome window
        const chromeWindow = await desktop.locator('role:Window|name:Chrome').first(5000);

        // Fill form fields - using the form's actual field names
        // Based on https://www.andrews.edu/~bidwell/examples/form.html

        // Title (radio buttons)
        logger.info('  Setting title...');
        if (rowData.Title === 'Dr.') {
          await chromeWindow.locator('role:RadioButton|name:Dr.').first(1000).setToggled(true);
        } else if (rowData.Title === 'Prof.') {
          await chromeWindow.locator('role:RadioButton|name:Prof.').first(1000).setToggled(true);
        } else if (rowData.Title === 'Mr.') {
          await chromeWindow.locator('role:RadioButton|name:Mr.').first(1000).setToggled(true);
        } else if (rowData.Title === 'Ms.') {
          await chromeWindow.locator('role:RadioButton|name:Ms.').first(1000).setToggled(true);
        }

        // First Name
        logger.info('  Filling First Name...');
        const firstNameField = await chromeWindow.locator('role:textbox|name:First').first(2000);
        await firstNameField.setValue(rowData.FirstName);

        // Last Name
        logger.info('  Filling Last Name...');
        const lastNameField = await chromeWindow.locator('role:textbox|name:Last').first(2000);
        await lastNameField.setValue(rowData.LastName);

        // Organization
        logger.info('  Filling Organization...');
        const orgField = await chromeWindow.locator('role:textbox|name:Organization').first(2000);
        await orgField.setValue(rowData.Organization);

        // Phone
        logger.info('  Filling Phone...');
        const phoneField = await chromeWindow.locator('role:textbox|name:Phone').first(2000);
        await phoneField.setValue(rowData.Phone);

        // Email
        logger.info('  Filling Email...');
        const emailField = await chromeWindow.locator('role:textbox|name:E-mail').first(2000);
        await emailField.setValue(rowData.Email);

        // Address
        logger.info('  Filling Address...');
        const addressField = await chromeWindow.locator('role:textbox|name:Street Address').first(2000);
        await addressField.setValue(rowData.Address1);

        // City
        logger.info('  Filling City...');
        const cityField = await chromeWindow.locator('role:textbox|name:City').first(2000);
        await cityField.setValue(rowData.City);

        // State
        logger.info('  Filling State...');
        const stateField = await chromeWindow.locator('role:textbox|name:State').first(2000);
        await stateField.setValue(rowData.State);

        // Zip
        logger.info('  Filling Zip...');
        const zipField = await chromeWindow.locator('role:textbox|name:Zip').first(2000);
        await zipField.setValue(rowData.Zip);

        logger.success(`  ✅ Row ${rowNumber} completed successfully`);
        results.succeeded++;

        // Reset form for next entry (if not last row)
        if (i < rows.length - 1) {
          logger.info('  🔄 Resetting form for next entry...');

          // Look for Reset button
          const resetButton = await chromeWindow.locator('role:button|name:Reset').first(2000);
          await resetButton.click();

          // Wait before next row
          await desktop.delay(input.delayBetweenRows);
        }

      } catch (error: any) {
        logger.error(`  ❌ Failed to process row ${rowNumber}: ${error.message}`);
        results.failed++;
        results.errors.push({
          row: rowNumber,
          error: error.message,
        });

        // Continue to next row even if this one failed
        continue;
      }
    }

    logger.info('\n' + '='.repeat(60));
    logger.success(`📊 Processing complete!`);
    logger.info(`   ✅ Succeeded: ${results.succeeded}`);
    logger.info(`   ❌ Failed: ${results.failed}`);
    logger.info('='.repeat(60));

    context.data.results = results;

    return results;
  },

  onError: async ({ error, logger }) => {
    logger.error(`❌ Critical error during form filling: ${error.message}`);
    return { recoverable: false, reason: error.message };
  },
});

// ============================================================================
// Step 4: Generate Summary Report
// ============================================================================

const generateReport = createStep({
  id: 'generate-report',
  name: 'Generate Summary Report',
  description: 'Create a summary report of the data entry process',

  execute: async ({ input, context, logger }) => {
    logger.info('📝 Generating summary report...');

    const results = context.data.results;
    const timestamp = new Date().toISOString();

    const report = {
      timestamp,
      csvFile: input.csvFile,
      webFormUrl: input.webFormUrl,
      rowsProcessed: results.succeeded + results.failed,
      succeeded: results.succeeded,
      failed: results.failed,
      successRate: `${((results.succeeded / (results.succeeded + results.failed)) * 100).toFixed(1)}%`,
      errors: results.errors,
    };

    // Save report to file
    const reportPath = path.resolve('./data-entry-report.json');
    await fs.writeFile(reportPath, JSON.stringify(report, null, 2));

    logger.success(`✅ Report saved to: ${reportPath}`);

    return report;
  },
});

// ============================================================================
// Workflow Definition
// ============================================================================

export default createWorkflow({
  name: 'Excel to Web Form Data Entry',
  description: 'Automate data entry from CSV/Excel to web form with error handling',
  version: '1.0.0',
  input: InputSchema,
})
  .step(readCsvData)
  .step(openWebForm)
  .step(fillWebFormFromExcel)
  .step(generateReport)

  // Success handler
  .onSuccess(async ({ logger, context }) => {
    const results = context.data.results;
    logger.success('\n🎉 Workflow completed successfully!');
    logger.info(`   Processed ${results.succeeded} rows successfully`);

    if (results.failed > 0) {
      logger.warn(`   ⚠️  ${results.failed} rows failed`);
    }
  })

  // Error handler
  .onError(async ({ error, step, logger }) => {
    logger.error('\n💥 Workflow failed!');
    logger.error(`   Failed at step: ${step.name}`);
    logger.error(`   Error: ${error.message}`);
  })

  .build();

// ============================================================================
// CLI Execution
// ============================================================================

if (require.main === module) {
  const input: Input = {
    csvFile: process.argv[2] || './sample-data.csv',
    webFormUrl: 'https://www.andrews.edu/~bidwell/examples/form.html',
    startRow: parseInt(process.argv[3] || '1'),
    endRow: process.argv[4] ? parseInt(process.argv[4]) : undefined,
    delayBetweenRows: 2000,
  };

  console.log('🚀 Starting Excel to Web Form Data Entry Workflow');
  console.log('='.repeat(60));
  console.log('Input:', input);
  console.log('='.repeat(60));
  console.log('');

  workflow.run(input).catch(error => {
    console.error('\n❌ Workflow execution failed');
    console.error(error);
    process.exit(1);
  });
}
