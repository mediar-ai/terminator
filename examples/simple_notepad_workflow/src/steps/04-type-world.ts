/**
 * Step 4: Type "World"
 */

import { createStep } from "@mediar-ai/workflow";

export const typeWorld = createStep({
  id: "type_world",
  name: "Type World",
  execute: async ({ desktop, logger }) => {
    logger.info("✍️ Typing 'World'...");

    try {
      // Directly find the Document element with name "Text editor"
      const textEditor = await desktop
        .locator("role:Document || role:Edit")
        .first(2000);

      // Type "World"
      textEditor.typeText("World");

      // Small delay for visual effect
      await desktop.delay(300);

      logger.info("✅ Typed 'World'");

      return {
        state: {
          textEditor,
          textTyped: "Hello World",
          worldTyped: true,
        },
      };
    } catch (error: any) {
      logger.error(`Failed to type 'World': ${error.message}`);
      throw error;
    }
  },
});