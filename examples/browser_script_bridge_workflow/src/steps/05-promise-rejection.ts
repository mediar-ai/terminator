import { createStep } from "@mediar-ai/workflow";

export const promiseRejection = createStep({
  id: "promise_rejection",
  name: "Handle Promise rejection from browser script",
  execute: async ({ context, logger }) => {
    const browser = context.data.browser;
    if (!browser) throw new Error("Browser element missing from context");

    let threw = false;
    try {
      // Throwing from script should be reported as a Promise rejection by the bridge
      await browser.executeBrowserScript("throw new Error('simulated failure E_TEST')");
    } catch (e: any) {
      threw = true;
      logger.info(`Caught expected error: ${e?.message || e}`);
    }

    if (!threw) {
      throw new Error("Expected Promise rejection to throw, but it did not");
    }

    logger.success("Promise rejection correctly surfaced as error");
    return { state: { rejectionHandled: true } };
  },
});
