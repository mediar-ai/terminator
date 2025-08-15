#!/usr/bin/env node
/**
 * Enhanced Highlight Demo with Text Overlays
 * 
 * This demo shows the new enhanced highlight functionality with text overlays.
 * Run this with Calculator open to see text overlays in different positions.
 * 
 * Requirements: 
 * - Build the Node.js bindings: cd bindings/nodejs && npm install
 * - Or install terminator package if available
 * - npm install -g tsx (to run TypeScript directly)
 * 
 * Usage: tsx enhanced_highlight_demo.ts
 */

import { dirname, resolve } from 'path';
import { fileURLToPath } from 'url';

// Add the bindings to require path
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const bindingsPath = resolve(__dirname, '..', 'bindings', 'nodejs');

let terminator: any;
try {
    terminator = require(bindingsPath);
} catch (error) {
    console.error('❌ Terminator not found. Please build the Node.js bindings first:');
    console.error('   cd bindings/nodejs && npm install');
    process.exit(1);
}

const { 
    Desktop,
    ElementNotFoundError,
    PlatformError,
    TextPosition,
    FontStyle
} = terminator;

function sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
}

async function main(): Promise<void> {
    console.log('🎯 Enhanced Highlight Demo - Text Overlays');
    console.log('='.repeat(50));
    
    try {
        const desktop = new Desktop();
        
        // Step 1: Open Calculator
        console.log('\n1. Opening Calculator...');
        await desktop.runCommand('calc.exe', 'calc.exe');
        await sleep(2000); // Wait for Calculator to open
        
        // Step 2: Find Calculator window and buttons
        console.log('\n2. Finding Calculator elements...');
        
        // Find the Calculator application
        const apps = desktop.applications();
        let calculator = null;
        for (const app of apps) {
            if (app.name().toLowerCase().includes('calculator')) {
                calculator = app;
                break;
            }
        }
        
        if (!calculator) {
            console.log('❌ Calculator not found. Please open Calculator manually and try again.');
            return;
        }
        
        console.log(`✅ Found Calculator: ${calculator.name()}`);
        
        // Find some number buttons
        const buttonNumbers = ['1', '2', '3', '4', '5'];
        const buttons: Array<{ number: string, element: any }> = [];
        
        for (const num of buttonNumbers) {
            try {
                const locator = desktop.locator(`name:${num}`);
                const button = await locator.first();
                buttons.push({ number: num, element: button });
                console.log(`✅ Found button: ${num}`);
            } catch (e) {
                console.log(`⚠️  Button ${num} not found`);
            }
        }
        
        if (buttons.length === 0) {
            console.log('❌ No calculator buttons found');
            return;
        }
        
        // Step 3: Demo different text overlay positions
        console.log('\n3. Testing different text overlay positions...');
        
        const demos = [
            { text: 'TOP', position: TextPosition.Top, style: { size: 16, bold: true, color: 0x000000 } },
            { text: 'RIGHT', position: TextPosition.Right, style: { size: 14, bold: false, color: 0x0000FF } },
            { text: 'BOTTOM', position: TextPosition.Bottom, style: { size: 18, bold: true, color: 0x008000 } },
            { text: 'LEFT', position: TextPosition.Left, style: { size: 12, bold: false, color: 0xFF0000 } },
            { text: 'INSIDE', position: TextPosition.Inside, style: { size: 20, bold: true, color: 0x800080 } },
        ];
        
        for (let i = 0; i < Math.min(demos.length, buttons.length); i++) {
            const demo = demos[i];
            const button = buttons[i];
            
            console.log(`\n   🔸 Testing ${demo.position} position with text '${demo.text}' on button ${button.number}`);
            
            // Create FontStyle object
            const fontStyle = new FontStyle();
            fontStyle.size = demo.style.size;
            fontStyle.bold = demo.style.bold;
            fontStyle.color = demo.style.color;
            
            // Highlight with text overlay
            const handle = button.element.highlight(
                0x00FF00,  // Green border (color)
                3000,      // 3 seconds (duration_ms)
                demo.text, // text
                demo.position, // text_position
                fontStyle  // font_style
            );
            
            // Wait to see the highlight
            await sleep(3500);
            
            // Demonstrate manual closing for the last one
            if (i === demos.length - 1 || i === buttons.length - 1) {
                console.log('   📝 Manually closing highlight...');
                handle.close();
                await sleep(1000);
            }
        }
        
        // Step 4: Test different colors and styles
        console.log('\n4. Testing different font styles and colors...');
        
        const styledemos = [
            { text: 'BOLD', style: { size: 24, bold: true, color: 0x000000 } },
            { text: 'BIG', style: { size: 32, bold: false, color: 0x800000 } },
            { text: 'SMALL', style: { size: 10, bold: true, color: 0x008080 } },
        ];
        
        for (let i = 0; i < Math.min(styledemos.length, buttons.length); i++) {
            const demo = styledemos[i];
            const button = buttons[i];
            
            console.log(`   🔸 Testing font style: ${demo.text} on button ${button.number}`);
            
            const fontStyle = new FontStyle();
            fontStyle.size = demo.style.size;
            fontStyle.bold = demo.style.bold;
            fontStyle.color = demo.style.color;
            
            const handle = button.element.highlight(
                0x0000FF,      // Blue border
                2000,          // 2 seconds
                demo.text,
                TextPosition.Top,
                fontStyle
            );
            
            await sleep(2500);
        }
        
        // Step 5: Demo longer text with truncation
        console.log('\n5. Testing text truncation...');
        
        if (buttons.length > 0) {
            const fontStyle = new FontStyle();
            fontStyle.size = 14;
            fontStyle.bold = true;
            fontStyle.color = 0x000000;
            
            const handle = buttons[0].element.highlight(
                0xFF00FF,      // Magenta border
                4000,          // 4 seconds
                'This text will be truncated to 10 chars', // Long text
                TextPosition.Bottom,
                fontStyle
            );
            
            console.log('   🔸 Testing text truncation (should show "This te...")');
            await sleep(4500);
        }
        
        console.log('\n🎉 Demo completed successfully!');
        console.log('\nKey features demonstrated:');
        console.log('  ✅ Text overlays in different positions');
        console.log('  ✅ Custom font sizes, colors, and bold styling');
        console.log('  ✅ Manual highlight closing');
        console.log('  ✅ Text truncation to 10 characters');
        console.log('  ✅ White background for text visibility');
        
    } catch (error) {
        console.error('❌ Error during demo:', error);
        if (error instanceof Error) {
            console.error(error.stack);
        }
    }
}

// Handle unhandled promise rejections
process.on('unhandledRejection', (reason, promise) => {
    console.error('Unhandled Rejection at:', promise, 'reason:', reason);
});

main().catch(err => {
    console.error('Fatal error:', err);
    process.exit(1);
});