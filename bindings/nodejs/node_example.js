const { NodeDesktop } = require('./index.js');

async function main() {
  const desktop = new NodeDesktop();
  console.log(desktop.hello());
  const root = desktop.root();
  console.log('Root element:', root.role, root.name);

  // List applications
  const apps = desktop.applications();
  console.log('Applications:', apps.map(a => [a.role, a.name]));

  // Find an element using a selector (async)
  const locator = desktop.locator('role:button');
  try {
    const button = await locator.first();
    console.log('Found button:', button.role, button.name);
    button.click();
  } catch (e) {
    if (e && e.code === 'ElementNotFoundError') {
      console.log('No button found:', e.message);
    } else {
      console.log('No button found or click failed:', e);
    }
  }

  // Run a command (async)
  const cmd = await desktop.runCommand('echo hello', 'echo hello');
  console.log('Command output:', cmd.stdout);

  // Screenshot (async)
  const screenshot = await desktop.captureScreen();
  console.log(`Screenshot: ${screenshot.width}x${screenshot.height}, ${screenshot.imageData.length} bytes`);

  // Error handling example
  try {
    desktop.application('NonExistentApp');
  } catch (e) {
    if (e && e.message && e.message.startsWith('ElementNotFoundError')) {
      console.log('Expected error:', e.message);
    } else {
      console.log('Expected error:', e);
    }
  }

  // Show help for NodeDesktop (prints method names)
  console.log('\nHelp for NodeDesktop:');
  console.log(Object.getOwnPropertyNames(NodeDesktop.prototype));
}

main().catch(err => {
  console.error('Fatal error:', err);
}); 