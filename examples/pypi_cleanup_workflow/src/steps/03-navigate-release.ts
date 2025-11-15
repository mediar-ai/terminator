import { createStep } from "@mediar-ai/workflow";

export const navigateToRelease = createStep({
  id: "navigate-release",
  name: "Navigate to Release Management",
  description: "Navigate to the oldest release management page",

  execute: async ({ desktop, context, input, logger }: any) => {
    const version = context.data.oldestVersion;
    const packageName = input.packageName;

    logger.info(
      `🧭 Navigating to release management for ${packageName} v${version}`
    );

    try {
      const releaseManageUrl = `https://pypi.org/manage/project/${packageName}/release/${version}/`;
      await desktop.navigateBrowser(releaseManageUrl);
      await desktop.delay(2000);

      const deleteElements = await desktop.findElements({
        selector:
          'input[type="checkbox"][data-action="input->delete-confirm#check"][data-delete-confirm-target="input"]',
      });

      if (deleteElements.length === 0) {
        throw new Error(
          "Delete checkboxes not found - may not be on correct release management page"
        );
      }

      context.data.deleteElementsFound = deleteElements.length;

      return {
        success: true,
        data: {
          navigated: true,
          releaseUrl: releaseManageUrl,
          deleteElementsFound: deleteElements.length,
        },
      };
    } catch (error: any) {
      logger.error(`❌ Navigation failed: ${error.message}`);
      throw error;
    }
  },
});
