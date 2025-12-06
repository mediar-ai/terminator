import { authenticator } from "otplib";
import { BrowserContext, chromium, Page } from "playwright";

type ReleaseInfo = {
  version: string;
  uploadedAt: Date;
};

type PyPIResponse = {
  releases?: Record<
    string,
    Array<{
      upload_time?: string;
      upload_time_iso_8601?: string;
    }>
  >;
};

const PYPI_BASE_URL = "https://pypi.org";
const BROWSER_UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 " +
  "(KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36";

function readEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value;
}

async function humanPause(min = 300, max = 1100) {
  await new Promise((resolve) =>
    setTimeout(resolve, min + Math.random() * (max - min))
  );
}

async function fetchOldestRelease(
  packageName: string
): Promise<ReleaseInfo | null> {
  const response = await fetch(`${PYPI_BASE_URL}/pypi/${packageName}/json`, {
    headers: {
      "User-Agent": BROWSER_UA,
      Accept: "application/json",
      "Accept-Language": "en-US,en;q=0.9",
    },
  });

  if (!response.ok) {
    throw new Error(
      `Failed to fetch release metadata (${response.status} ${response.statusText})`
    );
  }

  const data = (await response.json()) as PyPIResponse;

  const releases = Object.entries(data.releases ?? {})
    .map(([version, files]) => {
      if (!files?.length) return null;

      const ts = files[0].upload_time_iso_8601 ?? files[0].upload_time ?? null;

      if (!ts) return null;

      return {
        version,
        uploadedAt: new Date(ts),
      } as ReleaseInfo;
    })
    .filter((r): r is ReleaseInfo => r !== null)
    .sort((a, b) => a.uploadedAt.getTime() - b.uploadedAt.getTime());

  return releases[0] ?? null;
}

async function loginToPyPI(
  page: Page,
  username: string,
  password: string,
  totpSecret: string
) {
  await page.goto(`${PYPI_BASE_URL}/account/login/`, {
    waitUntil: "networkidle",
  });
  await humanPause();

  await page.fill('input[name="username"]', username);
  await humanPause();

  await page.fill('input[name="password"]', password);
  await humanPause();

  await Promise.all([
    page.waitForNavigation({ waitUntil: "networkidle" }),
    page.locator('input[type="submit"][value="Log in"]').click(),
  ]);

  await humanPause();

  const totpInput = page.locator(
    'input[type="text"][name="totp_value"][id="totp_value"]'
  );
  if (await totpInput.count()) {
    const code = authenticator.generate(totpSecret);
    await totpInput.fill(code);
    await humanPause();

    await Promise.all([
      page.waitForNavigation({ waitUntil: "networkidle" }),
      page.locator('input[type="submit"][value="Verify"]').first().click(),
    ]);
  }

  await humanPause();

  // if menu bar not found = login success!
  const menuLink = page.locator("a.horizontal-menu__link");
  if (await menuLink.count()) {
    throw new Error("Login failed: Menu link was found after login attempt.");
  }
}

async function deleteRelease(page: Page, packageName: string, version: string) {
  const url = `${PYPI_BASE_URL}/manage/project/${packageName}/release/${version}/`;

  await page.goto(url, { waitUntil: "networkidle" });
  await humanPause();

  const checkboxes = page.locator(
    'input[type="checkbox"][data-action="input->delete-confirm#check"][data-delete-confirm-target="input"]'
  );

  const count = await checkboxes.count();
  if (count === 0) {
    throw new Error("Delete checkbox not found on release management page.");
  }

  for (let i = 0; i < count; i++) {
    await checkboxes.nth(i).check();
    await humanPause();
  }

  const deleteButton = page.locator(
    'a.button.button--danger[data-delete-confirm-target="button"]'
  );
  if (!(await deleteButton.count())) {
    throw new Error("Delete button not found on release management page.");
  }

  await deleteButton.click();
  await humanPause();

  const confirmInput = page.locator(
    'input[type="text"][id="delete_version-modal-confirm_delete_version"]'
  );
  if (!(await confirmInput.count())) {
    throw new Error(
      "Confirmation input not found on delete confirmation modal."
    );
  }

  await confirmInput.fill(version);
  await humanPause();

  const finalDeleteButton = page.locator(
    `#delete_version-modal button.js-confirm[data-expected="${version}"]`
  );

  if ((await finalDeleteButton.count()) === 0) {
    throw new Error(
      "Final delete button not found on delete confirmation modal."
    );
  }

  await Promise.all([
    page.waitForURL(/\/manage\/project\/.*\/releases\/?$/, { timeout: 15000 }),
    finalDeleteButton.click(),
  ]);

  await humanPause();
}

async function launchBrowser(): Promise<BrowserContext> {
  return await chromium.launchPersistentContext("/tmp/pypi-profile", {
    headless: true,
    viewport: { width: 1300, height: 840 },
    userAgent: BROWSER_UA,
    args: ["--no-sandbox", "--disable-dev-shm-usage"],
  });
}

async function main() {
  const username = readEnv("PYPI_UI_USERNAME");
  const password = readEnv("PYPI_UI_PASSWORD");
  const packageName = readEnv("PACKAGE_NAME");
  const totpSecret = readEnv("PYPI_UI_TOTP_SECRET");

  console.log(`Checking oldest release for ${packageName}...`);

  const oldest = await fetchOldestRelease(packageName);
  if (!oldest) {
    console.log("No releases found with files — nothing to delete.");
    return;
  }

  console.log(
    `Oldest release: ${
      oldest.version
    } (uploaded ${oldest.uploadedAt.toDateString()})`
  );

  const context = await launchBrowser();
  const page = context.pages()[0] ?? (await context.newPage());

  try {
    console.log("Logging into PyPI...");
    await loginToPyPI(page, username, password, totpSecret);
    console.log("Login successful.");

    console.log("Deleting oldest release...");
    await deleteRelease(page, packageName, oldest.version);
    console.log(`Deleted release ${oldest.version}.`);
  } finally {
    await context.close();
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
