#!/usr/bin/env node
/**
 * Cursor Night Shift Agent - Automated Prompt Sender (TypeScript)
 * 
 * This example demonstrates how to use Terminator to automate sending prompts to Cursor IDE
 * at regular intervals. Perfect for running tasks while you're away from the keyboard!
 * 
 * Features:
 * - Automatically finds and focuses Cursor window
 * - Sends prompts from a configurable list
 * - Customizable intervals between prompts
 * - Graceful error handling and recovery
 * - Supports both chat and command modes
 * - Highlights UI elements for visual feedback
 * 
 * Usage:
 *     npx tsx cursor_night_shift_agent.ts
 *     # or
 *     node cursor_night_shift_agent.js
 * 
 * Configuration:
 *     Modify the PROMPTS array and INTERVAL_SECONDS to customize behavior.
 */

import { Desktop } from '../bindings/nodejs';

// Configure console logging with timestamps
const log = {
    info: (msg: string) => console.log(`${new Date().toISOString()} - INFO - ${msg}`),
    warn: (msg: string) => console.warn(`${new Date().toISOString()} - WARN - ${msg}`),
    error: (msg: string) => console.error(`${new Date().toISOString()} - ERROR - ${msg}`),
    debug: (msg: string) => console.debug(`${new Date().toISOString()} - DEBUG - ${msg}`)
};

// Utility sleep function
function sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Automated agent for sending prompts to Cursor IDE at intervals.
 */
class CursorNightShiftAgent {
    private prompts: string[];
    private intervalSeconds: number;
    private maxRetries: number;
    private desktop: Desktop;
    private cursorApp: any = null;
    private currentPromptIndex: number = 0;

    constructor(prompts: string[], intervalSeconds: number = 300, maxRetries: number = 3) {
        this.prompts = prompts;
        this.intervalSeconds = intervalSeconds;
        this.maxRetries = maxRetries;
        this.desktop = new Desktop(undefined, undefined, 'error');
    }

    /**
     * Find and focus the Cursor application window.
     */
    async findCursorWindow(): Promise<boolean> {
        try {
            log.info("Looking for Cursor application...");

            // Try different ways to find Cursor
            const cursorSelectors = [
                "name:Cursor",
                "name:cursor", 
                "window:Cursor",
                "window:cursor"
            ];

            for (const selector of cursorSelectors) {
                try {
                    const cursorWindow = await this.desktop.locator(selector).first();
                    if (cursorWindow) {
                        log.info(`Found Cursor window using selector: ${selector}`);
                        await cursorWindow.highlight(0x00FF00, 2000); // Green highlight
                        await cursorWindow.focus();
                        this.cursorApp = cursorWindow;
                        await sleep(1000); // Allow window to focus
                        return true;
                    }
                } catch (e) {
                    log.debug(`Selector ${selector} failed: ${e instanceof Error ? e.message : e}`);
                    continue;
                }
            }

            // If window not found, try to open Cursor
            log.info("Cursor window not found, attempting to launch...");
            try {
                this.cursorApp = this.desktop.openApplication("cursor.exe");
                await sleep(5000); // Allow app to fully load
                log.info("Cursor launched successfully");
                return true;
            } catch (e) {
                log.error(`Failed to launch Cursor: ${e instanceof Error ? e.message : e}`);
                return false;
            }

        } catch (e) {
            log.error(`Error finding Cursor window: ${e instanceof Error ? e.message : e}`);
            return false;
        }
    }

    /**
     * Find the chat input area in Cursor.
     */
    async findChatInput(): Promise<any> {
        try {
            // Common selectors for chat input in Cursor
            const chatSelectors = [
                "role:textbox",
                "role:EditableText",
                "role:Edit", 
                "name:*chat*",
                "name:*input*",
                "name:*message*",
                "placeholder:*message*",
                "placeholder:*chat*"
            ];

            for (const selector of chatSelectors) {
                try {
                    const chatInput = await this.cursorApp.locator(selector).first();
                    if (chatInput && await chatInput.isVisible()) {
                        log.info(`Found chat input using selector: ${selector}`);
                        await chatInput.highlight(0x0000FF, 1500); // Blue highlight
                        return chatInput;
                    }
                } catch (e) {
                    log.debug(`Chat selector ${selector} failed: ${e instanceof Error ? e.message : e}`);
                    continue;
                }
            }

            log.warn("Could not find chat input, will try keyboard shortcuts");
            return null;

        } catch (e) {
            log.error(`Error finding chat input: ${e instanceof Error ? e.message : e}`);
            return null;
        }
    }

