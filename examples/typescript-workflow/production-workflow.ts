#!/usr/bin/env tsx
/**
 * Production TypeScript Workflow Example
 *
 * Demonstrates:
 * - Type-safe inputs with Zod
 * - Error recovery and retry logic
 * - File management (processed/failed folders)
 * - Error classification
 * - AI-friendly structure
 */

import { createStep, createWorkflow, Desktop } from '@mediar/terminator';
import { z } from 'zod';
import fs from 'fs/promises';
import path from 'path';

// ============================================================================
// Input Schema
// ============================================================================

const InputSchema = z.object({
  jsonFile: z
    .string()
    .describe('Path to JSON file to process'),

  maxRetries: z
    .number()
    .default(3)
    .min(0)
    .max(10)
    .describe('Maximum retry attempts for temporary errors'),

  sendEmail: z
    .boolean()
    .default(true)
    .describe('Send notification email on completion'),
});

type Input = z.infer<typeof InputSchema>;

// ============================================================================
// Step 1: Read and Validate JSON
// ============================================================================

const readJsonFile = createStep({
  id: 'read-json',
  name: 'Read JSON File',
  description: 'Read and validate JSON file structure',

  execute: async ({ input, context, logger }) => {
    logger.info(`📄 Reading file: ${input.jsonFile}`);

    const content = await fs.readFile(input.jsonFile, 'utf-8');
    const data = JSON.parse(content);

    // Validate structure
    if (!data.outlet_code || !data.entries) {
      throw new Error('VALIDATION_ERROR: Missing required fields (outlet_code, entries)');
    }

    // Store in context for next steps
    context.data = data;

    logger.success(`✅ Loaded ${data.entries.length} entries for outlet ${data.outlet_code}`);

    return {
      entriesCount: data.entries.length,
      outletCode: data.outlet_code,
    };
  },

  // Error recovery
  onError: async ({ error, retry, attempt, input, logger }) => {
    logger.warn(`⚠️ Error reading file: ${error.message}`);

    // Classify error
    const isPermanent =
      error instanceof SyntaxError || // JSON parse error
      error.message.includes('VALIDATION_ERROR') || // Validation failed
      error.message.includes('ENOENT'); // File not found

    if (isPermanent) {
      logger.error('❌ Permanent error - cannot recover');
      return { recoverable: false, reason: error.message };
    }

    // Retry for temporary errors
    if (attempt < input.maxRetries) {
      logger.info(`🔄 Retrying (attempt ${attempt + 1}/${input.maxRetries})...`);
      await new Promise(r => setTimeout(r, 1000 * attempt)); // Exponential backoff
      return retry();
    }

    logger.error('❌ Max retries exceeded');
    return { recoverable: false, reason: 'Max retries exceeded' };
  },
});

// ============================================================================
// Step 2: Check for Duplicates
// ============================================================================

const checkDuplicates = createStep({
  id: 'check-duplicates',
  name: 'Check for Duplicates',
  description: 'Verify this entry hasn\'t been processed before',

  execute: async ({ input, context, logger }) => {
    logger.info('🔍 Checking for duplicates...');

    const data = context.data;
    const uniqueKey = `${data.outlet_code}_${data.date}_${data.time_period}`;

    // Check processed folder
    const dir = path.dirname(input.jsonFile);
    const processedDir = path.join(dir, 'processed');

    try {
      const files = await fs.readdir(processedDir);

      for (const file of files) {
        if (file.includes(uniqueKey)) {
          throw new Error(`DUPLICATE_ERROR: Entry already processed (${file})`);
        }
      }

      logger.success('✅ No duplicates found');
    } catch (error: any) {
      if (error.message.includes('DUPLICATE_ERROR')) {
        throw error;
      }
      // Processed folder doesn't exist - that's OK
      logger.info('📁 Processed folder not found (first run)');
    }
  },

  onError: async ({ error, logger }) => {
    if (error.message.includes('DUPLICATE_ERROR')) {
      logger.error('❌ Duplicate entry detected');
      return { recoverable: false, reason: 'Duplicate entry' };
    }
  },
});

// ============================================================================
// Step 3: Fill SAP Form
// ============================================================================

