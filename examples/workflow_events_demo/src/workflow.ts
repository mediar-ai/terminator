/**
 * Workflow Events Demo
 *
 * This workflow demonstrates the event streaming API - emitting progress updates,
 * step events, and custom data that get forwarded to MCP clients in real-time.
 *
 * Run this workflow via the MCP agent's execute_sequence tool:
 *
 *   {
 *     "url": "file://./examples/workflow_events_demo",
 *     "inputs": { "message": "Hello from workflow events!" }
 *   }
 *
 * The MCP client will receive:
 * - Progress notifications (notifications/progress)
 * - Logging messages (notifications/message) with structured event data
 */

import {
  z,
  createWorkflow,
  createStep,
  createWorkflowRunner,
  emit,
  createStepEmitter,
} from "@mediar-ai/workflow";

// Input schema
const inputSchema = z.object({
  message: z.string().default("Hello World!"),
  simulateError: z.boolean().default(false),
});

type Input = z.infer<typeof inputSchema>;

// Step 1: Initialize
const initializeStep = createStep<Input>({
  id: "initialize",
  name: "Initialize Workflow",
  execute: async ({ input, logger }) => {
    // Emit initial progress
    emit.progress(0, 4, "Workflow starting...");

    // Log something
    emit.log("info", `Received message: ${input.message}`);

    // Emit some custom data
    emit.data("startTime", new Date().toISOString());
    emit.data("config", { message: input.message, simulateError: input.simulateError });

    logger.info("Initialization complete");
    return { initialized: true };
  },
});

// Step 2: Open Notepad (simulated for demo)
const openNotepadStep = createStep<Input>({
  id: "open_notepad",
  name: "Open Notepad",
  execute: async ({ desktop, logger }) => {
    // Create a scoped emitter for this step
    const stepEmit = createStepEmitter("open_notepad", "Open Notepad", 1, 4);

    stepEmit.started();

    emit.progress(1, 4, "Opening Notepad...");

    // Simulate some work
    await new Promise((resolve) => setTimeout(resolve, 500));

    // Try to open notepad (will actually work on Windows)
    try {
      await desktop.openApplication("notepad.exe");
      emit.log("info", "Notepad opened successfully");
    } catch (e) {
      emit.log("warn", "Could not open Notepad (might not be on Windows)");
    }

    stepEmit.completed(500);
    return { notepadOpened: true };
  },
});

// Step 3: Type Message
const typeMessageStep = createStep<Input>({
  id: "type_message",
  name: "Type Message",
  execute: async ({ input, desktop }) => {
    emit.stepStarted("type_message", "Type Message", 2, 4);

    emit.progress(2, 4, `Typing message: ${input.message}`);

    // Simulate typing with progress updates
    const words = input.message.split(" ");
    for (let i = 0; i < words.length; i++) {
      emit.progress(2 + (i / words.length) * 0.9, 4, `Typing word ${i + 1}/${words.length}...`);
      await new Promise((resolve) => setTimeout(resolve, 200));
    }

    // Try to type in notepad
    try {
      const notepad = await desktop.locator("role:Edit").first(2000);
      if (notepad) {
        await notepad.typeText(input.message);
      }
    } catch (e) {
      emit.log("debug", "Could not find text editor element");
    }

    emit.stepCompleted("type_message", "Type Message", 1000, 2, 4);
    emit.data("typedMessage", input.message);

    return { messageTyped: true };
  },
});

// Step 4: Finalize
const finalizeStep = createStep<Input>({
  id: "finalize",
  name: "Finalize",
  execute: async ({ input, context, logger }) => {
    emit.stepStarted("finalize", "Finalize", 3, 4);

    // Check if we should simulate an error
    if (input.simulateError) {
      emit.log("error", "Simulating an error as requested");
      emit.stepFailed("finalize", "Finalize", "Simulated error for testing", 100);
      throw new Error("Simulated error for testing");
    }

    emit.progress(3, 4, "Finalizing workflow...");

    // Emit final data
    emit.data("endTime", new Date().toISOString());
    emit.data("success", true);

    // Take a screenshot (simulated path)
    emit.screenshot("/tmp/workflow-complete.png", "Final state after workflow completion");

    emit.progress(4, 4, "Workflow complete!");

    emit.stepCompleted("finalize", "Finalize", 200, 3, 4);

    // Set final output data for MCP response
    context.data = {
      message: input.message,
      success: true,
      steps_completed: 4,
    };

    logger.success("Workflow completed successfully!");
    return { finalized: true };
  },
});

// Create workflow - name/version/description are read from package.json
const workflow = createWorkflow({
  input: inputSchema,
  steps: [initializeStep, openNotepadStep, typeMessageStep, finalizeStep],
});

// Export the workflow for MCP to run
export default workflow;

// Run if executed directly
if (require.main === module) {
  const runner = createWorkflowRunner({
    workflow: workflow,
    inputs: {
      message: "Hello from events demo!",
      simulateError: false,
    },
  });
  runner.run().catch(console.error);
}