    /**
     * Try to open chat using keyboard shortcuts.
     */
    async openChatWithShortcut(): Promise<boolean> {
        try {
            log.info("Attempting to open chat with keyboard shortcuts...");

            // Common shortcuts to open chat in Cursor
            const shortcuts = [
                "{Ctrl}l",       // Ctrl+L (common for chat)
                "{Ctrl}k",       // Ctrl+K (command palette)
                "{Ctrl}{Shift}p", // Command palette
                "{F1}"           // Help/Command palette
            ];

            for (const shortcut of shortcuts) {
                log.info(`Trying shortcut: ${shortcut}`);
                await this.cursorApp.pressKey(shortcut);
                await sleep(2000);

                // Check if chat input appeared
                const chatInput = await this.findChatInput();
                if (chatInput) {
                    log.info(`Chat opened successfully with shortcut: ${shortcut}`);
                    return true;
                }
            }

            log.warn("Could not open chat with shortcuts");
            return false;

        } catch (e) {
            log.error(`Error opening chat with shortcuts: ${e instanceof Error ? e.message : e}`);
            return false;
        }
    }

    /**
     * Send a prompt to Cursor.
     */
    async sendPrompt(prompt: string): Promise<boolean> {
        try {
            log.info(`Sending prompt: ${prompt.substring(0, 50)}...`);

            // Find chat input
            let chatInput = await this.findChatInput();

            // If not found, try to open chat
            if (!chatInput) {
                if (!(await this.openChatWithShortcut())) {
                    log.error("Could not find or open chat input");
                    return false;
                }
                chatInput = await this.findChatInput();
            }

            if (chatInput) {
                // Clear existing text and type the prompt
                await chatInput.focus();
                await sleep(500);

                // Clear any existing text
                await chatInput.pressKey("{Ctrl}a");
                await sleep(200);

                // Type the prompt
                await chatInput.typeText(prompt);
                await sleep(1000);

                // Send the prompt (try different methods)
                const sendMethods = [
                    "{Enter}",
                    "{Ctrl}{Enter}",
                    "{Shift}{Enter}"
                ];

                for (const method of sendMethods) {
                    try {
                        log.info(`Attempting to send with: ${method}`);
                        await chatInput.pressKey(method);
                        await sleep(2000);

                        // Basic success check - if we can still find input, assume it worked
                        const testInput = await this.findChatInput();
                        if (testInput) {
                            log.info("Prompt sent successfully!");
                            return true;
                        }
                    } catch (e) {
                        log.debug(`Send method ${method} failed: ${e instanceof Error ? e.message : e}`);
                        continue;
                    }
                }

                log.warn("All send methods failed");
                return false;
            } else {
                // Fallback: try typing directly to focused window
                log.info("Fallback: typing directly to focused window");
                await this.cursorApp.focus();
                await sleep(1000);
                await this.cursorApp.typeText(prompt);
                await sleep(1000);
                await this.cursorApp.pressKey("{Enter}");
                log.info("Fallback method completed");
                return true;
            }

        } catch (e) {
            log.error(`Error sending prompt: ${e instanceof Error ? e.message : e}`);
            return false;
        }
    }

