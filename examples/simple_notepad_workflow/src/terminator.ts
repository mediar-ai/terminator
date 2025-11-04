/**
 * Simple Notepad Workflow
 * This workflow demonstrates basic UI automation with TypeScript
 * Steps:
 * 1. Open Notepad
 * 2. Type "Hello"
 * 3. Add a space
 * 4. Type "World"
 * 5. Add exclamation mark
 */

import { createWorkflow, createWorkflowRunner, z } from "@mediar-ai/workflow";
import { openNotepad } from "@/steps/01-open-notepad";
import { typeHello } from "@/steps/02-type-hello";
import { addSpace } from "@/steps/03-add-space";
import { typeWorld } from "@/steps/04-type-world";
import { addExclamation } from "@/steps/05-add-exclamation";

// Define input schema
const inputSchema = z.object({
  greeting: z.string().default("Hello World!"),
});

// Define the workflow
const workflowOrBuilder = createWorkflow({
  name: "Simple Notepad Workflow",
  description: "Opens Notepad and types 'Hello World!' in multiple steps",
  version: "1.0.0",
  input: inputSchema,

  // Define the steps in order
  steps: [
    openNotepad,
    typeHello,
    addSpace,
    typeWorld,
    addExclamation,
  ],
});

// Check if we need to build or not
const workflow = 'build' in workflowOrBuilder ? workflowOrBuilder.build() : workflowOrBuilder;

// Main execution
async function main() {
  try {
    console.log("Starting Simple Notepad Workflow...");

    const runner = createWorkflowRunner({
      workflow: workflow,
      inputs: {
        greeting: "Hello World!",
      },
    });

    const result = await runner.run();

    console.log("Workflow completed:", result);
  } catch (error) {
    console.error("Workflow failed:", error);
    process.exit(1);
  }
}

// Export the workflow for MCP to run
export default workflow;

// Run if this is the main module
if (require.main === module) {
  main();
}