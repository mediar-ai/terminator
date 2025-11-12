import { createWorkflow, createWorkflowRunner, z } from "@mediar-ai/workflow";
import { openBrowser } from "@/steps/01-open-browser";
import { basicExec } from "@/steps/02-basic-exec";
import { functionExec } from "@/steps/03-function-exec";
import { fileExec } from "@/steps/04-file-exec";
import { promiseRejection } from "@/steps/05-promise-rejection";
import { structuredFailure } from "@/steps/06-structured-failure";
import { retryAndReset } from "@/steps/07-retry-and-reset";

const inputSchema = z.object({
  url: z.string().default("about:blank"),
});

const workflowOrBuilder = createWorkflow({
  name: "Browser Script Bridge Workflow",
  description: "Tests executeBrowserScript via Chrome extension across edge cases",
  version: "1.0.0",
  input: inputSchema,
  steps: [
    openBrowser,
    basicExec,
    functionExec,
    fileExec,
    promiseRejection,
    structuredFailure,
    retryAndReset,
  ],
});

const workflow = 'build' in workflowOrBuilder ? workflowOrBuilder.build() : workflowOrBuilder;

async function main() {
  const runner = createWorkflowRunner({
    workflow,
    inputs: { url: "about:blank" },
  });
  const result = await runner.run();
  console.log("Workflow completed:", result.status);
}

export default workflow;

if (require.main === module) {
  main().catch((e) => {
    console.error("Workflow failed:", e);
    process.exit(1);
  });
}
