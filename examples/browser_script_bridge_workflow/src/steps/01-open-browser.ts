import { createStep } from "@mediar-ai/workflow";

export const openBrowser = createStep({
  id: "open_browser",
  name: "Open Chrome to URL",
  execute: async ({ desktop, input, logger, context }) => {
    logger.info(`Opening Chrome to ${input.url}...`);

    // Prefer Chrome explicitly to align with extension bridge (cast to any for editor compatibility)
    const browser = (desktop as any).navigateBrowser(input.url, "Chrome");

    // Give the page a moment to be ready
    await new Promise((r) => setTimeout(r, 1500));

    // Focus the browser window
    await browser.focus();
    await new Promise((r) => setTimeout(r, 250));

    // Store for subsequent steps
    context.data.browser = browser;

    logger.success("Chrome opened and focused");

    return {
      state: {
        opened: true,
        title: await browser.executeBrowserScript("document.title || ''"),
      },
    };
  },
});
