import { createStep } from "@mediar-ai/workflow";

interface Env {
  marker: string;
}

export const functionExec = createStep({
  id: "function_exec",
  name: "Execute function-based browser script",
  execute: async ({ context, logger }) => {
    const browser = context.data.browser;
    if (!browser) throw new Error("Browser element missing from context");

    // Function input should be serialized and the wrapper will JSON.parse on return
    const result = await browser.executeBrowserScript((env: Env) => {
      return {
        ok: true,
        href: window.location.href,
        title: document.title,
        marker: env?.marker ?? "none",
      };
    }, { marker: "func-env" });

    if (!result || typeof result !== 'object' || (result as any).ok !== true) {
      throw new Error("Function-based executeBrowserScript did not return expected object");
    }

    logger.success(`Function-based exec returned ok with marker ${(result as any).marker}`);
    return { state: { functionExec: result } };
  },
});
