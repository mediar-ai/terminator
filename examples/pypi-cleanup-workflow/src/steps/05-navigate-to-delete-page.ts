// src/steps/05-navigate-to-delete-page.ts
import { createStep } from "@mediar-ai/workflow";

export const navigateToDeletePage = createStep({
  id: "navigate_to_delete_page",
  name: "Navigate to Version Delete Page",
  execute: async ({ context, input, logger }) => {
    // Skip if deletion not needed
    if (context.state.deletionNeeded === false) {
      logger.info("⏭️  Skipping - deletion not needed");
      return { state: { skipped: true } };
    }

    logger.info("🗑️  Navigating to delete page...");

    const browser = context.data.browser;
    if (!browser) throw new Error("Browser element missing from context");

    const oldestVersion = context.data.oldestVersion;
    if (!oldestVersion) throw new Error("Oldest version not found in context");

    try {
      const packageName = input.packageName;
      const deleteUrl = `https://pypi.org/manage/project/${packageName}/release/${oldestVersion.version}/`;

      const navigateResult = await browser.executeBrowserScript(`
        window.location.href = '${deleteUrl}';
        'Navigating to delete page for ${oldestVersion.version}'
      `);

      logger.info(`   ${navigateResult}`);
      await new Promise((r) => setTimeout(r, 2500));

      // Verify we're on the right page
      const pageCheck = await browser.executeBrowserScript(`
        const url = window.location.href;
        const title = document.title;
        if (!url.includes('${oldestVersion.version}')) {
          throw new Error('Not on correct version page');
        }
        title
      `);

      logger.success(`✅ On delete page: ${pageCheck}`);

      return {
        state: {
          onDeletePage: true,
          deleteUrl: deleteUrl,
        },
      };
    } catch (error: any) {
      logger.error(`❌ Failed to navigate to delete page: ${error.message}`);
      throw error;
    }
  },
});
