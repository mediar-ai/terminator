/**
 * Step 3: Add a space
 */

import { createStep } from "@mediar-ai/workflow";

export const addSpace = createStep({
  id: "add_space",
  name: "Add Space",
  execute: async ({ desktop, logger }) => {
    logger.info("➕ Adding space...");

    try {
      // Directly find the Document element with name "Text editor"
      const textEditor = await desktop
        .locator("role:Document || role:Edit")
        .first(2000);

      // Type a space
      textEditor.typeText(" ");

      // Small delay for visual effect
      await desktop.delay(200);

      logger.info("✅ Added space");

      return {
        state: {
          textTyped: "Hello ",
          spaceAdded: true,
        },
      };
    } catch (error: any) {
      logger.error(`Failed to add space: ${error.message}`);
      throw error;
    }
  },
});