# TypeScript Workflows - The Best Approach

## TL;DR

**Just TypeScript. No YAML. Simple `createStep()` API.**

```typescript
import { createStep, createWorkflow } from '@mediar/terminator';

const login = createStep({
  id: 'login',
  name: 'Login to SAP',
  execute: async ({ desktop, input }) => {
    await desktop.locator('role:textbox|name:Username').fill(input.username);
    await desktop.locator('role:button|name:Login').click();
    return { sessionId: 'abc123' };
  },
  onError: async ({ error, desktop, retry }) => {
    if (error.message.includes('Session conflict')) {
      await desktop.locator('role:button|name:Close Session').click();
      return retry();
    }
  }
});

export default createWorkflow({
  name: 'SAP Login',
  input: z.object({
    username: z.string(),
    password: z.string(),
  }),
})
  .step(login)
  .build();
```

## Why This Approach?

### ✅ What You Said
> "normal TS project + createStep() function that we can easily parse using JS parser libs + no more yaml file - variables, inputs/whatever is defined in the typescript as types, with defaults + everything becomes typed, reliable, quick feedback for AI with linter"

### ✅ Benefits

1. **Type Safety**
   - Full TypeScript autocomplete
   - Compile-time error checking
   - AI gets instant feedback from LSP

2. **Parseable**
   - Use JS parser (babel, swc, ts-morph)
   - Extract metadata from AST
   - mediar-app can parse and render UI

3. **Simple**
   - One file per workflow
   - No YAML to sync
   - No config files

4. **AI-Friendly**
   - Linter gives immediate feedback
   - Type errors show up instantly
   - Standard TypeScript patterns

5. **Maintainable**
   - Refactor with confidence
   - Find all references
   - Rename symbols safely

### ❌ Why Not YAML?

- ❌ No type safety
- ❌ YAML syntax errors
- ❌ Files can get out of sync
- ❌ No autocomplete
- ❌ No refactoring support
- ❌ AI doesn't get quick feedback

## How mediar-app Parses Workflows

### Option 1: Static Analysis (AST Parsing)

```typescript
// mediar-app parses workflow.ts
import { parse } from '@typescript-eslint/parser';

const ast = parse(workflowCode);

// Extract metadata from AST
const workflowConfig = {
  name: findStringLiteral(ast, 'name'),
  steps: findStepCalls(ast),
  input: parseZodSchema(ast),
};

// Render UI
<WorkflowForm config={workflowConfig} />
```

**Libraries:**
- `@typescript-eslint/parser` - Parse TypeScript
- `ts-morph` - TypeScript AST manipulation
- `@babel/parser` - Babel parser

### Option 2: Runtime Execution

```typescript
// mediar-app executes workflow to extract metadata
const workflow = await import('./workflow.ts');
const metadata = workflow.default.getMetadata();

// Render UI
<WorkflowForm metadata={metadata} />
```

**Both work!** Start with Option 2 (simpler), add Option 1 later.

## Core API

### `createStep()`

```typescript
interface StepConfig<TInput, TOutput> {
  id: string;
  name: string;
  description?: string;

  execute: (context: {
    desktop: Desktop;
    input: TInput;
    context: WorkflowContext;
    logger: Logger;
  }) => Promise<TOutput>;

  onError?: (context: {
    error: Error;
    desktop: Desktop;
    retry: () => Promise<TOutput>;
    attempt: number;
  }) => Promise<{ recoverable: boolean } | void>;

  timeout?: number;
}

function createStep<TInput, TOutput>(
  config: StepConfig<TInput, TOutput>
): Step<TInput, TOutput>;
```

### `createWorkflow()`

```typescript
interface WorkflowConfig<TInput> {
  name: string;
  description?: string;
  version?: string;
  input: z.ZodSchema<TInput>;
}

function createWorkflow<TInput>(
  config: WorkflowConfig<TInput>
): WorkflowBuilder<TInput>;
```

### Type-Safe Input Schema (Using Zod)

