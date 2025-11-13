// src/steps/03-get-versions-list.ts
import { createStep } from "@mediar-ai/workflow";
import path from "path";

export const getVersionsList = createStep({
  id: "get_versions_list",
  name: "Get List of Package Versions",
  execute: async ({ context, input, logger }) => {
    logger.info("📋 Fetching versions list...");

    const browser = context.data.browser;
    if (!browser) throw new Error("Browser element missing from context");

    try {
      // Navigate to package releases page
      const packageName = input.packageName;
      const navigateResult = await browser.executeBrowserScript(`
        window.location.href = 'https://pypi.org/manage/project/${packageName}/releases/';
        'Navigating to releases page'
      `);

      logger.info(`   ${navigateResult}`);
      await new Promise((r) => setTimeout(r, 3000));

      // Use external script file to get versions
      const scriptPath = path.join(
        __dirname,
        "..",
        "scripts",
        "get-versions.js"
      );

      const versionsJson = await browser.executeBrowserScript({
        file: scriptPath,
        env: { packageName },
      });

      const versions = JSON.parse(versionsJson);

      if (!versions || !versions.ok || !Array.isArray(versions.data)) {
        throw new Error(`Failed to get versions: ${versionsJson}`);
      }

      logger.success(`✅ Found ${versions.data.length} versions`);

      // Log first few and last few versions
      if (versions.data.length > 0) {
        logger.info(`   Latest: ${versions.data[0].version}`);
        logger.info(
          `   Oldest: ${versions.data[versions.data.length - 1].version}`
        );
      }

      // Store in context
      context.data.versions = versions.data;

      return {
        state: {
          totalVersions: versions.data.length,
          versions: versions.data,
        },
      };
    } catch (error: any) {
      logger.error(`❌ Failed to get versions: ${error.message}`);
      throw error;
    }
  },
});
