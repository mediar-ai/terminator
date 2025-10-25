# TypeScript Workflow Examples

**Pure TypeScript. No YAML. Fully typed. AI-friendly.**

## Quick Start

```bash
npm install
npm run simple
```

## Examples

### 1. Simple Workflow (`simple-workflow.ts`)

Basic example showing the core pattern:

```typescript
import { createStep, createWorkflow } from '@mediar/terminator';
import { z } from 'zod';

const InputSchema = z.object({
  userName: z.string().default('World'),
});

const step1 = createStep({
  id: 'step1',
  name: 'Do Thing',
  execute: async ({ desktop, input }) => {
    // Your code here
  },
});

export default createWorkflow({
  name: 'My Workflow',
  input: InputSchema,
})
  .step(step1)
  .build();
```

**Run it:**
```bash
tsx simple-workflow.ts Alice
```

### 2. Production Workflow (`production-workflow.ts`)

Real-world example with:
- ✅ Error recovery and retry logic
- ✅ Error classification (permanent vs temporary)
- ✅ File management (processed/failed folders)
- ✅ Zod input validation
- ✅ Type-safe throughout

**Run it:**
```bash
tsx production-workflow.ts ./data.json
```

## Key Features

### Type-Safe Inputs (Zod)

```typescript
const InputSchema = z.object({
  jsonFile: z.string().describe('Path to JSON file'),
  maxRetries: z.number().default(3).min(0).max(10),
  sendEmail: z.boolean().default(true),
});

type Input = z.infer<typeof InputSchema>;
```

mediar-app reads the Zod schema and auto-generates a form UI!

### Error Recovery

```typescript
const step = createStep({
  execute: async ({ desktop }) => {
    // Main logic
  },

  onError: async ({ error, retry, attempt }) => {
    // Classify error
    if (isPermanent(error)) {
      return { recoverable: false };
    }

    // Retry with backoff
    if (attempt < maxRetries) {
      await wait(1000 * attempt);
      return retry();
    }
  },
});
```

### Context Sharing

```typescript
const step1 = createStep({
  execute: async ({ context }) => {
    const data = await loadData();
    context.data = data; // Share with next steps
  },
});

const step2 = createStep({
  execute: async ({ context }) => {
    const data = context.data; // Access from previous step
  },
});
```

## Benefits

### For Developers

✅ **Type Safety** - Full TypeScript autocomplete
✅ **Quick Feedback** - Linter shows errors instantly
✅ **Maintainable** - Refactor with confidence
✅ **Testable** - Test steps independently
✅ **Simple** - One file, no YAML

### For AI

✅ **AI-Friendly** - Standard TypeScript patterns
✅ **Instant Feedback** - LSP errors show immediately
✅ **Type Hints** - AI sees exact types
✅ **Parseable** - Standard AST parsing

### For mediar-app

✅ **Easy Parsing** - Standard TypeScript AST
✅ **Form Generation** - Auto-generate from Zod schema
✅ **Step Display** - Extract metadata from code
✅ **Execution** - Import and run directly

## How mediar-app Parses This

### Option 1: AST Parsing (Static)

```typescript
import { parse } from '@typescript-eslint/parser';

const ast = parse(workflowCode);
const metadata = extractMetadata(ast);

// Returns:
{
  name: 'My Workflow',
  input: { userName: { type: 'string', default: 'World' } },
  steps: [{ id: 'step1', name: 'Do Thing' }]
}
```

### Option 2: Runtime Execution

```typescript
const workflow = await import('./workflow.ts');
const metadata = workflow.default.getMetadata();
```

Both work! Start with Option 2 (simpler).

## Migration from YAML

### Before (YAML + JS):
```
workflow/
├── terminator.yaml (5MB, 2000+ lines)
├── classify_error.js
├── move_to_failed.js
└── ... 40+ files
```

### After (TypeScript):
```
workflow/
├── workflow.ts (single file, fully typed)
└── package.json
```

## Next Steps

1. **Implement SDK** - `createStep()`, `createWorkflow()`
2. **Build Parser** - Extract metadata from TypeScript
3. **UI Integration** - mediar-app renders and executes
4. **Ship Alpha!** - 2 weeks timeline

## Why This Approach?

> "normal TS project + createStep() + no yaml + typed + quick AI feedback"

This is exactly that. Simple, typed, maintainable, AI-friendly.

Let's ship it! 🚀
