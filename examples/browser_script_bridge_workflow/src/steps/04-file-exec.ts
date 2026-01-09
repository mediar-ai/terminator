import { createStep } from "@mediar-ai/workflow";
import path from 'path';

export const fileExec = createStep({
  id: "file_exec",
  name: "Execute browser script from file with env",
  execute: async ({ context, logger }) => {
    const browser = context.data.browser;
    if (!browser) throw new Error("Browser element missing from context");

    const scriptPath = path.join(__dirname, "..", "scripts", "get-title.js");

    const result = await browser.executeBrowserScript({
      file: scriptPath,
      env: { note: "file-env" },
    });

    if (typeof result !== 'string' || result.length === 0) {
      throw new Error("File-based executeBrowserScript returned empty result");
    }

    logger.success(`File-based exec returned: ${result}`);
    return { state: { fileExec: result } };
  },
});
