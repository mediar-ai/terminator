import { createStep } from "@mediar-ai/workflow";

export const asyncOperations = createStep({
    id: "async_operations",
    name: "Test async JavaScript operations",
    execute: async ({ desktop, input, logger, context }) => {
        logger.info("Testing async JavaScript operations...");

        const browser = context.data.browser;

        // Test 1: Promise resolution
        logger.info("1️⃣ Testing promise resolution");
        const promiseResult = await browser.executeBrowserScript("Promise.resolve('async result')");
        if (promiseResult !== "async result") {
            throw new Error(`Expected 'async result', got '${promiseResult}'`);
        }

        // Test 2: Async function with setTimeout
        logger.info("2️⃣ Testing async function with setTimeout");
        const asyncResult = await browser.executeBrowserScript(`
            (async function() {
                await new Promise(resolve => setTimeout(resolve, 500));
                return 'delayed result';
            })()
        `);
        if (asyncResult !== "delayed result") {
            throw new Error(`Expected 'delayed result', got '${asyncResult}'`);
        }

        // Test 3: Async operation with document
        logger.info("3️⃣ Testing async operation with document");
        const docResult = await browser.executeBrowserScript(`
            (async function() {
                // Wait a bit and return document info
                await new Promise(resolve => setTimeout(resolve, 100));
                return {
                    title: document.title || 'about:blank',
                    url: document.URL.substring(0, 50),
                    readyState: document.readyState
                };
            })()
        `);
        const parsedDoc = JSON.parse(docResult);
        if (typeof parsedDoc.title !== "string") {
            throw new Error(`Document title should be a string: ${docResult}`);
        }
        if (!parsedDoc.url || !parsedDoc.readyState) {
            throw new Error(`Document info incomplete: ${docResult}`);
        }
        logger.info(`   Result: ${docResult}`);

        logger.success("All async operation tests passed");

        return {
            state: {
                promiseResult,
                asyncResult,
                docResult: parsedDoc,
            },
        };
    },
});
