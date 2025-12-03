/**
 * PyPI Version Cleanup Workflow
 *
 * Automates deletion of the oldest PyPI package version to stay under release limits.
 * PyPI doesn't have an API for deletion, so we use UI automation.
 *
 * Usage:
 * 1. Set PYPI_USERNAME and PYPI_PASSWORD environment variables
 * 2. Run the workflow with the package name as input
 *
 * GitHub Actions integration:
 * Add this as a step before publishing to PyPI in ci-wheels.yml
 */

import { z } from 'zod';
import { createWorkflow, createStep } from '@mediar-ai/workflow';

// Input schema for the workflow
const InputSchema = z.object({
  /** PyPI package name (e.g., 'terminator' or 'terminator-py') */
  packageName: z.string().describe('Name of the PyPI package'),
  /** PyPI username (usually __token__ for API tokens) */
  pypiUsername: z.string().default('__token__').describe('PyPI username'),
  /** PyPI password or API token */
  pypiPassword: z.string().describe('PyPI password or API token'),
  /** Whether to keep at least N versions (default: 10) */
  keepMinVersions: z.number().default(10).describe('Minimum versions to keep'),
});

type WorkflowInput = z.infer<typeof InputSchema>;

// =============================================================================
// Step 1: Navigate to PyPI and login
// =============================================================================
const navigateToPyPI = createStep<
  WorkflowInput,
  void,
  {},
  { loggedIn: boolean }
>({
  id: 'navigate-to-pypi',
  name: 'Navigate to PyPI and Login',
  execute: async ({ desktop, input, logger }) => {
    logger.info('Opening PyPI in browser...');

    // Open browser and navigate to PyPI login page
    await desktop.openUrl('https://pypi.org/account/login/');
    await desktop.wait(3000);

    // Check if already logged in (redirected to account page)
    const currentUrl = await desktop.getUrl();
    if (currentUrl?.includes('/account/')) {
      logger.success('Already logged in to PyPI');
      return { state: { loggedIn: true } };
    }

    // Fill in login form
    logger.info('Filling login form...');
    const usernameField = desktop.locator('input[name="username"]');
    await usernameField.fill(input.pypiUsername);

    const passwordField = desktop.locator('input[name="password"]');
    await passwordField.fill(input.pypiPassword);

    // Click login button
    const loginButton = desktop.locator('button[type="submit"], input[type="submit"]');
    await loginButton.click();
    await desktop.wait(3000);

    // Verify login succeeded
    const postLoginUrl = await desktop.getUrl();
    if (postLoginUrl?.includes('/account/login/')) {
      throw new Error('Login failed - still on login page');
    }

    logger.success('Successfully logged in to PyPI');
    return { state: { loggedIn: true } };
  },
  retries: 2,
  retryDelayMs: 2000,
});

// =============================================================================
// Step 2: Navigate to package management page
// =============================================================================
const navigateToPackage = createStep<
  WorkflowInput,
  void,
  { loggedIn: boolean },
  { packageUrl: string; managementUrl: string }
>({
  id: 'navigate-to-package',
  name: 'Navigate to Package Management',
  execute: async ({ desktop, input, logger, context }) => {
    if (!context.state.loggedIn) {
      throw new Error('Not logged in to PyPI');
    }

    // Navigate to the package's manage page
    const manageUrl = `https://pypi.org/manage/project/${input.packageName}/releases/`;
    logger.info(`Navigating to ${manageUrl}...`);

    await desktop.openUrl(manageUrl);
    await desktop.wait(2000);

    // Verify we're on the right page
    const pageTitle = await desktop.locator('h1, .page-title').getText();
    if (!pageTitle?.toLowerCase().includes(input.packageName.toLowerCase())) {
      throw new Error(`Package ${input.packageName} not found or no access`);
    }

    logger.success(`Found package: ${input.packageName}`);
    return {
      state: {
        packageUrl: `https://pypi.org/project/${input.packageName}/`,
        managementUrl: manageUrl,
      },
    };
  },
  retries: 2,
  retryDelayMs: 2000,
});

// =============================================================================
// Step 3: Get list of versions and find oldest
// =============================================================================
const findOldestVersion = createStep<
  WorkflowInput,
  void,
  { loggedIn: boolean; packageUrl: string; managementUrl: string },
  { versions: string[]; oldestVersion: string | null; shouldDelete: boolean }
