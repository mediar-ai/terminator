// src/steps/02-login-to-pypi.ts
import { createStep } from "@mediar-ai/workflow";

export const loginToPyPI = createStep({
  id: "login_to_pypi",
  name: "Login to PyPI Account",
  execute: async ({ context, input, logger }) => {
    logger.info("🔐 Logging into PyPI...");

    const browser = context.data.browser;
    if (!browser) throw new Error("Browser element missing from context");

    try {
      // Wait for login form to be ready
      await new Promise((r) => setTimeout(r, 1000));

      // Fill username using browser script
      const usernameResult = await browser.executeBrowserScript(`
        const usernameInput = document.querySelector('input[name="username"]');
        if (!usernameInput) throw new Error('Username input not found');
        usernameInput.value = '${input.pypiUsername}';
        usernameInput.dispatchEvent(new Event('input', { bubbles: true }));
        'Username filled'
      `);

      logger.info(`   ${usernameResult}`);
      await new Promise((r) => setTimeout(r, 500));

      // Fill password
      const passwordResult = await browser.executeBrowserScript(`
        const passwordInput = document.querySelector('input[name="password"]');
        if (!passwordInput) throw new Error('Password input not found');
        passwordInput.value = '${input.pypiPassword}';
        passwordInput.dispatchEvent(new Event('input', { bubbles: true }));
        'Password filled'
      `);

      logger.info(`   ${passwordResult}`);
      await new Promise((r) => setTimeout(r, 500));

      // Submit form
      const submitResult = await browser.executeBrowserScript(`
        const submitBtn = document.querySelector('button[type="submit"]');
        if (!submitBtn) throw new Error('Submit button not found');
        submitBtn.click();
        'Login form submitted'
      `);

      logger.info(`   ${submitResult}`);

      // Wait for login to complete
      await new Promise((r) => setTimeout(r, 3000));

      // Verify login success
      const loginCheck = await browser.executeBrowserScript(`
        const url = window.location.href;
        if (url.includes('/account/login/')) {
          throw new Error('Login failed - still on login page');
        }
        'Login successful'
      `);

      logger.success(`✅ ${loginCheck}`);

      return {
        state: {
          loggedIn: true,
        },
      };
    } catch (error: any) {
      logger.error(`❌ Login failed: ${error.message}`);
      throw error;
    }
  },
});
