// examples/win_calculator.ts
// Automate Windows Calculator using the new Desktop/Element/Locator API
// This example matches the style and logic of win_calculator.py

import { Desktop } from '../bindings/nodejs';

// Utility sleep function
function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

async function runCalculator() {
    const desktop = new Desktop(undefined, undefined, 'error');
    try {
        // 1. Open Calculator
        console.log("Opening Calculator...");
        const calculatorWindow = desktop.openApplication("uwp:Microsoft.WindowsCalculator");
        await sleep(2000); // Allow app to open

        // 2. Create locators for UI elements
        const displayElement = calculatorWindow.locator("nativeid:CalculatorResults");
        const button1 = await calculatorWindow.locator("Name:One").first();
        const buttonPlus = await calculatorWindow.locator("Name:Plus").first();
        const button2 = await calculatorWindow.locator("Name:Two").first();
        const buttonEquals = await calculatorWindow.locator("Name:Equals").first();

        // 3. Get initial display text
        console.log("Getting initial display text...");
        try {
            const element = await displayElement.first();
            const text = await element.name();
            console.log(`Text: ${text}`);
        } catch (e) {
            console.warn(`Warning: Could not get initial display text: ${e instanceof Error ? e.message : e}`);
        }

        // 4. Perform clicks (1 + 2 =)
        console.log("Performing clicks: 1 + 2 =");
        await button1.click();
        await sleep(500);
        await buttonPlus.click();
        await sleep(500);
        await button2.click();
        await sleep(500);
        await buttonEquals.click();
        await sleep(1000); // Wait for calculation

        // 5. Get final text & verify
        console.log("Verifying final text (expecting 3)...");
        try {
            const element = await displayElement.first();
            const textResponse = await element.name();
            if (textResponse === "Display is 3") {
                console.log("Final display text is verified to be 'Display is 3'");
            } else if (textResponse === "3") {
                console.log("Final display text is verified to be '3'");
            } else {
                console.log(`Unexpected text: ${textResponse}`);
            }
        } catch (e) {
            console.error(`Verification failed or could not get final text: ${e instanceof Error ? e.message : e}`);
        }

        // Example: Get attributes of the equals button
        console.log("Getting attributes of '=' button...");
        const attrs = await buttonEquals.attributes();
        console.log(`Equals button attributes: ${JSON.stringify(attrs, null, 2)}`);

        // Example: Check visibility of the equals button
        console.log("Checking visibility of '=' button...");
        const isVisible = await buttonEquals.isVisible();
        console.log(`Is Equals button visible? ${isVisible}`);

    } catch (e) {
        console.error(`An unexpected error occurred: ${e instanceof Error ? e.message : e}`);
    }
}

// Entry point
runCalculator();
