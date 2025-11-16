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
      await desktop.navigateBrowser(releaseManageUrl, "Chrome");
      await desktop.delay(3000);

      // Scroll to bottom to ensure delete controls are in view
      for (let i = 0; i < 4; i++) {
        await desktop.pressKey("End");
        await desktop.delay(400);
      }

      const deleteCheckboxes = await desktop.locator("role:CheckBox").all(5000);
      const deleteElementsFound = deleteCheckboxes.length;

      if (deleteElementsFound === 0) {
        throw new Error(
          "Delete checkboxes not found - may not be on correct release management page"
        );
      }

      return {
        data: {
          navigated: true,
          releaseUrl: releaseManageUrl,
          deleteElementsFound,
        },
        state: { deleteElementsFound },
      };
    } catch (error: any) {
      logger.error(`❌ Navigation failed: ${error.message}`);
      throw error;
    }
  },
});