```typescript
import { z } from 'zod';

const workflow = createWorkflow({
  name: 'Process Invoice',

  // Input schema using Zod
  input: z.object({
    jsonFile: z.string().describe('Path to JSON file'),
    sendEmail: z.boolean().default(true).describe('Send notification email'),
    retries: z.number().default(3).min(0).max(10),
  }),
})
  .step(processData)
  .build();

// TypeScript knows the exact shape!
workflow.run({
  jsonFile: './data.json',
  sendEmail: true,
  retries: 3,
});
```

**mediar-app reads Zod schema** → Generates form UI automatically!

## Simple Example

```typescript
// workflow.ts
import { createStep, createWorkflow, Desktop } from '@mediar/terminator';
import { z } from 'zod';

// Define input type
const InputSchema = z.object({
  userName: z.string().default('World').describe('User name to greet'),
  includeDate: z.boolean().default(true).describe('Include current date'),
});

type Input = z.infer<typeof InputSchema>;

// Step 1
const openNotepad = createStep({
  id: 'open-notepad',
  name: 'Open Notepad',

  execute: async ({ desktop, logger }) => {
    logger.info('📝 Opening Notepad...');
    desktop.openApplication('notepad');
    await desktop.wait(2000);
  },
});

// Step 2
const typeGreeting = createStep({
  id: 'type-greeting',
  name: 'Type Greeting',

  execute: async ({ desktop, input, logger }: { desktop: Desktop; input: Input; logger: any }) => {
    logger.info(`👋 Typing greeting for ${input.userName}...`);

    const textbox = desktop.locator('role:Edit');
    await textbox.type(`Hello, ${input.userName}!\n`);

    if (input.includeDate) {
      await textbox.type(`Date: ${new Date().toLocaleDateString()}\n`);
    }
  },
});

// Workflow
export default createWorkflow({
  name: 'Simple Notepad Demo',
  description: 'Opens Notepad and types a greeting',
  version: '1.0.0',
  input: InputSchema,
})
  .step(openNotepad)
  .step(typeGreeting)
  .build();

// Execute
if (require.main === module) {
  const input = {
    userName: process.argv[2] || 'World',
    includeDate: true,
  };

  workflow.run(input);
}
```

**That's it!** One file. Fully typed. AI-friendly. Parseable.

## Production Example with Error Recovery

```typescript
import { createStep, createWorkflow } from '@mediar/terminator';
import { z } from 'zod';
import fs from 'fs/promises';

const InputSchema = z.object({
  jsonFile: z.string().describe('JSON file to process'),
  maxRetries: z.number().default(3).describe('Max retry attempts'),
});

type Input = z.infer<typeof InputSchema>;

const processFile = createStep({
  id: 'process-file',
  name: 'Process JSON File',

  execute: async ({ input, context, logger }) => {
    const content = await fs.readFile(input.jsonFile, 'utf-8');
    const data = JSON.parse(content);

    // Validate
    if (!data.outlet_code || !data.entries) {
      throw new Error('Invalid JSON structure');
    }

    context.data = data;
    return { entriesCount: data.entries.length };
  },

  // Error classification
  onError: async ({ error, retry, attempt, input }) => {
    // Permanent errors - don't retry
    if (error.message.includes('Invalid JSON') ||
        error instanceof SyntaxError) {
      return { recoverable: false };
    }

    // Temporary errors - retry
    if (attempt < input.maxRetries) {
      await new Promise(r => setTimeout(r, 1000 * attempt)); // Backoff
      return retry();
    }

    return { recoverable: false };
  },
});

const fillForm = createStep({
  id: 'fill-form',
  name: 'Fill SAP Form',

  execute: async ({ desktop, context, logger }) => {
    const data = context.data;

    for (const entry of data.entries) {
      await desktop.locator(`role:cell[account]`).fill(entry.account);
      await desktop.locator(`role:cell[debit]`).fill(entry.debit);
    }
  },

  onError: async ({ error, desktop, retry }) => {
    // Check if popup blocking us
    const popup = desktop.locator('role:dialog');
    if (await popup.exists()) {
      await popup.locator('role:button|name:Close').click();
      return retry();
    }
  },
});

export default createWorkflow({
  name: 'SAP Journal Entry',
  description: 'Process JSON and fill SAP form',
  input: InputSchema,
})
  .step(processFile)
  .step(fillForm)
  .onSuccess(async ({ input, logger }) => {
    // Move to processed
    const processed = input.jsonFile.replace('.json', '_processed.json');
    await fs.rename(input.jsonFile, processed);
    logger.success(`✅ Moved to ${processed}`);
  })
  .onError(async ({ error, step, input, logger }) => {
    // Move to failed
    const failed = input.jsonFile.replace('.json', '_failed.json');
    await fs.rename(input.jsonFile, failed);

    // Write error metadata
    await fs.writeFile(failed + '.meta.json', JSON.stringify({
      error: error.message,
      step: step.id,
      timestamp: new Date().toISOString(),
    }));

    logger.error(`❌ Moved to ${failed}`);
  })
  .build();
```

