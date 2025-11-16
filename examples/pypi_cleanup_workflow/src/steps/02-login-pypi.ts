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
      await desktop.delay(3000);

      // Fill username/password via accessibility locators
      const usernameField = await desktop
        .locator("role:Edit|name:Username||name:Email")
        .first(10000);
      await usernameField.click();
      await desktop.pressKey("Ctrl+A");
      await usernameField.typeText(input.username);

      const passwordField = await desktop.locator("role:Edit|name:Password").first(10000);
      await passwordField.click();
      await desktop.pressKey("Ctrl+A");
      await passwordField.typeText(input.password);

      const loginButton = await desktop.locator("role:Button|name:Log in").first(5000);
      await loginButton.click();

      await desktop.delay(4000);

      // Handle TOTP if present
      const totpFieldResult = await desktop
        .locator("role:Edit|name:Authentication code||name:TOTP||name:Verification code")
        .validate(3000);
      if (totpFieldResult.exists) {
        logger.info("🔒 2FA required, generating TOTP code...");
        if (!input.totpSecret) {
          throw new Error("TOTP secret is required for 2FA but not provided");
        }
        const code = authenticator.generate(input.totpSecret);
        const totpField = totpFieldResult.element!;
        await totpField.click();
        await desktop.pressKey("Ctrl+A");
        await totpField.typeText(code);
        const verifyButton = await desktop
          .locator("role:Button|name:Verify||name:Continue")
          .first(5000);
        await verifyButton.click();
        await desktop.delay(4000);
      }

      // Basic success check (account menu or username on page)
      const successCheck =
        (await desktop
          .locator("name:Account settings||name:Logout||name:Log out")
          .validate(3000)).exists ||
        (await desktop.locator(`text:${input.username}`).validate(3000)).exists;
      if (successCheck) {
        logger.success("✅ Successfully logged into PyPI");
        return { success: true, data: { loggedIn: true } };
      }

      throw new Error("Login failed - expected account elements not found");
    } catch (error: any) {
      logger.error(`❌ PyPI login failed: ${error.message}`);
      throw error;
    }
  },
});
