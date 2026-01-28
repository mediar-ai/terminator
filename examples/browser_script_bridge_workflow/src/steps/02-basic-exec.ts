import { createStep } from "@mediar-ai/workflow";

export const basicExec = createStep({
  id: "basic_exec",
  name: "Execute basic inline browser scripts",
  execute: async ({ context, logger }) => {
    const browser = context.data.browser;
    if (!browser) throw new Error("Browser element missing from context");

    const title = await browser.executeBrowserScript(
      "document.title || 'about:blank'"
    );
    logger.info(`document.title: ${title}`);

    const ua = await browser.executeBrowserScript(
      "navigator.userAgent"
    );
    if (!ua || ua.length < 10) {
      throw new Error("navigator.userAgent seems invalid");
    }
    logger.success("Basic inline scripts executed successfully");

    return { state: { basic: { title, ua } } };
  },
});
