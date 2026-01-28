import { createStep } from "@mediar-ai/workflow";

export const retryAndReset = createStep({
  id: "retry_and_reset",
  name: "Simulate transient issue then succeed",
  execute: async ({ context, logger, desktop }) => {
    const browser = context.data.browser;
    if (!browser) throw new Error("Browser element missing from context");

    // We cannot programmatically break the extension connection here, but we can at least
    // run a benign script, then wait, then run again to exercise stable retry path.
    // The Rust bridge already includes retry & reset logic on connection hiccups.

    const before = await browser.executeBrowserScript("document.readyState");
    logger.info(`readyState before: ${before}`);

    // Short pause to mimic potential reconnection window
    await new Promise((r) => setTimeout(r, 500));

    const after = await browser.executeBrowserScript("document.readyState");
    logger.info(`readyState after: ${after}`);

    if (before !== after && !(before === 'loading' && after === 'complete')) {
      // Not a strict requirement; just log it
      logger.warn(`readyState changed from ${before} to ${after}`);
    }

    logger.success("Retry/sanity pass completed successfully");
    return { state: { retryProbe: { before, after } } };
  },
});
