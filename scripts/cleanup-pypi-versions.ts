#!/usr/bin/env npx tsx
/**
 * PyPI Version Cleanup Script
 *
 * This script is designed to be called from GitHub Actions before publishing
 * to PyPI. It uses the Terminator workflow system to automate the deletion
 * of the oldest PyPI version when approaching the release limit.
 *
 * Usage:
 *   npx tsx scripts/cleanup-pypi-versions.ts <package-name>
 *
 * Environment variables:
 *   PYPI_USERNAME - PyPI username (default: __token__)
 *   PYPI_PASSWORD - PyPI password or API token (required)
 *   PYPI_API_TOKEN - Alternative to PYPI_PASSWORD
 *   KEEP_MIN_VERSIONS - Minimum versions to keep (default: 10)
 *
 * Example GitHub Actions step:
 *   - name: Cleanup old PyPI versions
 *     env:
 *       PYPI_PASSWORD: ${{ secrets.PYPI_API_TOKEN }}
 *     run: npx tsx scripts/cleanup-pypi-versions.ts terminator
 */

import deletePyPIOldestVersionWorkflow from '../packages/workflow/examples/delete-pypi-oldest-version';

async function main() {
  // Parse arguments
  const packageName = process.argv[2];
  if (!packageName) {
    console.error('Usage: cleanup-pypi-versions.ts <package-name>');
    console.error('');
    console.error('Example:');
    console.error('  npx tsx scripts/cleanup-pypi-versions.ts terminator');
    console.error('');
    console.error('Environment variables:');
    console.error('  PYPI_PASSWORD or PYPI_API_TOKEN - Required');
    console.error('  PYPI_USERNAME - Optional (default: __token__)');
    console.error('  KEEP_MIN_VERSIONS - Optional (default: 10)');
    process.exit(1);
  }

  // Get credentials from environment
  const pypiUsername = process.env.PYPI_USERNAME || '__token__';
  const pypiPassword = process.env.PYPI_PASSWORD || process.env.PYPI_API_TOKEN || '';
  const keepMinVersions = parseInt(process.env.KEEP_MIN_VERSIONS || '10', 10);

  if (!pypiPassword) {
    console.error('Error: PYPI_PASSWORD or PYPI_API_TOKEN environment variable is required');
    process.exit(1);
  }

  console.log('========================================');
  console.log('  PyPI Version Cleanup');
  console.log('========================================');
  console.log(`Package: ${packageName}`);
  console.log(`Keep minimum: ${keepMinVersions} versions`);
  console.log('----------------------------------------');
  console.log('');

  try {
    const result = await deletePyPIOldestVersionWorkflow.run({
      packageName,
      pypiUsername,
      pypiPassword,
      keepMinVersions,
    });

    console.log('');
    console.log('========================================');
    console.log('  Result');
    console.log('========================================');
    console.log(JSON.stringify(result, null, 2));

    if (result.status === 'success') {
      console.log('');
      console.log('PyPI cleanup completed successfully!');
      process.exit(0);
    } else {
      console.error('');
      console.error('PyPI cleanup failed!');
      process.exit(1);
    }
  } catch (error) {
    console.error('');
    console.error('========================================');
    console.error('  Error');
    console.error('========================================');
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}

main();
