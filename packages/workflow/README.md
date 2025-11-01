# @mediar-ai/workflow

TypeScript SDK for building Terminator workflows with type safety, error recovery, and easy parsing for mediar-app UI.

## Installation

```bash
npm install @mediar-ai/workflow zod
```

## Quick Start

```typescript
import { createStep, createWorkflow, z } from '@mediar-ai/workflow';

// Define input schema
const InputSchema = z.object({
  userName: z.string().default('World'),
});

// Create steps
const openApp = createStep({
  id: 'open-app',
  name: 'Open Notepad',
  execute: async ({ desktop }) => {
    desktop.openApplication('notepad');
    await desktop.wait(2000);
  },
});

const typeGreeting = createStep({
  id: 'type-greeting',
  name: 'Type Greeting',
  execute: async ({ desktop, input }) => {
    const textbox = desktop.locator('role:Edit');
    await textbox.type(`Hello, ${input.userName}!`);
  },
});

// Create workflow
const workflow = createWorkflow({
  name: 'Simple Demo',
  input: InputSchema,
})
  .step(openApp)
  .step(typeGreeting)
  .build();

// Run it
workflow.run({ userName: 'Alice' });
```

## Features

### ✅ Type Safety

Full TypeScript support with Zod schemas:

```typescript
const InputSchema = z.object({
  jsonFile: z.string().describe('Path to JSON file'),
  maxRetries: z.number().default(3).min(0).max(10),
  sendEmail: z.boolean().default(true),
});

type Input = z.infer<typeof InputSchema>; // Fully typed!
```

### ✅ Error Recovery

Built-in error recovery and retry logic:

```typescript
const step = createStep({
  execute: async ({ desktop }) => {
    // Your logic
  },
  onError: async ({ error, retry, attempt }) => {
    if (error.message.includes('temporary')) {
      await new Promise(r => setTimeout(r, 1000 * attempt));
      return retry();
    }
    return { recoverable: false };
  },
});
```

### ✅ Context Sharing

Share data between steps:

```typescript
const step1 = createStep({
  execute: async ({ context }) => {
    context.data = { userId: 123 };
  },
});

const step2 = createStep({
  execute: async ({ context }) => {
    console.log(context.data.userId); // 123
  },
});
```

### ✅ Conditional Execution

Steps run conditionally:

```typescript
const step = createStep({
  condition: ({ input }) => input.sendEmail === true,
  execute: async ({ desktop }) => {
    // Only runs if sendEmail is true
  },
});
```

### ✅ Success/Error Handlers

Workflow-level handlers:

```typescript
const workflow = createWorkflow({ ... })
  .step(step1)
  .onSuccess(async ({ logger }) => {
    logger.success('All done!');
  })
  .onError(async ({ error, step }) => {
    console.error(`Failed at: ${step.config.name}`);
  })
  .build();
```

## API Reference

### `createStep(config)`

Creates a workflow step.

**Parameters:**
- `config.id` - Unique step identifier
- `config.name` - Human-readable name
- `config.description` - Optional description
- `config.execute` - Main execution function
- `config.onError` - Optional error recovery function
- `config.timeout` - Optional timeout in ms
- `config.condition` - Optional condition function

### `createWorkflow(config)`

Creates a workflow builder.

**Parameters:**
- `config.name` - Workflow name
- `config.description` - Optional description
- `config.version` - Optional version
- `config.input` - Zod input schema

**Methods:**
- `.step(step)` - Add a step
- `.onSuccess(handler)` - Set success handler
- `.onError(handler)` - Set error handler
- `.build()` - Build the workflow

## Examples

See `examples/` for complete examples:

- `meeting_notes_workflow.ts` - Complete real-world Windows-only example demonstrating:
  - Multi-step automation (open Notepad, type notes, save file, screenshot)
  - Context sharing between steps
  - Conditional execution
  - Error recovery with retry logic
  - Type safety with Zod schemas

To run the example:
```bash
cd packages/workflow
npx ts-node examples/meeting_notes_workflow.ts
```

## License

MIT
