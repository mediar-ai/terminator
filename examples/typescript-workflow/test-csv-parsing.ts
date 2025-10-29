#!/usr/bin/env tsx
/**
 * Test the CSV parsing step independently
 */

import { createWorkflowRunner } from '../../packages/terminator-workflow/src/runner';

async function main() {
  console.log('🧪 Testing CSV Parsing Step...\n');

  const workflow = await import('./excel-to-webform-workflow');

  console.log('Creating workflow runner...');
  const runner = createWorkflowRunner({
    workflow: workflow.default,
    inputs: {
      csvFile: './sample-data.csv',
      webFormUrl: 'https://www.andrews.edu/~bidwell/examples/form.html',
      startRow: 1,
      endRow: 2, // Only process 2 rows for testing
      delayBetweenRows: 100,
    },
    startFromStep: 'read-csv',
    endAtStep: 'read-csv', // Only run the CSV parsing step
  });

  console.log('Running CSV parsing step...\n');
  const result = await runner.run();

  console.log('\n📊 Result:');
  console.log(JSON.stringify(result, null, 2));

  const state = runner.getState();
  console.log('\n📦 State:');
  console.log(`  Rows loaded: ${state.context.data.rows?.length || 0}`);
  if (state.context.data.rows && state.context.data.rows.length > 0) {
    console.log('  First row:', state.context.data.rows[0]);
  }

  console.log('\n✅ CSV parsing test completed successfully!');
}

main().catch(error => {
  console.error('❌ Test failed:', error);
  process.exit(1);
});
