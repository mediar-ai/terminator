// src/steps/01-navigate-to-pypi.ts
import { createStep } from "@mediar-ai/workflow";

export const navigateToPyPI = createStep({
  id: "navigate_to_pypi",
  name: "Navigate to PyPI Manage Page",
  execute: async ({ desktop, input, logger, context }) => {
    logger.info("🌐 Opening PyPI in Chrome...");

    try {
      // Open Chrome with PyPI login page
      const browser = (desktop as any).navigateBrowser(
        "https://pypi.org/account/login/",
        "Chrome"
      );

      // Wait for page to load
      await new Promise((r) => setTimeout(r, 2000));

      // Focus browser window
      await browser.focus();
      await new Promise((r) => setTimeout(r, 500));

      // Store browser for subsequent steps
      context.data.browser = browser;

      // Verify page loaded
      const title = await browser.executeBrowserScript(
        "document.title || 'Unknown'"
      );

      logger.success(`✅ PyPI opened: ${title}`);

      return {
        state: {
          navigated: true,
          pageTitle: title,
        },
      };
    } catch (error: any) {
      logger.error(`❌ Failed to open PyPI: ${error.message}`);
      throw error;
    }
  },
});
