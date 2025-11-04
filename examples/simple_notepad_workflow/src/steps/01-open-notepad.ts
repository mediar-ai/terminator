/**
 * Step 1: Open Notepad
 */

import { createStep } from "@mediar-ai/workflow";

export const openNotepad = createStep({
  id: "open_notepad",
  name: "Open Notepad Application",
  execute: async ({ desktop, logger }) => {
    logger.info("📝 Opening Notepad...");

    try {
      // Open Notepad application
      const notepadApp = desktop.openApplication("notepad");

      // Wait for Notepad to fully load
      await desktop.delay(1500);

      // Verify Notepad opened by finding the text editor
      const textEditor = await desktop
        .locator("role:Document || role:Edit")
        .first(2000);

      logger.info("✅ Notepad opened successfully");

      return {
        state: {
          notepadOpened: true,
          timestamp: new Date().toISOString(),
        },
      };
    } catch (error: any) {
      logger.error(`Failed to open Notepad: ${error.message}`);
      throw error;
    }
  },
});