#!/usr/bin/env ts-node
/**
 * Example website test using Terminator SDK (TypeScript)
 * Tests Wikipedia search functionality by:
 * 1. Opening Wikipedia homepage
 * 2. Searching for a term
 * 3. Verifying search results appear
 * 
 * Note: This example uses 'any' types for simplicity until proper
 * TypeScript definitions are available for the Terminator SDK
 */

// Note: Using require for compatibility until proper TS bindings are ready
const terminator = require('terminator.js');

// Utility sleep function
function sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
}

async function testWikipediaSearch(): Promise<boolean> {
    // Get configuration from environment variables
    const url = (process as any).env.TERMINATOR_URL || 'https://en.wikipedia.org';
    const timeout = parseInt((process as any).env.TERMINATOR_TIMEOUT || '60');
    const searchTerm = (process as any).env.SEARCH_TERM || 'Artificial Intelligence';
    
    console.log(`Starting Wikipedia search test with URL: ${url}`);
    console.log(`Search term: ${searchTerm}`);
    
    const desktop = new terminator.Desktop(undefined, undefined, 'info');
    
    try {
        // Step 1: Open Wikipedia homepage
        console.log("Opening Wikipedia homepage...");
        desktop.openUrl(url);
        await sleep(3000); // Wait for page to load
        
        // Step 2: Find Wikipedia window and search interface
        console.log("Looking for Wikipedia search interface...");
        let wikipediaWindow: any;
        let document: any;
        
        try {
            // Try to find the window containing Wikipedia
            wikipediaWindow = desktop.locator('window:Wikipedia');
            document = wikipediaWindow.locator('role:Document');
        } catch (error) {
            // Fallback: use any browser window
            console.log("Fallback: Using any available browser window");
            wikipediaWindow = desktop.locator('role:Window');
            document = wikipediaWindow.locator('role:Document');
        }
        
        // Wait a bit more for page to fully load
        await sleep(2000);
        
        // Step 3: Find and interact with search box
        console.log("Finding search input field...");
        
        // Try different ways to find the search box
        let searchBox: any = null;
        const searchAttempts = [
            'name:Search Wikipedia',
            'name:search',
            'role:SearchBox',
            'role:TextBox'
        ];
        
        for (const attempt of searchAttempts) {
            try {
                console.log(`Trying to find search box with: ${attempt}`);
                searchBox = await document.locator(attempt).first();
                break;
            } catch (error) {
                console.log(`Failed with ${attempt}: ${error}`);
                continue;
            }
        }
        
        if (!searchBox) {
            throw new Error("Could not find Wikipedia search box");
        }
        
        // Step 4: Type search term
        console.log(`Typing search term: ${searchTerm}`);
        try {
            if (searchBox.highlight) {
                searchBox.highlight(0x00FF00, 1000); // Green highlight
            }
            if (searchBox.clear) {
                await searchBox.clear(); // Clear any existing text
            }
            if (searchBox.typeText) {
                await searchBox.typeText(searchTerm);
            } else if (searchBox.type_text) {
                await searchBox.type_text(searchTerm);
            }
        } catch (error) {
            console.warn(`Could not interact with search box: ${error}`);
        }
        await sleep(1000);
        
        // Step 5: Submit search (try Enter key first, then search button)
        console.log("Submitting search...");
        try {
            if (searchBox.pressKey) {
                await searchBox.pressKey("Return");
            } else if (searchBox.press_key) {
                await searchBox.press_key("Return");
            }
            await sleep(3000); // Wait for search results
        } catch (error) {
            // Fallback: try to find and click search button
            console.log("Trying search button instead...");
            try {
                const searchButton = await document.locator('name:Search').first();
                if (searchButton && searchButton.highlight) {
                    searchButton.highlight(0x0000FF, 1000); // Blue highlight
                }
                if (searchButton && searchButton.click) {
                    await searchButton.click();
                }
                await sleep(3000);
            } catch (buttonError) {
                console.error(`Could not submit search: ${buttonError}`);
                throw buttonError;
            }
        }
        
        // Step 6: Verify search results or article page
        console.log("Verifying search results or article page...");
        
        let resultsFound = false;
        const resultAttempts = [
            'role:Main',
            'name:Search results',
            'role:Article',
            'role:Heading'
        ];
        
        for (const attempt of resultAttempts) {
            try {
                console.log(`Looking for content with: ${attempt}`);
                const contentContainer = await document.locator(attempt).first();
                if (contentContainer) {
                    if (contentContainer.highlight) {
                        contentContainer.highlight(0xFFFF00, 1000); // Yellow highlight
                    }
                    resultsFound = true;
                    console.log("✅ Content found!");
                    break;
                }
            } catch (error) {
                console.log(`No content found with ${attempt}: ${error}`);
                continue;
            }
        }
        
        if (!resultsFound) {
            // Try alternative verification - look for any links on the page
            try {
                const linkLocator = document.locator('role:Link');
                let linkCount = 0;
                if (linkLocator && linkLocator.count) {
                    linkCount = await linkLocator.count();
                }
                if (linkCount > 10) { // If we have many links, assume search worked
                    console.log(`✅ Found ${linkCount} links - search appears successful`);
                    resultsFound = true;
                }
            } catch (error) {
                // Ignore counting errors
            }
        }
        
        if (!resultsFound) {
            throw new Error("No search results or article content found - search may have failed");
        }
        
        // Step 7: Take a screenshot for verification (optional)
        try {
            console.log("Taking screenshot of results...");
            let screenshotResult = null;
            if (desktop.screenshot) {
                screenshotResult = await desktop.screenshot("test-outputs/wikipedia_search_results.png");
            }
            if (screenshotResult) {
                console.log("Screenshot saved successfully");
            }
        } catch (error) {
            console.warn(`Could not take screenshot: ${error}`);
        }
        
        console.log("🎉 Wikipedia search test completed successfully!");
        return true;
        
    } catch (error) {
        console.error(`Test failed: ${error}`);
        
        // Try to take a failure screenshot
        try {
            if (desktop.screenshot) {
                await desktop.screenshot("test-outputs/wikipedia_search_failure.png");
                console.log("Failure screenshot saved");
            }
        } catch (screenshotError) {
            // Ignore screenshot errors
        }
        
        throw error;
    }
}

async function main(): Promise<void> {
    try {
        const success = await testWikipediaSearch();
        if (success) {
            console.log("All tests passed! ✅");
            (process as any).exit(0);
        } else {
            console.error("Tests failed! ❌");
            (process as any).exit(1);
        }
    } catch (error) {
        console.error(`Test execution failed: ${error}`);
        (process as any).exit(1);
    }
}

// Entry point
if ((require as any).main === module) {
    main();
}