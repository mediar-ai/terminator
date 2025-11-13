// src/pypi-cleanup.ts
import { navigateToPyPI } from "@/steps/01-navigate-to-pypi";
import { loginToPyPI } from "@/steps/02-login-to-pypi";
import { getVersionsList } from "@/steps/03-get-versions-list";
import { findOldestVersion } from "@/steps/04-find-oldest-version";
import { navigateToDeletePage } from "@/steps/05-navigate-to-delete-page";
import { confirmDeletion } from "@/steps/06-confirm-deletion";
import { verifyDeletion } from "@/steps/07-verify-deletion";
import { createWorkflow, createWorkflowRunner, z } from "@mediar-ai/workflow";

const inputSchema = z.object({
  packageName: z.string(),
  pypiUsername: z.string(),
  pypiPassword: z.string(),
  keepVersions: z.number().default(10),
});

const workflowOrBuilder = createWorkflow({
  name: "PyPI Version Cleanup Workflow",
  description:
    "Automatically deletes oldest version from PyPI before publishing",
  version: "1.0.0",
  input: inputSchema,
  steps: [
    navigateToPyPI,
    loginToPyPI,
    getVersionsList,
    findOldestVersion,
    navigateToDeletePage,
    confirmDeletion,
    verifyDeletion,
  ],
});

const workflow =
  "build" in workflowOrBuilder ? workflowOrBuilder.build() : workflowOrBuilder;

async function main() {
  const runner = createWorkflowRunner({
    workflow,
    inputs: {
      packageName: process.env.PYPI_PACKAGE_NAME || "terminator",
      pypiUsername: process.env.PYPI_USERNAME || "",
      pypiPassword: process.env.PYPI_PASSWORD || "",
      keepVersions: parseInt(process.env.KEEP_VERSIONS || "10"),
    },
  });

  const result = await runner.run();
  console.log("Workflow completed:", result.status);

  if (result.status !== "success") {
    throw new Error(`Workflow failed with status: ${result.status}`);
  }
}

export default workflow;

if (require.main === module) {
  main().catch((e) => {
    console.error("Workflow failed:", e);
    process.exit(1);
  });
}
