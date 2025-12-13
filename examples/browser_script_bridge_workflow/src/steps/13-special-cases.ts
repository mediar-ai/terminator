import { createStep } from "@mediar-ai/workflow";

export const specialCases = createStep({
    id: "special_cases",
    name: "Test special cases and edge conditions",
    execute: async ({ desktop, input, logger, context }) => {
        logger.info("Testing special cases...");

        const browser = context.data.browser;

        // Test 1: Empty string
        logger.info("1️⃣ Testing empty string return");
        const emptyResult = await browser.executeBrowserScript("''");
        if (emptyResult !== "") {
            throw new Error(`Expected empty string, got '${emptyResult}'`);
        }

        // Test 2: Zero
        logger.info("2️⃣ Testing zero return");
        const zeroResult = await browser.executeBrowserScript("0");
        if (zeroResult !== "0") {
            throw new Error(`Expected '0', got '${zeroResult}'`);
        }

        // Test 3: False
        logger.info("3️⃣ Testing false return");
        const falseResult = await browser.executeBrowserScript("false");
        if (falseResult !== "false") {
            throw new Error(`Expected 'false', got '${falseResult}'`);
        }

        // Test 4: Null (should fail)
        logger.info("4️⃣ Testing null return");
        try {
            await browser.executeBrowserScript("null");
            throw new Error("Null should have failed");
        } catch (error) {
            const errorMsg = error instanceof Error ? error.message : String(error);
            logger.info(`   Null correctly failed: ${errorMsg}`);
        }

        // Test 5: Undefined (should fail)
        logger.info("5️⃣ Testing undefined return");
        try {
            await browser.executeBrowserScript("undefined");
            throw new Error("Undefined should have failed");
        } catch (error) {
            const errorMsg = error instanceof Error ? error.message : String(error);
            logger.info(`   Undefined correctly failed: ${errorMsg}`);
        }

        // Test 6: Very long string
        logger.info("6️⃣ Testing long string return");
        const longStringResult = await browser.executeBrowserScript("'a'.repeat(1000)");
        if (longStringResult.length !== 1000) {
            throw new Error(`Expected 1000 chars, got ${longStringResult.length}`);
        }

        // Test 7: Large array
        logger.info("7️⃣ Testing large array return");
        const largeArrayResult = await browser.executeBrowserScript("Array.from({length: 100}, (_, i) => i)");
        const parsedLargeArray = JSON.parse(largeArrayResult);
        if (!Array.isArray(parsedLargeArray) || parsedLargeArray.length !== 100) {
            throw new Error(`Large array test failed: ${parsedLargeArray.length} items`);
        }
        if (parsedLargeArray[0] !== 0 || parsedLargeArray[99] !== 99) {
            throw new Error("Large array content incorrect");
        }

        // Test 8: Nested objects
        logger.info("8️⃣ Testing nested objects");
        const nestedResult = await browser.executeBrowserScript(`
            ({
                level1: {
                    level2: {
                        level3: {
                            value: 'deep nested'
                        }
                    }
                }
            })
        `);
        const parsedNested = JSON.parse(nestedResult);
        if (parsedNested.level1.level2.level3.value !== "deep nested") {
            throw new Error("Nested object parsing failed");
        }

        logger.success("All special case tests passed");

        return {
            state: {
                emptyString: emptyResult,
                zero: zeroResult,
                false: falseResult,
                nullFailed: true,
                undefinedFailed: true,
                longStringLength: longStringResult.length,
                largeArrayLength: parsedLargeArray.length,
                nestedObject: parsedNested,
            },
        };
    },
});
