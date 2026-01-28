import { createStep } from "@mediar-ai/workflow";

export const basicTypes = createStep({
    id: "basic_types",
    name: "Test basic JavaScript return types",
    execute: async ({ desktop, input, logger, context }) => {
        logger.info("Testing basic JavaScript return types...");

        const browser = context.data.browser;

        // Test 1: String return
        logger.info("1️⃣ Testing string return");
        const stringResult = await browser.executeBrowserScript("'hello world'");
        if (stringResult !== "hello world") {
            throw new Error(`Expected 'hello world', got '${stringResult}'`);
        }

        // Test 2: Number return
        logger.info("2️⃣ Testing number return");
        const numberResult = await browser.executeBrowserScript("42");
        if (numberResult !== "42") {
            throw new Error(`Expected '42', got '${numberResult}'`);
        }

        // Test 3: Boolean return
        logger.info("3️⃣ Testing boolean return");
        const booleanResult = await browser.executeBrowserScript("true");
        if (booleanResult !== "true") {
            throw new Error(`Expected 'true', got '${booleanResult}'`);
        }

        // Test 4: Object return
        logger.info("4️⃣ Testing object return");
        const objectResult = await browser.executeBrowserScript("({name: 'test', value: 123})");
        const parsedObject = JSON.parse(objectResult);
        if (parsedObject.name !== "test" || parsedObject.value !== 123) {
            throw new Error(`Object parsing failed: ${objectResult}`);
        }

        // Test 5: Array return
        logger.info("5️⃣ Testing array return");
        const arrayResult = await browser.executeBrowserScript("[1, 2, 3, 4, 5]");
        const parsedArray = JSON.parse(arrayResult);
        if (!Array.isArray(parsedArray) || parsedArray.length !== 5) {
            throw new Error(`Array parsing failed: ${arrayResult}`);
        }

        // Test 6: Expression evaluation
        logger.info("6️⃣ Testing expression evaluation");
        const expressionResult = await browser.executeBrowserScript("2 + 2 * 3");
        if (expressionResult !== "8") {
            throw new Error(`Expected '8', got '${expressionResult}'`);
        }

        logger.success("All basic type tests passed");

        return {
            state: {
                stringResult,
                numberResult,
                booleanResult,
                objectResult: parsedObject,
                arrayResult: parsedArray,
                expressionResult,
            },
        };
    },
});