## How mediar-app Renders UI

### 1. Parse Workflow File

```typescript
// mediar-app backend
import { parseWorkflow } from '@mediar/workflow-parser';

const metadata = parseWorkflow('./workflow.ts');

// Returns:
{
  name: 'SAP Journal Entry',
  description: 'Process JSON and fill SAP form',
  input: {
    jsonFile: { type: 'string', description: 'JSON file to process' },
    maxRetries: { type: 'number', default: 3, description: 'Max retry attempts' },
  },
  steps: [
    { id: 'process-file', name: 'Process JSON File' },
    { id: 'fill-form', name: 'Fill SAP Form' },
  ],
}
```

### 2. Render UI

```tsx
// mediar-app frontend
function WorkflowPage({ metadata }) {
  return (
    <div>
      <h1>{metadata.name}</h1>
      <p>{metadata.description}</p>

      {/* Auto-generated form from Zod schema */}
      <Form schema={metadata.input} onSubmit={runWorkflow}>
        <Input name="jsonFile" label="JSON File" type="string" />
        <Input name="maxRetries" label="Max Retries" type="number" default={3} />
        <Button>Run Workflow</Button>
      </Form>

      {/* Steps display */}
      <StepList steps={metadata.steps} />
    </div>
  );
}
```

### 3. Execute Workflow

```typescript
// mediar-app backend
async function runWorkflow(workflowPath: string, input: any) {
  const workflow = await import(workflowPath);
  await workflow.default.run(input);
}
```

## Parser Implementation

```typescript
// @mediar/workflow-parser
import { parse } from '@typescript-eslint/parser';
import { zodToJsonSchema } from 'zod-to-json-schema';

export function parseWorkflow(filePath: string) {
  const code = fs.readFileSync(filePath, 'utf-8');
  const ast = parse(code);

  // Find createWorkflow() call
  const workflowCall = findWorkflowCall(ast);

  // Extract metadata
  const metadata = {
    name: extractStringLiteral(workflowCall, 'name'),
    description: extractStringLiteral(workflowCall, 'description'),
    input: extractZodSchema(workflowCall, 'input'),
    steps: extractSteps(ast),
  };

  return metadata;
}
```

## Migration Path

### Current (YAML + many files):
```
workflow/
├── terminator.yaml (5MB!)
├── classify_error.js
├── move_to_failed.js
├── check_duplicate.js
└── ... 40+ files
```

### New (Single TypeScript file):
```
workflow/
├── workflow.ts (all logic in one file)
└── package.json
```

**Migration:** Convert each .js file to a `createStep()`, compose in one workflow.

## Recommendation

### Start Here (MVP - Next 2 Weeks)

**Implement:**
1. `createStep()` API
2. `createWorkflow()` API
3. Zod input schemas
4. Simple parser (AST or runtime)
5. mediar-app integration

**Don't implement:**
- ❌ YAML support
- ❌ Multiple file formats
- ❌ Complex abstractions

### Ship Alpha With This

**Benefits:**
- ✅ Simple, maintainable
- ✅ AI-friendly (type safety + linter)
- ✅ One source of truth
- ✅ Easy to parse for UI
- ✅ Production-ready patterns

**Timeline:** 2 weeks is doable!

Week 1: SDK implementation
Week 2: mediar-app integration + parser

## Conclusion

**Your instinct is 100% correct:**

> "normal TS project + createStep() + no yaml + typed + quick AI feedback"

This is the best approach. Simple, typed, maintainable, AI-friendly.

Let's ship it! 🚀