>({
  id: 'find-oldest-version',
  name: 'Find Oldest Version',
  execute: async ({ desktop, input, logger }) => {
    logger.info('Scanning available versions...');

    // Find all version rows on the releases page
    // PyPI typically lists versions with links containing the version number
    const versionElements = await desktop.locator('.release__version, td.release-version, a[href*="/releases/"]').all();

    const versions: string[] = [];
    for (const elem of versionElements) {
      const text = await elem.getText();
      if (text && /^\d+\.\d+/.test(text.trim())) {
        versions.push(text.trim());
      }
    }

    // If we couldn't find versions via specific selectors, try table rows
    if (versions.length === 0) {
      const tableRows = await desktop.locator('table tbody tr').all();
      for (const row of tableRows) {
        const versionCell = await row.locator('td:first-child, .version').getText();
        if (versionCell && /^\d+\.\d+/.test(versionCell.trim())) {
          versions.push(versionCell.trim());
        }
      }
    }

    logger.info(`Found ${versions.length} versions`);

    if (versions.length === 0) {
      logger.warn('No versions found on the page');
      return {
        state: {
          versions: [],
          oldestVersion: null,
          shouldDelete: false,
        },
      };
    }

    // Sort versions and find the oldest
    // PyPI usually lists newest first, so oldest is at the end
    const sortedVersions = [...versions].sort((a, b) => {
      // Compare version strings
      const partsA = a.split('.').map((p) => parseInt(p, 10) || 0);
      const partsB = b.split('.').map((p) => parseInt(p, 10) || 0);
      for (let i = 0; i < Math.max(partsA.length, partsB.length); i++) {
        const diff = (partsA[i] || 0) - (partsB[i] || 0);
        if (diff !== 0) return diff;
      }
      return 0;
    });

    const oldestVersion = sortedVersions[0];

    // Only delete if we have more versions than the minimum to keep
    const shouldDelete = versions.length > input.keepMinVersions;

    if (shouldDelete) {
      logger.info(`Will delete oldest version: ${oldestVersion} (${versions.length} versions exist, min: ${input.keepMinVersions})`);
    } else {
      logger.info(`Keeping all versions (${versions.length} <= ${input.keepMinVersions})`);
    }

    return {
      state: {
        versions: sortedVersions,
        oldestVersion,
        shouldDelete,
      },
    };
  },
});

// =============================================================================
// Step 4: Delete the oldest version
// =============================================================================
const deleteOldestVersion = createStep<
  WorkflowInput,
  void,
  {
    loggedIn: boolean;
    packageUrl: string;
    managementUrl: string;
    versions: string[];
    oldestVersion: string | null;
    shouldDelete: boolean;
  },
  { deleted: boolean; deletedVersion: string | null }
>({
  id: 'delete-oldest-version',
  name: 'Delete Oldest Version',
  condition: ({ context }) => context.state.shouldDelete && context.state.oldestVersion !== null,
  execute: async ({ desktop, input, logger, context }) => {
    const { oldestVersion, managementUrl } = context.state;

    if (!oldestVersion) {
      logger.warn('No version to delete');
      return { state: { deleted: false, deletedVersion: null } };
    }

    logger.info(`Deleting version ${oldestVersion}...`);

    // Navigate to the specific version's options/delete page
    const versionManageUrl = `https://pypi.org/manage/project/${input.packageName}/release/${oldestVersion}/`;
    await desktop.openUrl(versionManageUrl);
    await desktop.wait(2000);

    // Look for delete button/link
    const deleteButton = desktop.locator(
      'a[href*="delete"], button:has-text("Delete"), .danger-button, .btn-danger'
    );

    // Click delete
    await deleteButton.click();
    await desktop.wait(1000);

    // Handle confirmation dialog
    // PyPI typically asks you to type the project name to confirm deletion
    const confirmInput = desktop.locator('input[name="confirm"], #confirm, input[placeholder*="project name"]');
    const isConfirmVisible = await confirmInput.isVisible();

    if (isConfirmVisible) {
      logger.info('Filling confirmation dialog...');
      await confirmInput.fill(input.packageName);

      // Click the final delete/confirm button
      const confirmButton = desktop.locator(
        'button[type="submit"]:has-text("Delete"), button.danger-button, input[type="submit"][value*="Delete"]'
      );
      await confirmButton.click();
      await desktop.wait(3000);
    }

    // Verify deletion by checking if version is gone
    await desktop.openUrl(managementUrl);
    await desktop.wait(2000);

    const pageContent = await desktop.locator('body').getText();
    if (pageContent?.includes(oldestVersion)) {
      throw new Error(`Version ${oldestVersion} still appears on the page - deletion may have failed`);
    }

    logger.success(`Successfully deleted version ${oldestVersion}`);
    return {
      state: {
        deleted: true,
        deletedVersion: oldestVersion,
      },
    };
  },
  onError: async ({ error, retry, attempt, logger }) => {
    if (attempt < 2 && error.message.includes('not found')) {
      logger.warn(`Delete button not found, retrying (attempt ${attempt + 1})...`);
      return retry();
    }
    return { recoverable: false, reason: error.message };
  },
});

