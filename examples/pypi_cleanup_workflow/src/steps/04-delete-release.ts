import { createStep } from "@mediar-ai/workflow";

export const deleteRelease = createStep({
  id: "delete-release",
  name: "Delete Release",
  description: "Delete the oldest PyPI release",

  execute: async ({ desktop, context, input, logger }: any) => {
    const version = context.data.oldestVersion;
    const packageName = input.packageName;

    logger.info(`🗑️ Deleting release ${packageName} v${version}`);

    try {
      if (!version) {
        throw new Error("Missing release version from previous steps");
      }

      const checkboxes = await desktop.locator("role:CheckBox").all(5000);
      if (checkboxes.length === 0) {
        throw new Error("No delete checkboxes found on the page");
      }

      for (const cb of checkboxes) {
        await cb.click();
        await desktop.delay(200);
      }

      const deleteButton = await desktop
        .locator("role:Link|name:Delete||role:Button|name:Delete")
        .first(5000);
      await deleteButton.click();

      const confirmInput = await desktop
        .locator("role:Edit||name:Confirm version||name:Confirm delete")
        .first(5000);
      await confirmInput.click();
      await confirmInput.typeText(version, { clear: true });

      const confirmButton = await desktop
        .locator("role:Button|name:Delete||name:Confirm")
        .first(5000);
      await confirmButton.click();

      await desktop.delay(4000);

      const currentUrl = await desktop.getCurrentUrl();
      if (
        currentUrl == `https://pypi.org/manage/project/${packageName}/releases/`
      ) {
        logger.success(`✅ Successfully deleted release ${version}`);

        return {
          deletedCheckboxes: checkboxes.length,
          deleted: true,
          version,
          packageName,
          redirectUrl: currentUrl,
        };
      }

      logger.warning(
        `⚠️ Delete completion suspected - unexpected URL: ${currentUrl}`
      );
      return {
        deletedCheckboxes: checkboxes.length,
        deleted: true,
        version,
        packageName,
        redirectUrl: currentUrl,
        warning: "Unexpected redirect URL",
      };
    } catch (error: any) {
      logger.error(`❌ Delete operation failed: ${error.message}`);
      throw error;
    }
  },
});
