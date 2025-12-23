/**
 * PyPI Cleanup Workflow
 * This workflow demonstrates PyPI release management automation using Terminator
 * Steps:
 * 1. Fetch oldest release info from PyPI JSON API
 * 2. Login to PyPI
 * 3. Navigate to release management page
 * 4. Delete the oldest release
 */

import { fetchOldestRelease } from "./steps/01-fetch-oldest-release";
import { loginToPyPI } from "./steps/02-login-pypi";
import { navigateToRelease } from "./steps/03-navigate-release";
import { deleteRelease } from "./steps/04-delete-release";
import { createWorkflow, createWorkflowRunner, z } from "@mediar-ai/workflow";

const inputSchema = z.object({
  packageName: z.string(),
  username: z.string(),
  password: z.string(),
  totpSecret: z.string(),
});

const workflowOrBuilder = createWorkflow({
  name: "PyPI Oldest Release Cleanup",
  description: "Automatically finds and deletes the oldest PyPI release",
  version: "1.0.0",
  input: inputSchema,

  steps: [fetchOldestRelease, loginToPyPI, navigateToRelease, deleteRelease],
});

const workflow =
  "build" in workflowOrBuilder ? workflowOrBuilder.build() : workflowOrBuilder;

async function main() {
  const input = {
    packageName: process.env.PACKAGE_NAME || "",
    username: process.env.PYPI_UI_USERNAME || "",
    password: process.env.PYPI_UI_PASSWORD || "",
    totpSecret: process.env.PYPI_UI_TOTP_SECRET || "",
  };

  if (
    !input.username ||
    !input.password ||
    !input.packageName ||
    !input.totpSecret
  ) {
    console.error(
      "❌ Missing PYPI_UI_USERNAME, PYPI_UI_PASSWORD, PACKAGE_NAME, or PYPI_UI_TOTP_SECRET environment variables"
    );
    process.exit(1);
  }

  try {
    console.log("🚀 Starting PyPI Cleanup Workflow...");
    const runner = createWorkflowRunner({
      workflow,
      inputs: input,
    });
    const result = await runner.run();
    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    console.error("❌ Workflow execution failed:", error);
    process.exit(1);
  }
}

// Export the workflow for MCP to run
export default workflow;

// Run if this is the main module
if (require.main === module) {
  main();
}
