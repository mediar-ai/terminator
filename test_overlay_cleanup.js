#!/usr/bin/env node
/**
 * Test overlay window cleanup fix
 * This test rapidly creates and destroys highlights to verify no ghost windows remain
 */

const terminator = require('./bindings/nodejs');
const { Desktop } = terminator;

function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

async function main() {
    console.log('🧪 Testing Overlay Window Cleanup Fix');
    console.log('='.repeat(60));

    const desktop = new Desktop();

    // Open Calculator
    console.log('\n1. Opening Calculator...');
    await desktop.runCommand('calc.exe', 'calc.exe');
    await sleep(2000);

    // Find Calculator
    const apps = desktop.applications();
    let calculator = null;
    for (const app of apps) {
        if (app.name().toLowerCase().includes('calculator')) {
            calculator = app;
            break;
        }
    }

    if (!calculator) {
        console.log('❌ Calculator not found');
        return;
    }

    console.log(`✅ Found Calculator: ${calculator.name()}`);

    // Find a button to highlight
    try {
        const locator = desktop.locator('name:1');
        const button = await locator.first();

        console.log('\n2. Testing rapid highlights (ghost window test)...');
        console.log('   Creating 10 highlights rapidly - old windows should be cleaned up');

        // Rapidly create highlights - each should clean up the previous one
        for (let i = 0; i < 10; i++) {
            console.log(`   → Highlight ${i + 1}/10`);
            const fontStyle = {
                size: 14,
                bold: true,
                color: 0x000000
            };

            const handle = button.highlight(
                0x00FF00,  // Green
                500,       // 500ms duration
                `Test ${i}`,
                'Top',     // TextPosition.Top
                fontStyle
            );
            await sleep(200); // Overlap highlights intentionally
        }

        console.log('\n3. Waiting for all highlights to expire...');
        await sleep(2000);

        console.log('\n✅ Test complete!');
        console.log('\n📝 Expected behavior:');
        console.log('   - Only ONE overlay window should exist at a time');
        console.log('   - All overlays should be cleaned up after expiration');
        console.log('   - NO ghost/steamy mirror effects should remain');
        console.log('\n🔍 Check your screen - is it clean? No ghost artifacts?');

    } catch (error) {
        console.error('❌ Error:', error);
    }
}

main().catch(err => {
    console.error('Fatal error:', err);
    process.exit(1);
});
