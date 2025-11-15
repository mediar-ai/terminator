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
      const result = (await desktop.executeBrowserScript(
        ({
          version: versionToDelete,
        }: {
          version: string;
        }) => {
          const deleteCheckboxes = Array.from(
            document.querySelectorAll<HTMLInputElement>(
              'input[type="checkbox"][data-action="input->delete-confirm#check"][data-delete-confirm-target="input"]'
            )
          );

          if (deleteCheckboxes.length === 0) {
            throw new Error("No delete checkboxes found on the page");
          }

          deleteCheckboxes.forEach((checkbox) => {
            if (!checkbox.checked) {
              checkbox.click();
            }
          });

          const deleteButton = document.querySelector<HTMLAnchorElement>(
            'a.button.button--danger[data-delete-confirm-target="button"]'
          );
          if (!deleteButton) {
            throw new Error("Delete button not found");
          }
          deleteButton.click();

          const confirmInput = document.querySelector<HTMLInputElement>(
            "#delete_version-modal-confirm_delete_version"
          );
          const confirmButton = document.querySelector<HTMLButtonElement>(
            `#delete_version-modal button.js-confirm[data-expected="${versionToDelete}"]`
          );

          if (!confirmInput || !confirmButton) {
            throw new Error("Delete confirmation modal not found");
          }

          confirmInput.focus();
          confirmInput.value = versionToDelete;
          confirmInput.dispatchEvent(new Event("input", { bubbles: true }));
          confirmButton.click();

          return { checkboxCount: deleteCheckboxes.length };
        },
        { version }
      )) as { checkboxCount: number };

      await desktop.delay(3000);

      const currentUrl = await desktop.executeBrowserScript(() => {
        return window.location.href;
      });
      if (
        currentUrl == `https://pypi.org/manage/project/${packageName}/releases/`
      ) {
        logger.success(`✅ Successfully deleted release ${version}`);

        return {
          deleted: true,
          version,
          packageName,
          redirectUrl: currentUrl,
          deletedCheckboxes: result.checkboxCount,
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
        deletedCheckboxes: result.checkboxCount,
      };
    } catch (error: any) {
      logger.error(`❌ Delete operation failed: ${error.message}`);
      throw error;
    }
  },
});
