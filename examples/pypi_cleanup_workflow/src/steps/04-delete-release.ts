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

      // Scroll to bottom to make sure controls exist
      for (let i = 0; i < 8; i++) {
        await desktop.pressKey("End");
        await desktop.delay(200);
      }

      const checkboxes = await desktop.locator("role:CheckBox").all(8000);
      if (checkboxes.length === 0) {
        throw new Error("No delete checkboxes found on the page");
      }

      for (const cb of checkboxes) {
        await cb.click();
        await desktop.delay(200);
      }

      const deleteButton = await desktop
        .locator(
          "role:Button|name:Delete release||role:Link|name:Delete release||role:Button|name:Delete||role:Link|name:Delete"
        )
        .first(10000);
      await deleteButton.click();

      const confirmInput = await desktop
        .locator(
          "role:Edit|name:Confirm||role:Edit|name:Confirm delete||role:Edit|name:Confirm version"
        )
        .first(8000);
      await confirmInput.click();
      await desktop.pressKey("Ctrl+A");
      // PyPI typically requires the project name to confirm deletion
      await confirmInput.typeText(packageName);

      const confirmButton = await desktop
        .locator("role:Button|name:Delete release||role:Button|name:Delete")
        .first(8000);
      await confirmButton.click();

      await desktop.delay(5000);

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
