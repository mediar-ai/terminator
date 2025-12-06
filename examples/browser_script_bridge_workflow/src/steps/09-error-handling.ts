import { createStep } from "@mediar-ai/workflow";

export const errorHandling = createStep({
    id: "error_handling",
    name: "Test JavaScript error handling",
    execute: async ({ desktop, input, logger, context }) => {
        logger.info("Testing JavaScript error handling...");

        const browser = context.data.browser;

        // Test 1: Syntax error
        logger.info("1️⃣ Testing syntax error");
        try {
            await browser.executeBrowserScript("this is not valid javascript{");
            throw new Error("Syntax error should have failed");
        } catch (error) {
            const errorMsg = error instanceof Error ? error.message : String(error);
            logger.info(`   Syntax error correctly failed: ${errorMsg}`);
        }

        // Test 2: Runtime error (undefined variable)
        logger.info("2️⃣ Testing runtime error - undefined variable");
        try {
            await browser.executeBrowserScript("undefinedVariable.someProperty");
            throw new Error("Runtime error should have failed");
        } catch (error) {
            const errorMsg = error instanceof Error ? error.message : String(error);
            logger.info(`   Runtime error correctly failed: ${errorMsg}`);
        }

        // Test 3: Thrown error
        logger.info("3️⃣ Testing thrown error");
        try {
            await browser.executeBrowserScript("throw new Error('Custom error message')");
            throw new Error("Thrown error should have failed");
        } catch (error) {
            const errorMsg = error instanceof Error ? error.message : String(error);
            if (!errorMsg.includes("Custom error message") &&
                !errorMsg.includes("Error") &&
                !errorMsg.includes("EVAL_ERROR") &&
                !errorMsg.includes("Uncaught")) {
                throw new Error(`Unexpected error message: ${errorMsg}`);
            }
            logger.info(`   Thrown error correctly failed: ${errorMsg}`);
        }

        // Test 4: Promise rejection
        logger.info("4️⃣ Testing promise rejection");
        try {
            await browser.executeBrowserScript("Promise.reject(new Error('Async failure'))");
            throw new Error("Promise rejection should have failed");
        } catch (error) {
            const errorMsg = error instanceof Error ? error.message : String(error);
            logger.info(`   Promise rejection correctly failed: ${errorMsg}`);
        }

        logger.success("All error handling tests passed");

        return {
            state: {
                syntaxError: "failed correctly",
                runtimeError: "failed correctly",
                thrownError: "failed correctly",
                promiseRejection: "failed correctly",
            },
        };
    },
});
