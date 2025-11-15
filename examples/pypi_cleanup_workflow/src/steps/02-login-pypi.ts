import { createStep } from "@mediar-ai/workflow";
import { authenticator } from "otplib";

export const loginToPyPI = createStep({
  id: "login-pypi",
  name: "Login to PyPI",
  description: "Login to PyPI using credentials",

  execute: async ({ desktop, input, logger }: any) => {
    logger.info("🔐 Logging into PyPI...");

    try {
      await desktop.navigateBrowser(
        "https://pypi.org/account/login/",
        "Chrome"
      );
      await desktop.delay(2000);

      // Fill username/password and submit via browser script
      await desktop.executeBrowserScript(
        ({ username, password }: { username: string; password: string }) => {
          const usernameInput = document.querySelector<HTMLInputElement>(
            'input[name="username"]'
          );
          const passwordInput = document.querySelector<HTMLInputElement>(
            'input[name="password"]'
          );
          const loginButton = document.querySelector<HTMLInputElement>(
            'form[action="/account/login/"] input[type="submit"]'
          );

          if (!usernameInput || !passwordInput || !loginButton) {
            throw new Error("Unable to locate login form fields");
          }

          usernameInput.focus();
          usernameInput.value = username;
          usernameInput.dispatchEvent(new Event("input", { bubbles: true }));

          passwordInput.focus();
          passwordInput.value = password;
          passwordInput.dispatchEvent(new Event("input", { bubbles: true }));

          loginButton.click();
          return true;
        },
        {
          username: input.username,
          password: input.password,
        }
      );

      await desktop.delay(3000);

      const totpRequired = (await desktop.executeBrowserScript(() => {
        return Boolean(
          document.querySelector<HTMLInputElement>(
            'input[type="text"][name="totp_value"][id="totp_value"]'
          )
        );
      })) as boolean;

      if (totpRequired) {
        logger.info("🔒 2FA required, generating TOTP code...");

        if (!input.totpSecret) {
          throw new Error("TOTP secret is required for 2FA but not provided");
        }

        const code = authenticator.generate(input.totpSecret);

        await desktop.executeBrowserScript(
          ({ totpCode }: { totpCode: string }) => {
            const totpInput = document.querySelector<HTMLInputElement>(
              'input[type="text"][name="totp_value"][id="totp_value"]'
            );
            const verifyButton = document.querySelector<HTMLInputElement>(
              'input[type="submit"][value="Verify"]'
            );

            if (!totpInput || !verifyButton) {
              throw new Error("Unable to locate TOTP verification fields");
            }

            totpInput.focus();
            totpInput.value = totpCode;
            totpInput.dispatchEvent(new Event("input", { bubbles: true }));
            verifyButton.click();
            return true;
          },
          { totpCode: code }
        );

        await desktop.delay(3000);
      }

      const currentUrl = (await desktop.executeBrowserScript(() => {
        return window.location.href;
      })) as string;

      if (currentUrl.startsWith("https://pypi.org/")) {
        logger.success("✅ Successfully logged into PyPI");
        return { success: true, data: { loggedIn: true } };
      }

      throw new Error(
        `Login failed - unexpected redirect after authentication: ${currentUrl}`
      );
    } catch (error: any) {
      logger.error(`❌ PyPI login failed: ${error.message}`);
      throw error;
    }
  },
});
