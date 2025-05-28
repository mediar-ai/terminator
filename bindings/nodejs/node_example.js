const { 
    Desktop,
    ElementNotFoundError,
    PlatformError 
} = require('.');

async function main() {
  const desktop = new Desktop();
  const root = desktop.root();
  console.log('Root element:', root.role(), root.name());

  // List applications
  const apps = desktop.applications();
  console.log('Applications:', apps.map(a => [a.role(), a.name()]));

  // Find an element using a selector (async)
  const locator = desktop.locator('role:button');
  try {
    const button = await locator.first();
    console.log('Found button:', button.role(), button.name());
    await button.click();
  } catch (e) {
    if (e instanceof ElementNotFoundError) {
      console.log('No button found:', e.message);
    } else {
      console.log('No button found or click failed:', e);
    }
  }

  // Run a command (async)
  try {
    const cmd = await desktop.runCommand('echo hello', 'echo hello');
    console.log('Command output:', cmd.stdout);
  } catch (e) {
    console.log('Command failed:', e);
  }

  // Screenshot (async)
  try {
    const screenshot = await desktop.captureScreen();
    console.log(`Screenshot: ${screenshot.width}x${screenshot.height}, ${screenshot.imageData.length} bytes`);
  } catch (e) {
    console.log('Screenshot failed:', e);
  }

  // Error handling example
  try {
    desktop.application('NonExistentApp');
  } catch (e) {
    if (e instanceof PlatformError) {
      console.log('Expected error:', e.message);
    } else {
      console.log('Unexpected error:', e);
    }
  }

  // Show help for Desktop (prints method names)
  console.log('\nHelp for Desktop:');
  console.log(Object.getOwnPropertyNames(Desktop.prototype));
}

main().catch(err => {
  console.error('Fatal error:', err);
}); 