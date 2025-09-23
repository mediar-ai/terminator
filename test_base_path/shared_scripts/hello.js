// Test script for scripts_base_path functionality
console.log("Hello from scripts_base_path!");
console.log("This script was loaded from:", __filename || "browser context");

// Return structured data to verify environment passing works
return {
    status: "success",
    message: "Script loaded from scripts_base_path",
    timestamp: new Date().toISOString(),
    source: "shared_scripts/hello.js"
};