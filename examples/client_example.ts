// examples/client_example.ts
// Import necessary components from the installed SDK
import { DesktopUseClient, ApiError, sleep, Locator } from '../ts-sdk/src/index'; // Adjust if using package name after build/install

// --- Example Usage using the SDK ---

/**
 * Find the element with AutomationId 'CalculatorResults' within the Group element.
 * @param calculatorWindow The calculator window locator
 * @returns A locator for the CalculatorResults element or null if not found
 */
async function findCalculatorResults(calculatorWindow: Locator): Promise<Locator | null> {
    // Get the display element and explore result
    const displayElement = calculatorWindow.locator("Id:CalculatorResults");
    const exploreResult = await displayElement.explore();
    
    // Find the Group element
    for (const child of exploreResult.children) {
        if (child.role === 'Group' && child.suggested_selector) {
            // Get the Group element's children
            const groupResult = await calculatorWindow.locator(child.suggested_selector).explore();
            
            // Search for CalculatorResults within the Group's children
            for (const groupChild of groupResult.children) {
                if (groupChild.suggested_selector) {
                    const childLocator = calculatorWindow.locator(groupChild.suggested_selector);
                    const childAttrs = await childLocator.getAttributes();
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

// Use an async IIFE (Immediately Invoked Function Expression) to allow top-level await
(async () => {
    // Ensure the Terminator server (e.g., examples/server.rs) is running!
    const client = new DesktopUseClient();

    try {
        // 1. Open Calculator
        console.log("\n--- 1. Opening Application ---");
        // Adjust app name if necessary (e.g., 'Calculator' or 'calc' on Windows)
        await client.openApplication("Calc");
        await sleep(2000); // Allow app to open

        // 2. Create locators using chaining from the SDK
        console.log("\n--- 2. Defining Locators ---");
        const calculatorWindow = client.locator("window:Calculator"); // Adjust selector if window title is different
        // Locators relative to the calculator window
        // IMPORTANT: Selectors might differ significantly on non-Windows platforms or even Win versions
        let displayElement = calculatorWindow.locator("Id:CalculatorResults"); // Using AutomationId is often more stable
        const button1 = calculatorWindow.locator("Name:One");
        const buttonPlus = calculatorWindow.locator("Name:Plus");
        const button2 = calculatorWindow.locator("Name:Two");
        const buttonEquals = calculatorWindow.locator("Name:Equals");

        // 3. Get initial text
        console.log("\n--- 3. Getting Initial Text ---");
        let needsCalculatorResults = false; // Initialize the variable
        try {
            // Get the explore result
            const displayElementAttributes = await displayElement.getAttributes();
            
            // Remember if we need to find CalculatorResults
            needsCalculatorResults = !displayElementAttributes.properties || 
                !displayElementAttributes.properties.AutomationId || 
                !String(displayElementAttributes.properties.AutomationId).includes('CalculatorResults');
            
            // Only proceed if not already CalculatorResults
            if (needsCalculatorResults) {
                // Find the element with AutomationId CalculatorResults
                const foundElement = await findCalculatorResults(calculatorWindow);
                if (foundElement) {
                    displayElement = foundElement;
                    const textResponse = await displayElement.getText();
                    console.log(`Text: ${textResponse?.text}`);
                } else {
                    console.log("Could not find element with AutomationId 'CalculatorResults'");
                }
            } else {
                console.log("Element already has AutomationId 'CalculatorResults'");
            }
                
        } catch (e) {
            console.warn(`Could not get initial display text: ${e instanceof Error ? e.message : e}`);
        }

        // 4. Perform clicks (1 + 2 =)
        console.log("\n--- 4. Performing Clicks --- (1 + 2 =)");
        await button1.click();
        await sleep(500);
        await buttonPlus.click();
        await sleep(500);
        await button2.click();
        await sleep(500);
        await buttonEquals.click();
        await sleep(1000); // Wait for calculation

        // 5. Get final text & Verify using expect
        console.log("\n--- 5. Verifying Final Text --- (Expecting 3)");
        try {
            // If we needed to find CalculatorResults earlier, find it again as it might be unstable
            if (needsCalculatorResults) {
                const foundElement = await findCalculatorResults(calculatorWindow);
                if (foundElement) {
                    displayElement = foundElement;
                } else {
                    throw new Error("Could not find CalculatorResults element for verification");
                }
            }

            // Get the text and verify it
            const textResponse = await displayElement.getText();
            if (textResponse?.text === "Display is 3") {
                await displayElement.expectTextEquals("Display is 3", { timeout: 5000, maxDepth: 1 });
                console.log("Final display text is verified to be 'Display is 3'");
            } else if (textResponse?.text === "3") {
                await displayElement.expectTextEquals("3", { timeout: 5000, maxDepth: 1 });
                console.log("Final display text is verified to be '3'");
            } else {
                console.log(`Unexpected text: ${textResponse?.text}`);
            }

            // Optionally get text again after verification
            const finalText = await displayElement.getText();
            console.log(`Final display text (raw): ${finalText?.text}`);

        } catch (e) {
            console.error(`Verification failed or could not get final text: ${e instanceof Error ? e.message : e}`);
            // Try getting raw text anyway on failure for debugging
            try {
                const rawText = await displayElement.getText();
                console.error(`Raw text on failure: ${rawText?.text}`);
            } catch (innerErr) {
                 console.error(`Could not even get raw text after verification failure: ${innerErr instanceof Error ? innerErr.message : innerErr}`);
            }
        }

        // Example: Get attributes of the equals button
        console.log("\n--- Example: Get Attributes of '=' button ---");
        const attrs = await buttonEquals.getAttributes();
        console.log(`Equals button attributes: ${JSON.stringify(attrs, null, 2)}`);

        // Example: Check visibility of the equals button
        console.log("\n--- Example: Check Visibility of '=' button ---");
        const visible = await buttonEquals.isVisible();
        console.log(`Is Equals button visible? ${visible}`);

        // Optional: Close the calculator
        // console.log("\n--- Optional: Closing Calculator ---");
        // try {
        //     await calculatorWindow.pressKey("%{F4}"); // Alt+F4 on Windows - Ensure key codes match server expectation
        //     console.log("Sent close command.");
        // } catch (e) {
        //      console.warn(`Could not send close command: ${e instanceof Error ? e.message : e}`);
        // }

    } catch (e) {
        if (e instanceof ApiError) {
             console.error(`\nAPI Error occurred: ${e.message} (Status: ${e.status ?? 'N/A'})`);
        } else if (e instanceof Error) {
            console.error(`\nAn unexpected error occurred: ${e.message}`);
            console.error(e.stack);
        } else {
             console.error(`\nAn unknown error occurred: ${e}`);
        }
        // Use process.exit only if we're in a Node.js environment
        if (typeof process !== 'undefined') {
            process.exit(1); // Exit with error code
        }
    }

    console.log("\n--- Example Finished --- (Press Ctrl+C if server doesn't exit)");

})(); // End of async IIFE
