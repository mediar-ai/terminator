# Quick Start - TypeScript Workflows

## 30-Second Overview

**Before (YAML):** 40+ files, no type safety, hard to maintain
**After (TypeScript):** 1 file, fully typed, AI-friendly

## Run the Examples

```bash
cd examples/typescript-workflow

# Install dependencies
npm install

# Run simple example (Notepad automation)
tsx simple-workflow.ts

# Run production example (SAP-style workflow)
tsx production-workflow.ts

# Run Excel to web form example
tsx excel-to-webform-workflow.ts
```

## Create Your First Workflow

```typescript
import { createStep, createWorkflow, z } from '@mediar/terminator-workflow';

// 1. Define inputs (type-safe with Zod)
const InputSchema = z.object({
  userName: z.string().default('World'),
});

// 2. Create steps
const step1 = createStep({
  id: 'greet',
  name: 'Say Hello',
  execute: async ({ input, logger }) => {
    logger.info(`Hello, ${input.userName}!`);
  }
});

// 3. Compose workflow
export default createWorkflow({
  name: 'My First Workflow',
  input: InputSchema,
})
  .step(step1)
  .build();
```

## Key Features

### Type Safety
```typescript
interface FormData {
  firstName: string;
  lastName: string;
}

// TypeScript catches typos!
row.firstName  // ✅ Works
row.fisrtName  // ❌ Error: Property 'fisrtName' does not exist
```

### Error Recovery
```typescript
onError: async ({ error, retry, attempt }) => {
  if (attempt < 3) {
    await desktop.delay(1000);
    return retry();
  }
  return { recoverable: false };
}
```

### Context Sharing
```typescript
// Step 1: Store data
context.data.users = users;

// Step 2: Access data
const users = context.data.users;
```

## Use with mediar-app

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "url": "file://./workflow.ts",
    "inputs": {
      "userName": "Alice"
    }
  }
}
```

## Debug at Any Step

```json
{
  "tool_name": "execute_sequence",
  "arguments": {
    "url": "file://./workflow.ts",
    "start_from_step": "step-3",
    "end_at_step": "step-5"
  }
}
```

State is automatically saved and restored from `.workflow_state/`.

## Next Steps

1. **Read:** `TYPESCRIPT_WORKFLOWS.md` - Core concepts
2. **Try:** `excel-to-webform-workflow.ts` - Real-world example
3. **Learn:** `EXCEL_WEBFORM_EXAMPLE.md` - Detailed walkthrough
4. **Build:** Your own workflow!

## Questions?

- **Documentation:** `examples/typescript-workflow/README.md`
- **Requirements:** `REQUIREMENTS_COVERAGE.md`
- **API Reference:** `packages/terminator-workflow/src/types.ts`
