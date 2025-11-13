// src/scripts/get-versions.js
(function (env) {
  try {
    // Find all version elements on the page
    const versionElements = document.querySelectorAll(
      "[data-version], .package-snippet__version, .release__version"
    );

    if (versionElements.length === 0) {
      // Try alternative selectors
      const rows = document.querySelectorAll("table tbody tr");
      const versions = [];

      rows.forEach((row, index) => {
        const versionCell = row.querySelector("td:first-child, th:first-child");
        if (versionCell) {
          const versionText = versionCell.textContent.trim();
          if (versionText && versionText.match(/^\d+\.\d+/)) {
            versions.push({
              version: versionText,
              index: index,
            });
          }
        }
      });

      return JSON.stringify({
        ok: true,
        data: versions,
        packageName: env && env.packageName,
      });
    }

    const versions = Array.from(versionElements).map((el, index) => ({
      version: el.textContent.trim() || el.getAttribute("data-version"),
      index: index,
    }));

    return JSON.stringify({
      ok: true,
      data: versions,
      packageName: env && env.packageName,
    });
  } catch (e) {
    return JSON.stringify({
      ok: false,
      error: String(e),
      message: "Failed to extract versions from page",
    });
  }
});
