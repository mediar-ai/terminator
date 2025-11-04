/**
 * Step 5: Add exclamation mark
 */

import { createStep } from "@mediar-ai/workflow";

export const addExclamation = createStep({
  id: "add_exclamation",
  name: "Add Exclamation Mark",
  execute: async ({ desktop, logger, context }) => {
    logger.info("❗ Adding exclamation mark...");

    try {
      // Use the state property to access the text editor element
      const textEditor = context.state.textEditor;

      // Type exclamation mark
      textEditor.typeText("!");

      // Small delay for visual effect
      await desktop.delay(500);

      logger.info("✅ Added exclamation mark");

      const finalText = "Hello World!";
      logger.info(`📝 Final text typed: "${finalText}"`);

      return {
        state: {
          textTyped: finalText,
          exclamationAdded: true,
          workflowComplete: true,
        },
      };
    } catch (error: any) {
      logger.error(`Failed to add exclamation: ${error.message}`);
      throw error;
    }
  },
});
