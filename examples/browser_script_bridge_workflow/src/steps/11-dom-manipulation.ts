import { createStep } from "@mediar-ai/workflow";

export const domManipulation = createStep({
    id: "dom_manipulation",
    name: "Test DOM manipulation",
    execute: async ({ desktop, input, logger, context }) => {
        logger.info("Testing DOM manipulation...");

        const browser = context.data.browser;

        // Test 1: Create element
        logger.info("1️⃣ Testing element creation");
        const createResult = await browser.executeBrowserScript(`
            const div = document.createElement('div');
            div.id = 'test-element';
            div.textContent = 'Test Content';
            document.body.appendChild(div);
            'Element created'
        `);
        if (createResult !== "Element created") {
            throw new Error(`Element creation failed: ${createResult}`);
        }

        // Test 2: Query and verify element
        logger.info("2️⃣ Testing element query");
        const queryResult = await browser.executeBrowserScript(`
            const el = document.getElementById('test-element');
            if (!el) throw new Error('Element not found');
            el.textContent
        `);
        if (queryResult !== "Test Content") {
            throw new Error(`Element query failed: ${queryResult}`);
        }

        // Test 3: Modify element
        logger.info("3️⃣ Testing element modification");
        const modifyResult = await browser.executeBrowserScript(`
            const el = document.getElementById('test-element');
            el.textContent = 'Modified Content';
            el.setAttribute('data-test', 'value');
            el.getAttribute('data-test')
        `);
        if (modifyResult !== "value") {
            throw new Error(`Element modification failed: ${modifyResult}`);
        }

        // Test 4: Remove element
        logger.info("4️⃣ Testing element removal");
        const removeResult = await browser.executeBrowserScript(`
            const el = document.getElementById('test-element');
            el.remove();
            document.getElementById('test-element') === null
        `);
        if (removeResult !== "true") {
            throw new Error(`Element removal failed: ${removeResult}`);
        }

        logger.success("All DOM manipulation tests passed");

        return {
            state: {
                elementCreated: true,
                elementQueried: true,
                elementModified: true,
                elementRemoved: true,
            },
        };
    },
});
