// examples/win_calculator.ts
// Automate Windows Calculator using the new Desktop/Element/Locator API
// This example matches the style and logic of win_calculator.py

import { Desktop } from '../bindings/nodejs';

// Utility sleep function
function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Find the element with AutomationId 'CalculatorResults' within the Group element.
 * @param calculatorWindow The calculator window locator
 * @returns A locator for the CalculatorResults element or null if not found
 */
async function findCalculatorResults(calculatorWindow: ReturnType<Desktop['locator']>): Promise<ReturnType<Desktop['locator']> | null> {
    // Get the display element and explore result
    const displayElement = calculatorWindow.locator("Id:CalculatorResults");
    const exploreResult = await displayElement.explore();
    // Find the Group element
    for (const child of exploreResult.children) {
        if (child.role === 'Group' && child.suggestedSelector) {
            // Get the Group element's children
            const groupResult = await calculatorWindow.locator(child.suggestedSelector).explore();
            // Search for CalculatorResults within the Group's children
            for (const groupChild of groupResult.children) {
                if (groupChild.suggestedSelector) {
                    const childLocator = calculatorWindow.locator(groupChild.suggestedSelector);
                    const childAttrs = await childLocator.attributes();
                    if (childAttrs.properties &&
                        childAttrs.properties.AutomationId &&
                        String(childAttrs.properties.AutomationId).includes('CalculatorResults')) {
                        return childLocator;
                    }
                }
            }
        }
    }
    return null;
}

async function runCalculator() {
    const desktop = new Desktop(undefined, undefined, 'error');
    try {
        // 1. Open Calculator
        console.log("Opening Calculator...");
        desktop.openApplication("uwp:Microsoft.WindowsCalculator");
        await sleep(2000); // Allow app to open

        // 2. Create locators for UI elements
        const calculatorWindow = desktop.locator("window:Calculator");
        let displayElement = calculatorWindow.locator("Id:CalculatorResults");
        const button1 = calculatorWindow.locator("Name:One");
        const buttonPlus = calculatorWindow.locator("Name:Plus");
        const button2 = calculatorWindow.locator("Name:Two");
        const buttonEquals = calculatorWindow.locator("Name:Equals");

        // 3. Get initial display text
        console.log("Getting initial display text...");
        let needsCalculatorResults = false;
        try {
            const displayElementAttributes = await displayElement.attributes();
            needsCalculatorResults = !displayElementAttributes.properties ||
                !displayElementAttributes.properties.AutomationId ||
                !String(displayElementAttributes.properties.AutomationId).includes('CalculatorResults');
            if (needsCalculatorResults) {
                console.log("Finding CalculatorResults element...");
                const found = await findCalculatorResults(calculatorWindow);
                if (found) {
                    displayElement = found;
                    const text = await displayElement.name();
                    console.log(`Text: ${text}`);
                } else {
                    console.log("Could not find element with AutomationId 'CalculatorResults'");
                }
            } else {
                console.log("Element already has AutomationId 'CalculatorResults'");
            }
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
            if (needsCalculatorResults) {
                const found = await findCalculatorResults(calculatorWindow);
                if (found) {
                    displayElement = found;
                } else {
                    console.log("Could not find CalculatorResults element for verification");
                    return;
                }
            }
            const textResponse = await displayElement.name();
            if (textResponse === "Display is 3") {
                console.log("Final display text is verified to be 'Display is 3'");
            } else if (textResponse === "3") {
                console.log("Final display text is verified to be '3'");
            } else {
                console.log(`Unexpected text: ${textResponse}`);
            }
            const finalText = await displayElement.name();
            console.log(`Final display text (raw): ${finalText}`);
        } catch (e) {
            console.error(`Verification failed or could not get final text: ${e instanceof Error ? e.message : e}`);
            try {
                const rawText = await displayElement.name();
                console.error(`Raw text on failure: ${rawText}`);
            } catch (innerErr) {
                console.error(`Could not get raw text after verification failure: ${innerErr instanceof Error ? innerErr.message : innerErr}`);
            }
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
