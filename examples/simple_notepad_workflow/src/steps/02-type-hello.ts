/**
 * Step 2: Type "Hello"
 */

import { createStep } from "@mediar-ai/workflow";

export const typeHello = createStep({
  id: "type_hello",
  name: "Type Hello",
  execute: async ({ desktop, logger }) => {
    logger.info("✍️ Typing 'Hello'...");

    try {
      // Directly find the Document element with name "Text editor" (no chaining)
      const textEditor = await desktop
        .locator("role:Document || role:Edit")
        .first(3000);

      // Type "Hello"
      textEditor.typeText("Hello");

      // Small delay for visual effect
      await desktop.delay(300);

      logger.info("✅ Typed 'Hello'");

      return {
        state: {
          textTyped: "Hello",
          helloTyped: true,
        },
      };
    } catch (error: any) {
      logger.error(`Failed to type 'Hello': ${error.message}`);
      throw error;
    }
  },
});