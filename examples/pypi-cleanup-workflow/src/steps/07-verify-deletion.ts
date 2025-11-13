// src/steps/07-verify-deletion.ts
import { createStep } from "@mediar-ai/workflow";

export const verifyDeletion = createStep({
  id: "verify_deletion",
  name: "Verify Version Was Deleted",
  execute: async ({ context, input, logger }) => {
    // Skip if deletion not needed
    if (context.state.deletionNeeded === false) {
      logger.info("⏭️  Skipping - deletion not needed");
      return { state: { skipped: true, verified: true } };
    }

    logger.info("✔️  Verifying deletion...");

    const browser = context.data.browser;
    if (!browser) throw new Error("Browser element missing from context");

    const deletedVersion = context.state.deletedVersion;

    try {
      // Navigate back to releases page
      const packageName = input.packageName;
      const navigateResult = await browser.executeBrowserScript(`
        window.location.href = 'https://pypi.org/manage/project/${packageName}/releases/';
        'Navigating to releases page'
      `);

      logger.info(`   ${navigateResult}`);
      await new Promise((r) => setTimeout(r, 2000));

      // Check if deleted version still exists
      const verifyResult = await browser.executeBrowserScript(`
        const pageText = document.body.textContent;
        const stillExists = pageText.includes('${deletedVersion}');
        if (stillExists) {
          throw new Error('Version ${deletedVersion} still appears on page');
        }
        'Version ${deletedVersion} no longer found'
      `);

      logger.success(`✅ ${verifyResult}`);

      return {
        state: {
          verified: true,
          deletedVersion: deletedVersion,
        },
      };
    } catch (error: any) {
      logger.error(`❌ Verification failed: ${error.message}`);
      throw error;
    }
  },
});
