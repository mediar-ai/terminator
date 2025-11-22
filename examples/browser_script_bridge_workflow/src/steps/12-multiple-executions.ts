import { createStep } from "@mediar-ai/workflow";

export const multipleExecutions = createStep({
    id: "multiple_executions",
    name: "Test multiple script executions",
    execute: async ({ desktop, input, logger, context }) => {
        logger.info("Testing multiple script executions...");

        const browser = context.data.browser;

        // Test: Execute 10 scripts sequentially
        logger.info("1️⃣ Executing 10 scripts sequentially");
        const executionResults = [];
        for (let i = 1; i <= 10; i++) {
            const script = `'execution ${i}'`;
            const result = await browser.executeBrowserScript(script);
            if (result !== `execution ${i}`) {
                throw new Error(`Execution ${i} failed: expected 'execution ${i}', got '${result}'`);
            }
            executionResults.push(result);
            if (i % 3 === 0) {
                logger.info(`   ✓ Completed ${i} executions`);
            }
        }

        // Test: Execute scripts with shared state
        logger.info("2️⃣ Testing shared state across executions");

        // Set a value
        await browser.executeBrowserScript("window.testCounter = 0");

        // Increment it multiple times
        const counterResults = [];
        for (let i = 1; i <= 5; i++) {
            const result = await browser.executeBrowserScript("window.testCounter++; window.testCounter");
            const expected = i.toString();
            if (result !== expected) {
                throw new Error(`Counter increment failed: expected '${expected}', got '${result}'`);
            }
            counterResults.push(parseInt(result));
        }

        logger.success("All multiple execution tests passed");

        return {
            state: {
                sequentialExecutions: executionResults.length,
                counterResults,
                finalCounter: counterResults[counterResults.length - 1],
            },
        };
    },
});
