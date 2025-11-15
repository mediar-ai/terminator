import { createStep } from "@mediar-ai/workflow";
import { authenticator } from "otplib";

export const loginToPyPI = createStep({
  id: "login-pypi",
  name: "Login to PyPI",
  description: "Login to PyPI using credentials",

  execute: async ({ desktop, input, logger }: any) => {
    logger.info("🔐 Logging into PyPI...");

    try {
      await desktop.openBrowser("https://pypi.org/account/login/");
      await desktop.wait(2000);

      await desktop.type(input.username, {
        selector: 'input[name="username"]',
      });
      await desktop.wait(500);

      await desktop.type(input.password, {
        selector: 'input[name="password"]',
      });
      await desktop.wait(500);

      await desktop.click({ selector: 'input[type="submit"][value="Log in"]' });
      await desktop.wait(3000);

      const totpElements = await desktop.findElements({
        selector: 'input[type="text"][name="totp_value"][id="totp_value"]',
      });

      if (totpElements.length > 0) {
        logger.info("🔒 2FA required, generating TOTP code...");

        if (!input.totpSecret) {
          throw new Error("TOTP secret is required for 2FA but not provided");
        }

        const code = authenticator.generate(input.totpSecret);

        await desktop.type(code, {
          selector: 'input[type="text"][name="totp_value"][id="totp_value"]',
        });
        await desktop.wait(500);

        await desktop.click({
          selector: 'input[type="submit"][value="Verify"]',
        });
        await desktop.wait(3000);
      }

      const currentUrl = await desktop.getCurrentUrl();
      if (currentUrl == "https://pypi.org/") {
        logger.success("✅ Successfully logged into PyPI");
        return { success: true, data: { loggedIn: true } };
      } else {
        throw new Error("Login failed - not redirected to home page");
      }
    } catch (error: any) {
      logger.error(`❌ PyPI login failed: ${error.message}`);
      throw error;
    }
  },
});
