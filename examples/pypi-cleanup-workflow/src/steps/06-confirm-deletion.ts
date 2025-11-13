// src/steps/06-confirm-deletion.ts
import { createStep } from "@mediar-ai/workflow";

export const confirmDeletion = createStep({
  id: "confirm_deletion",
  name: "Confirm and Execute Deletion",
  execute: async ({ context, logger }) => {
    // Skip if deletion not needed
    if (context.state.deletionNeeded === false) {
      logger.info("⏭️  Skipping - deletion not needed");
      return { state: { skipped: true } };
    }

    logger.info("⚠️  Confirming deletion...");

    const browser = context.data.browser;
    if (!browser) throw new Error("Browser element missing from context");

    const oldestVersion = context.data.oldestVersion;
    if (!oldestVersion) throw new Error("Oldest version not found in context");

    try {
      // Find and fill confirmation input
      const fillConfirmResult = await browser.executeBrowserScript(`
        const confirmInput = document.querySelector('input[name="confirm_delete_version"]');
        if (!confirmInput) {
          throw new Error('Confirmation input not found');
        }
        confirmInput.value = '${oldestVersion.version}';
        confirmInput.dispatchEvent(new Event('input', { bubbles: true }));
        'Confirmation text entered'
      `);

      logger.info(`   ${fillConfirmResult}`);
      await new Promise((r) => setTimeout(r, 500));

      // Click delete button
      const clickDeleteResult = await browser.executeBrowserScript(`
        const deleteBtn = document.querySelector('button[value="Delete"], button:has-text("Delete")');
        if (!deleteBtn) {
          throw new Error('Delete button not found');
        }
        deleteBtn.click();
        'Delete button clicked'
      `);

      logger.info(`   ${clickDeleteResult}`);

      // Wait for deletion to process
      await new Promise((r) => setTimeout(r, 3000));

      logger.success(`✅ Version ${oldestVersion.version} deleted`);

      return {
        state: {
          deleted: true,
          deletedVersion: oldestVersion.version,
        },
      };
    } catch (error: any) {
      logger.error(`❌ Failed to confirm deletion: ${error.message}`);
      throw error;
    }
  },
});
