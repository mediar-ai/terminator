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
      const deleteCheckboxes = await desktop.findElements({
        selector:
          'input[type="checkbox"][data-action="input->delete-confirm#check"][data-delete-confirm-target="input"]',
      });

      if (deleteCheckboxes.length === 0) {
        throw new Error("No delete checkboxes found on the page");
      }

      logger.info(`Checking ${deleteCheckboxes.length} delete checkbox(es)...`);

      for (let i = 0; i < deleteCheckboxes.length; i++) {
        await desktop.click({
          elementIndex: i,
          selector:
            'input[type="checkbox"][data-action="input->delete-confirm#check"][data-delete-confirm-target="input"]',
        });
        await desktop.wait(500);
      }

      await desktop.click({
        selector:
          'a.button.button--danger[data-delete-confirm-target="button"]',
      });
      await desktop.wait(1000);

      await desktop.type(version, {
        selector:
          'input[type="text"][id="delete_version-modal-confirm_delete_version"]',
      });
      await desktop.wait(500);

      await desktop.click({
        selector: `#delete_version-modal button.js-confirm[data-expected="${version}"]`,
      });

      await desktop.wait(3000);

      const currentUrl = await desktop.getCurrentUrl();
      if (
        currentUrl == `https://pypi.org/manage/project/${packageName}/releases/`
      ) {
        logger.success(`✅ Successfully deleted release ${version}`);

        return {
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