const fillSapForm = createStep({
  id: 'fill-sap-form',
  name: 'Fill SAP Form',
  description: 'Fill journal entry form in SAP',

  execute: async ({ desktop, context, logger }) => {
    const data = context.data;

    logger.info(`📝 Filling form for ${data.outlet_code}...`);

    // Fill header
    await desktop.locator('role:textbox|name:Outlet').fill(data.outlet_code);
    await desktop.locator('role:textbox|name:Date').fill(data.date);

    // Fill entries
    for (let i = 0; i < data.entries.length; i++) {
      const entry = data.entries[i];

      logger.info(`  Row ${i + 1}/${data.entries.length}: ${entry.account}`);

      await desktop.locator(`role:cell[row:${i}][col:account]`).fill(entry.account);
      await desktop.locator(`role:cell[row:${i}][col:debit]`).fill(entry.debit || '');
      await desktop.locator(`role:cell[row:${i}][col:credit]`).fill(entry.credit || '');
    }

    logger.success(`✅ Filled ${data.entries.length} entries`);
  },

  onError: async ({ error, desktop, retry, logger }) => {
    logger.warn(`⚠️ Error filling form: ${error.message}`);

    // Check for popups blocking us
    const popup = desktop.locator('role:dialog');
    if (await popup.exists()) {
      logger.info('🔧 Closing popup and retrying...');
      await popup.locator('role:button|name:Close').click();
      await desktop.wait(1000);
      return retry();
    }

    // Check for session conflict
    if (error.message.includes('Session conflict')) {
      logger.info('🔧 Handling session conflict...');
      await desktop.locator('role:button|name:Close Other Session').click();
      await desktop.wait(2000);
      return retry();
    }
  },
});

// ============================================================================
// Step 4: Verify and Submit
// ============================================================================

const submitForm = createStep({
  id: 'submit-form',
  name: 'Submit Form',
  description: 'Verify totals and submit form',

  execute: async ({ desktop, logger }) => {
    logger.info('🔍 Verifying totals...');

    const debitTotal = await desktop.locator('role:cell|name:Debit Total').text();
    const creditTotal = await desktop.locator('role:cell|name:Credit Total').text();

    const debit = parseFloat(debitTotal);
    const credit = parseFloat(creditTotal);

    if (Math.abs(debit - credit) > 0.01) {
      throw new Error(`BALANCE_ERROR: Debit (${debit}) != Credit (${credit})`);
    }

    logger.info('💾 Submitting form...');
    await desktop.locator('role:button|name:Submit').click();

    // Wait for confirmation
    await desktop.locator('text:Successfully saved').waitFor({ timeout: 10000 });

    logger.success('✅ Form submitted successfully');
  },
});

// ============================================================================
// Workflow Definition
// ============================================================================

export default createWorkflow({
  name: 'SAP Journal Entry Processing',
  description: 'Process JSON files and create journal entries in SAP B1',
  version: '1.0.0',

  input: InputSchema,
})
  .step(readJsonFile)
  .step(checkDuplicates)
  .step(fillSapForm)
  .step(submitForm)

  // Success handler - move to processed
  .onSuccess(async ({ input, logger }) => {
    logger.success('🎉 Workflow completed successfully!');

    const fileName = path.basename(input.jsonFile);
    const dir = path.dirname(input.jsonFile);
    const processedPath = path.join(dir, 'processed', fileName);

    await fs.mkdir(path.dirname(processedPath), { recursive: true });
    await fs.rename(input.jsonFile, processedPath);

    logger.info(`📁 Moved to processed: ${processedPath}`);
  })

  // Error handler - move to failed
  .onError(async ({ error, step, input, logger }) => {
    logger.error(`❌ Workflow failed at step: ${step.name}`);
    logger.error(`   Error: ${error.message}`);

    const fileName = path.basename(input.jsonFile);
    const dir = path.dirname(input.jsonFile);
    const failedPath = path.join(dir, 'failed', fileName);

    await fs.mkdir(path.dirname(failedPath), { recursive: true });
    await fs.rename(input.jsonFile, failedPath);

    // Write error metadata
    await fs.writeFile(failedPath + '.meta.json', JSON.stringify({
      error: error.message,
      step: step.id,
      timestamp: new Date().toISOString(),
      stack: error.stack,
    }, null, 2));

    logger.error(`📁 Moved to failed: ${failedPath}`);
  })

  .build();

// ============================================================================
// Execute (CLI)
// ============================================================================

if (require.main === module) {
  const input: Input = {
    jsonFile: process.argv[2] || './data.json',
    maxRetries: 3,
    sendEmail: true,
  };

  console.log('🚀 Starting SAP Journal Entry workflow...');
  console.log('Input:', input);
  console.log('='.repeat(60));

  workflow.run(input).catch(error => {
    console.error('\n❌ Workflow execution failed');
    process.exit(1);
  });
}