// =============================================================================
// Build the workflow
// =============================================================================
const deletePyPIOldestVersionWorkflow = createWorkflow({
  name: 'Delete PyPI Oldest Version',
  description: 'Deletes the oldest version of a PyPI package to stay under release limits',
  input: InputSchema,
})
  .step(navigateToPyPI)
  .step(navigateToPackage)
  .step(findOldestVersion)
  .step(deleteOldestVersion)
  .onSuccess(async ({ context, logger, duration }) => {
    const { deleted, deletedVersion, versions, oldestVersion } = context.state;

    if (deleted && deletedVersion) {
      logger.success(`\n=== PyPI Cleanup Complete ===`);
      logger.success(`Deleted version: ${deletedVersion}`);
      logger.success(`Remaining versions: ${versions.length - 1}`);
      logger.info(`Duration: ${duration}ms`);

      return {
        success: true,
        message: `Deleted PyPI version ${deletedVersion}`,
        summary: `# PyPI Version Cleanup\n\n- **Deleted**: ${deletedVersion}\n- **Remaining**: ${versions.length - 1} versions\n- **Duration**: ${duration}ms`,
        data: { deletedVersion, remainingVersions: versions.length - 1 },
      };
    } else {
      logger.info(`\n=== PyPI Cleanup Complete ===`);
      logger.info(`No deletion needed - ${versions?.length || 0} versions exist`);

      return {
        success: true,
        message: 'No version deletion needed',
        summary: `# PyPI Version Cleanup\n\nNo deletion needed. Current version count (${versions?.length || 0}) is within limits.`,
        data: { deletedVersion: null, currentVersionCount: versions?.length || 0 },
      };
    }
  })
  .onError(async ({ error, step, logger }) => {
    logger.error(`\n=== PyPI Cleanup Failed ===`);
    logger.error(`Failed at step: ${step.config.name}`);
    logger.error(`Error: ${error.message}`);

    return {
      status: 'error',
      error: {
        category: 'business',
        code: 'PYPI_CLEANUP_FAILED',
        message: error.message,
        recoverable: false,
        metadata: { failedStep: step.config.id },
      },
    };
  })
  .build();

export default deletePyPIOldestVersionWorkflow;

// =============================================================================
// CLI runner for testing
// =============================================================================
if (require.main === module) {
  const packageName = process.argv[2] || 'terminator';
  const pypiUsername = process.env.PYPI_USERNAME || '__token__';
  const pypiPassword = process.env.PYPI_PASSWORD || process.env.PYPI_API_TOKEN || '';

  if (!pypiPassword) {
    console.error('Error: PYPI_PASSWORD or PYPI_API_TOKEN environment variable required');
    process.exit(1);
  }

  console.log(`\nRunning PyPI cleanup workflow for package: ${packageName}\n`);

  deletePyPIOldestVersionWorkflow
    .run({
      packageName,
      pypiUsername,
      pypiPassword,
      keepMinVersions: 10,
    })
    .then((result) => {
      console.log('\nWorkflow result:', JSON.stringify(result, null, 2));
      process.exit(result.status === 'success' ? 0 : 1);
    })
    .catch((err) => {
      console.error('Workflow failed:', err);
      process.exit(1);
    });
}