    /**
     * Run a single cycle of the night shift agent.
     */
    async runSingleCycle(): Promise<boolean> {
        try {
            // Get the current prompt
            if (this.currentPromptIndex >= this.prompts.length) {
                this.currentPromptIndex = 0; // Loop back to start
            }

            const prompt = this.prompts[this.currentPromptIndex];

            log.info(`=== Cycle ${this.currentPromptIndex + 1}/${this.prompts.length} ===`);

            // Ensure Cursor is focused
            if (!(await this.findCursorWindow())) {
                log.error("Could not find or focus Cursor window");
                return false;
            }

            // Send the prompt
            const success = await this.sendPrompt(prompt);

            if (success) {
                log.info(`Successfully sent prompt ${this.currentPromptIndex + 1}`);
                this.currentPromptIndex++;
            } else {
                log.warn(`Failed to send prompt ${this.currentPromptIndex + 1}`);
            }

            return success;

        } catch (e) {
            log.error(`Error in single cycle: ${e instanceof Error ? e.message : e}`);
            return false;
        }
    }

    /**
     * Run the night shift agent continuously.
     */
    async run(maxCycles?: number): Promise<void> {
        log.info("🌙 Starting Cursor Night Shift Agent...");
        log.info(`📝 ${this.prompts.length} prompts configured`);
        log.info(`⏰ ${this.intervalSeconds} seconds between prompts`);
        log.info("🔄 Press Ctrl+C to stop");

        let cycleCount = 0;
        let consecutiveFailures = 0;

        try {
            while (maxCycles === undefined || cycleCount < maxCycles) {
                cycleCount++;

                // Run a cycle
                const success = await this.runSingleCycle();

                if (success) {
                    consecutiveFailures = 0;
                    log.info(`✅ Cycle ${cycleCount} completed successfully`);
                } else {
                    consecutiveFailures++;
                    log.warn(`❌ Cycle ${cycleCount} failed (consecutive failures: ${consecutiveFailures})`);

                    // If too many consecutive failures, longer wait
                    if (consecutiveFailures >= 3) {
                        log.warn("Multiple failures detected, waiting longer before retry...");
                        await sleep(this.intervalSeconds * 2 * 1000);
                        consecutiveFailures = 0; // Reset after long wait
                        continue;
                    }
                }

                // Wait for the next cycle
                if (maxCycles === undefined || cycleCount < maxCycles) {
                    log.info(`😴 Sleeping for ${this.intervalSeconds} seconds until next prompt...`);
                    await sleep(this.intervalSeconds * 1000);
                }
            }
        } catch (e) {
            if (e instanceof Error && e.message.includes('interrupted')) {
                log.info("🛑 Night shift agent stopped by user");
            } else {
                log.error(`💥 Unexpected error: ${e instanceof Error ? e.message : e}`);
            }
        } finally {
            log.info("🌅 Night shift agent finished");
        }
    }
}

// Configuration
const PROMPTS = [
    "Review the codebase and suggest any potential improvements to error handling",
    "Check for any unused imports or variables in the current project",
    "Look for opportunities to add helpful comments or documentation", 
    "Analyze the code for potential performance optimizations",
    "Check if there are any security considerations that should be addressed",
    "Suggest any refactoring opportunities to improve code maintainability",
    "Review the project structure and suggest any organizational improvements",
    "Check for consistent coding style and naming conventions",
    "Look for any TODO or FIXME comments that could be addressed",
    "Suggest any additional tests that might be valuable"
];

// Time between prompts (in seconds)
const INTERVAL_SECONDS = 300; // 5 minutes

async function main(): Promise<void> {
    try {
        // Create and run the agent
        const agent = new CursorNightShiftAgent(
            PROMPTS,
            INTERVAL_SECONDS,
            3 // max_retries
        );

        // Run indefinitely (or set maxCycles for testing)
        await agent.run();

    } catch (e) {
        log.error(`Failed to start night shift agent: ${e instanceof Error ? e.message : e}`);
    }
}

// Entry point - simplified to avoid TypeScript issues
console.log(`
🌙 Cursor Night Shift Agent (TypeScript)
=========================================

This agent will automatically send prompts to Cursor at regular intervals.
Make sure Cursor is running before starting!

Default configuration:
- 10 different code review prompts
- 5 minute intervals between prompts
- Automatic error recovery

Press Ctrl+C to stop at any time.
`);

main().catch(error => {
    console.error('Fatal error:', error);
});