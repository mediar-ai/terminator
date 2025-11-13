// src/steps/04-find-oldest-version.ts
import { createStep } from "@mediar-ai/workflow";

export const findOldestVersion = createStep({
  id: "find_oldest_version",
  name: "Identify Oldest Version to Delete",
  execute: async ({ context, input, logger }) => {
    logger.info("🔍 Finding oldest version...");

    try {
      const versions = context.data.versions;

      if (!versions || versions.length === 0) {
        throw new Error("No versions found in context");
      }

      // Check if we need to delete (based on keepVersions input)
      const keepVersions = input.keepVersions || 10;

      if (versions.length <= keepVersions) {
        logger.info(
          `   Current versions (${versions.length}) <= keep limit (${keepVersions})`
        );
        logger.success("✅ No deletion needed");

        return {
          state: {
            deletionNeeded: false,
            currentCount: versions.length,
            keepLimit: keepVersions,
          },
        };
      }

      // Oldest is typically the last in the array
      const oldestVersion = versions[versions.length - 1];

      logger.info(`   Oldest version: ${oldestVersion.version}`);
      logger.info(`   Total versions: ${versions.length}`);
      logger.info(`   Keep limit: ${keepVersions}`);
      logger.success(`✅ Will delete: ${oldestVersion.version}`);

      // Store for next steps
      context.data.oldestVersion = oldestVersion;

      return {
        state: {
          deletionNeeded: true,
          oldestVersion: oldestVersion.version,
          currentCount: versions.length,
          keepLimit: keepVersions,
        },
      };
    } catch (error: any) {
      logger.error(`❌ Failed to find oldest version: ${error.message}`);
      throw error;
    }
  },
});
