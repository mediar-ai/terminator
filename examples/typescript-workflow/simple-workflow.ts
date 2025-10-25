#!/usr/bin/env tsx
/**
 * Simple TypeScript Workflow Example
 *
 * No YAML. Just TypeScript. Fully typed. AI-friendly.
 */

import { createStep, createWorkflow, z, type Desktop } from '../../packages/terminator-workflow/src';

// ============================================================================
// Input Schema (Using Zod)
// ============================================================================

const InputSchema = z.object({
  userName: z
    .string()
    .default('World')
    .describe('User name to greet'),

  includeDate: z
    .boolean()
    .default(true)
    .describe('Include current date in message'),
});

type Input = z.infer<typeof InputSchema>;

// ============================================================================
// Steps
// ============================================================================

const openNotepad = createStep({
  id: 'open-notepad',
  name: 'Open Notepad',
  description: 'Opens Notepad application',

  execute: async ({ desktop, logger }) => {
    logger.info('📝 Opening Notepad...');
    desktop.openApplication('notepad');
    await desktop.delay(2000);
  },
});

const typeGreeting = createStep({
  id: 'type-greeting',
  name: 'Type Greeting Message',
  description: 'Types personalized greeting',

  execute: async ({ desktop, input, logger }) => {
    logger.info(`👋 Typing greeting for ${input.userName}...`);

    const textbox = await desktop.locator('role:Edit').first(5000);
    textbox.typeText(`Hello, ${input.userName}!\n\n`);

    if (input.includeDate) {
      const date = new Date().toLocaleDateString();
      textbox.typeText(`Date: ${date}\n`);
    }
  },
});

// ============================================================================
// Workflow
// ============================================================================

const workflow = createWorkflow({
  name: 'Simple Notepad Demo',
  description: 'Opens Notepad and types a personalized greeting',
  version: '1.0.0',
  input: InputSchema,
})
  .step(openNotepad)
  .step(typeGreeting)
  .build();

// ============================================================================
// Execute (CLI)
// ============================================================================

if (require.main === module) {
  const input: Input = {
    userName: process.argv[2] || 'World',
    includeDate: true,
  };

  workflow.run(input).catch(error => {
    console.error('\n❌ Workflow execution failed');
    process.exit(1);
  });
}

export default workflow;
