/**
 * Test runner for strip-styles browser script using terminator SDK
 *
 * Usage:
 *   npx tsx run-strip-styles.ts [url]
 *
 * Examples:
 *   npx tsx run-strip-styles.ts                     # Use current Chrome page
 *   npx tsx run-strip-styles.ts https://reddit.com  # Navigate to URL first
 */

import { Desktop } from "@mediar-ai/terminator";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

async function main() {
  const url = process.argv[2];
  const scriptPath = path.join(__dirname, "strip-styles.js");

  console.log("🚀 Starting strip-styles automation...");

  const desktop = new Desktop();

  // Get Chrome window
  const chrome = desktop.application("chrome").window();
  if (!chrome) {
    console.error("❌ Chrome not found. Please open Chrome first.");
    process.exit(1);
  }

  chrome.focus();
  console.log("✅ Found Chrome window");

  // Navigate if URL provided
  if (url) {
    console.log(`📍 Navigating to: ${url}`);
    console.log("   Please navigate manually to the URL and run again without URL argument.");
    console.log("   Or navigate in Chrome yourself, then run: npx tsx run-strip-styles.ts");
    process.exit(0);
  }

  // Read script content directly (workaround for wrapper.ts bug with file paths)
  const scriptContent = fs.readFileSync(scriptPath, "utf-8");
  console.log(`📜 Executing browser script from: ${scriptPath}`);

  try {
    // Pass script as string directly, not as file path
    const result = await desktop.executeBrowserScript(scriptContent, "chrome", 30000);
    console.log("✅ Script executed successfully!");
    console.log("📊 Result:", result);
  } catch (error: any) {
    console.error("❌ Script execution failed:", error.message);
    process.exit(1);
  }

  console.log("\n🎉 Done! The page should now be in minimal/markdown-like mode.");
  console.log("💡 Tip: Refresh the page (F5) to restore original styles.");
}

main().catch(console.error);
