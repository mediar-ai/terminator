import { createStep } from "@mediar-ai/workflow";

export const structuredFailure = createStep({
  id: "structured_failure",
  name: "Structured failure object should fail",
  execute: async ({ context, logger }) => {
    const browser = context.data.browser;
    if (!browser) throw new Error("Browser element missing from context");

    let failed = false;
    try {
      // Return a JSON indicating failure; Rust bridge should interpret and error out
      await browser.executeBrowserScript(`JSON.stringify({ success: false, message: 'explicit failure path' })`);
    } catch (e: any) {
      failed = true;
      logger.info(`Caught expected structured failure: ${e?.message || e}`);
    }

    if (!failed) {
      throw new Error("Expected structured failure to throw, but it did not");
    }

    logger.success("Structured failure correctly surfaced as error");
    return { state: { structuredFailureHandled: true } };
  },
});
