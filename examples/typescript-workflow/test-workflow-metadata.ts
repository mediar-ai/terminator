#!/usr/bin/env tsx
/**
 * Test script to verify workflow metadata extraction
 */

async function main() {
  console.log('Loading workflow...');
  const workflow = await import('./excel-to-webform-workflow');

  console.log('\n✅ Workflow loaded successfully!');

  const metadata = workflow.default.getMetadata();

  console.log('\nWorkflow Metadata:');
  console.log('==================');
  console.log(JSON.stringify(metadata, null, 2));

  console.log('\n✅ Metadata extraction successful!');
  console.log(`\nSteps: ${metadata.steps.length}`);
  metadata.steps.forEach((step: any, i: number) => {
    console.log(`  ${i + 1}. ${step.name} (${step.id})`);
  });
}

main().catch(error => {
  console.error('❌ Error:', error);
  process.exit(1);
});
