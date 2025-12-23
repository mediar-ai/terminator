import { createStep } from "@mediar-ai/workflow";
import fetch from "node-fetch";

interface ReleaseInfo {
  version: string;
  uploadTime: Date;
}

export const fetchOldestRelease = createStep({
  id: "fetch-oldest-release",
  name: "Fetch Oldest Release",
  description: "Fetch the oldest PyPI release information via JSON API",

  execute: async ({ context, input, logger }: any) => {
    const packageName = input.packageName;
    logger.info(`📡 Fetching oldest release for package: ${packageName}`);

    try {
      const url = `https://pypi.org/pypi/${packageName}/json`;
      const response = await fetch(url);

      if (!response.ok) {
        throw new Error(
          `Failed to fetch PyPI JSON: ${response.status} ${response.statusText}`
        );
      }

      const data: any = await response.json();
      const releases: ReleaseInfo[] = Object.entries<any>(data.releases)
        .filter(([_, files]) => Array.isArray(files) && files.length > 0)
        .map(([version, files]) => ({
          version,
          uploadTime: new Date(files[0].upload_time),
        }))
        .sort((a, b) => a.uploadTime.getTime() - b.uploadTime.getTime());

      if (releases.length === 0) {
        throw new Error("No releases found with files");
      }

      const oldest = releases[0];
      logger.success(
        `✅ Found oldest release: ${
          oldest.version
        } (uploaded ${oldest.uploadTime.toDateString()})`
      );

      context.data.oldestVersion = oldest.version;
      context.data.oldestUploadTime = oldest.uploadTime.toISOString();
      context.data.totalReleases = releases.length;

      return {
        success: true,
        data: {
          version: oldest.version,
          uploadTime: oldest.uploadTime.toISOString(),
          totalReleases: releases.length,
        },
      };
    } catch (error: any) {
      logger.error(`❌ Failed to fetch release info: ${error.message}`);
      throw error;
    }
  },
});
