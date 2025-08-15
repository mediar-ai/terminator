// examples/pdf-to-form/src/index.ts
import * as dotenv from 'dotenv';
import * as fs from 'fs/promises'; // Use promises for async file operations
import * as path from 'path';
import inquirer from 'inquirer';
// Vercel AI SDK imports
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { tool, streamText, CoreMessage } from 'ai';
import { z } from 'zod';

// Terminator/DesktopUseClient import
import { Desktop } from '../../../bindings/nodejs'; // Assuming package name


// Suppress Node.js warnings (use with caution)
process.removeAllListeners('warning');

dotenv.config(); // Load .env file

const ENV_PATH = path.resolve(__dirname, '../.env'); // Path to .env in parent dir

// --- Constants ---
// Use path.resolve to get the absolute path to the PDF file
// Go one level up from __dirname (src) to find data.pdf in the parent directory
const PDF_FILE_PATH = path.resolve(__dirname, '../data.pdf');
const WEB_APP_URL = 'https://v0-pharmaceutical-form-design-5aeik3.vercel.app/';
const EDGE_PATH = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe"; // Escaped backslashes for JS string

// IMPORTANT: Selectors likely need adjustment based on actual application behavior


async function getApiKey(): Promise<string> {
    let apiKey = process.env.GEMINI_API_KEY;

    if (!apiKey) {
        console.log("Gemini API Key not found in environment variables (.env).");
        const answers = await inquirer.prompt([
            {
                type: 'password', // Use password type to mask input
                name: 'apiKey',
                message: 'Please enter your Gemini API Key:',
                mask: '*',
                validate: (input: string) => !!input || 'API Key cannot be empty.',
            },
        ]);
        apiKey = answers.apiKey;

        // Save the API key to .env
        try {
            await fs.appendFile(ENV_PATH, `
GEMINI_API_KEY=${apiKey}`);
            console.log("API Key saved to .env file.");
            dotenv.config({ path: ENV_PATH, override: true });
        } catch (err) {
            console.error("Error saving API key to .env:", err);
            console.warn("Proceeding without saving API key.");
        }
    }

    if (!apiKey) { // Double check
         throw new Error("Failed to get Gemini API Key.");
    }

    return apiKey;
}

async function main() {
    console.log(`
✨ Welcome to the AI PDF-to-Form Automator! ✨`);

    const apiKey = await getApiKey();
    console.log("🔑 Gemini API Key loaded.");

    const google = createGoogleGenerativeAI({
        apiKey: apiKey,
    });
    const model = google('models/gemini-2.0-flash');
    console.log(`🤖 Initialized Gemini Model: ${model.modelId}`);

    let desktopClient: Desktop | null = null;
    try {
        desktopClient = new Desktop(); // Assumes server running on default localhost:9375
        console.log("🖥️ Connected to Terminator server.");

        // Automated setup is now commented out. Manual setup prompt follows.
        // await _runAutomatedSetup(desktopClient!);

    } catch (error) {
        // Adjust error message if only connection can fail now
        console.error("❌ Failed to connect to Terminator server.");
        // console.error("   Ensure Terminator server is running and check PowerShell script output/errors.");
        if (error instanceof Error) {
             console.error(`   Details: ${error.message}`);
        } else {
             console.error(error);
        }

        process.exit(1);
    }

    if (!desktopClient) {
        console.error("❌ Desktop client initialization failed unexpectedly.");
        process.exit(1);
    }

    // --- Manual Setup Confirmation ---
    console.log(`\n--- Manual Setup Required ---`);
    console.log(`Please ensure the Terminator server is running.`);
    console.log(`Run the following commands in PowerShell (adjust paths if needed):`);
    // Escape paths for PowerShell command line arguments
    // Using single quotes for the outer PowerShell string avoids most inner escaping issues.
    const pdfCmd = `Start-Process -FilePath '${EDGE_PATH}' -ArgumentList '--new-window \"${PDF_FILE_PATH}\"'`;
    const appCmd = `Start-Process -FilePath '${EDGE_PATH}' -ArgumentList '--app=${WEB_APP_URL}'`;
    console.log(`\n# 1. Open PDF in Edge:\n${pdfCmd}\n`);
    console.log(`# 2. Open Web App in Edge:\n${appCmd}\n`);
    console.log(`Then, arrange the PDF window on the LEFT and the Web App window on the RIGHT.`);

    const { ready } = await inquirer.prompt([
        {
            type: 'confirm',
            name: 'ready',
            message: 'Are the PDF (left) and Form App (right) windows open side-by-side and ready to proceed?',
            default: true,
        },
    ]);

    if (!ready) {
        console.log("Setup not confirmed. Exiting.");
        process.exit(0);
    }

    console.log("✅ Setup confirmed by user. Sleeping 2 seconds...");

    await new Promise(resolve => setTimeout(resolve, 2000));

    console.log(`
🧠 AI starting PDF-to-Form process...`);

    // --- Define Tools for AI ---
    const tools = {
        // Tool to read text content from a specific UI element
        readElementText: tool({
            description: `Reads the text content from a UI element specified by a selector string.
                        Use this to get text from the PDF viewer.`,
            parameters: z.object({
                selector: z.string().describe(`The selector string for the UI element to read text from`)
            }),
            execute: async ({ selector }) => {
                try {
                    console.log(`
🔧 [Tool Call] Reading text from element: "${selector}"`);
                    const result = (await desktopClient!.locator(selector).first()).text();
                    console.log(`
✅ [Tool Result] Got text snippet: "${result.substring(0, 100)}..."`);
                    return { success: true, text: result };
                } catch (error: any) {
                    console.error(`
❌ [Tool Error] Failed to get text from element "${selector}": ${error.message}`);
                    return { success: false, error: error.message };
                }
            }
        }),
        // Tool to type text into a specific UI element (like a form field)
        typeIntoElement: tool({
            description: `Types the given text into the UI element (usually an input field or text area) identified by the selector string.
                        Ensure the element is visible and enabled before typing. Use 'findElement' or 'listMatchingElements' first if unsure.`,
            parameters: z.object({
                inputName: z.string().describe("The name of the UI element to type into (eg. 'Patient Full Name', etc. read the text of the form to find the names of the inputs)"),
                textToType: z.string().describe("The text to type into the element")
            }),
            execute: async ({ inputName, textToType }) => {
                 try {
                    console.log(`
🔧 [Tool Call] Typing "${textToType}" into element: "${inputName}"`);
                    const element = desktopClient!.locator(`name:${inputName}`);
                    // Consider adding .focus() before typing if needed, though typeText often handles it
                    // await element.focus(); // Optional: Explicitly focus
                    const result = await element.typeText(textToType);
                    console.log(`
✅ [Tool Result] Typed text into "${inputName}".`);
                    return { success: true, details: result };
                } catch (error: any) {
                    console.error(`
❌ [Tool Error] Failed to type into element "${inputName}": ${error.message}`);
                    return { success: false, error: error.message };
                }
            }
        }),
        // --- NEW TOOL: findWindow ---
        findWindow: tool({
            description: `Finds a top-level application window based on criteria like title or process name.
                        Use this as the *first step* to identify the specific window you want to interact with (e.g., the PDF viewer or the Web Form).
                        Returns details of the found window element. Subsequent actions should use this window's details or a locator derived from it.`,
            parameters: z.object({
                titleContains: z.string().optional().describe("A substring of the window title to search for (case-insensitive)."),
            }),
            execute: async ({ titleContains }) => {
                 if (!titleContains) {
                     return { success: false, error: "findWindow requires 'titleContains'." };
                 }
                try {
                    console.log(`
🔧 [Tool Call] Finding window: titleContains="${titleContains}"`);
                    // Use the new client method. Note: It returns a Locator directly.
                    // For the tool, we might want the element details, so we call .first() on the returned locator.
                    const windowElement = await desktopClient!.findWindowByCriteria(titleContains);
                    console.log(`
✅ [Tool Result] Found window: Role=${windowElement.role}, Name=${windowElement.name()}, ID=${windowElement.id()}`);
                    // Return the element details which includes role, label, id
                    return { success: true, windowElement: windowElement };
                } catch (error: any) {
                    console.error(`
❌ [Tool Error] Failed to find window (titleContains="${titleContains}"): ${error.message}`);
                    return { success: false, error: `Failed findWindow (titleContains="${titleContains}"): ${error.message}` };
                }
            }
        }),
         // Placeholder tool for finishing the process
        finishTask: tool({
            description: "Call this tool ONLY when you have successfully read the PDF, identified all relevant fields in the form, and filled them completely according to the PDF data. This indicates the automation task is complete.",
            parameters: z.object({
                summary: z.string().describe("A brief summary of the data transferred and the completion status."),
            }),
            execute: async ({ summary }) => {
                 console.log(`
🏁 [Tool Call] Finishing Task: ${summary}`);
                console.log(`
🎉 Automation task marked as complete by AI.`);
                // We can potentially exit the process here or signal completion
                // process.exit(0);
                return { success: true, message: "Task finished successfully.", summary: summary };
            },
        }),
    };

    // --- Construct Prompt for AI ---
    const systemPrompt = `You are an AI assistant specialized in automating data entry from a PDF document into a web application form using the 'desktop-use' SDK via provided tools.

    **Setup:**
    The user has manually opened the required PDF ('${PDF_FILE_PATH}') and the web application form ('${WEB_APP_URL}'), arranged them side-by-side (PDF left, form right), and confirmed readiness. Assume this is complete.

    **Your Goal - Follow This Order Strictly:**
    1.  **Identify Windows:** Use **'findWindow'** twice: first with \`titleContains:"data.pdf"\` for the PDF viewer, then with \`titleContains:"v0 App"\` (or similar relevant title part) for the Web Form. **Mentally note the unique ID or selector returned for EACH window.**
    2.  **Read PDF Text:** Use the **'readElementText'** tool. **CRITICAL:** Target the PDF content area *specifically within the PDF window* found in step 1. Use a selector like \`#PDF_WINDOW_ID role:document\` (replace \`#PDF_WINDOW_ID\` with the actual ID) or another specific selector found within that window. **Do NOT use a generic \`role:document\` selector without specifying the PDF window.**
    3.  **Fill Form:** Use the **'typeIntoElement'** tool for each text field. **CRITICAL:** Use the **precise selectors** for each input field from names you read from the form window text. ALSO MAKE SURE TO USE THE CORRECT DATA FROM THE PDF, NO FUCKING HALLUCINATIONS.
    4.  **Complete Task:** Once all relevant data is accurately transferred, call 'finishTask' with a summary.

    **Tool Usage Guidelines:**
    - **Targeting is Key:** Most errors happen when tools are not targeted at the correct window or element. Use the specific IDs/selectors obtained from 'findWindow' and 'listInputs' when calling subsequent tools like 'readElementText', 'listInputs', and 'typeIntoElement'. Chain selectors like \`#WINDOW_ID specific_child_selector\`.
    - **Selectors:** Prefer specific names (\`name:"Label"\`), roles (\`role:edit\`, \`role:button\`), or IDs (\`#elementId\`) found via tools. Avoid vague selectors. Remember these are desktop UI selectors (UIA/ATK), not web selectors.
    - **Common Roles:** \`window\`, \`button\`, \`checkbox\`, \`menu\`, \`menuitem\`, \`dialog\`, \`text\`, \`edit\` (for text inputs), \`document\`, \`pane\`, \`list\`, \`listitem\`, \`combobox\`, \`radiobutton\`, \`tab\`, \`tabitem\`, \`toolbar\`, \`image\`, \`link\`.
    - **Error Handling:** If a selector fails, re-evaluate: Did you target the correct window ID? Is the selector specific enough? Was it found using 'listInputs' or exploration within the correct parent?

    **Start now by using 'findWindow' twice as described in Step 1.**`;

    const initialUserMessage = "The PDF and Form windows are open side-by-side. Please start the process following the system prompt exactly: find both windows, read the PDF content *from the PDF window*, list inputs *from the form window*, then fill the form inputs using specific selectors.";

    const messages: CoreMessage[] = [
        { role: 'system', content: systemPrompt },
        { role: 'user', content: initialUserMessage }
    ];

    // --- Generate and Stream AI Actions ---
    try {
        const { textStream, toolResults } = streamText({
            model: model,
            tools: tools,
            messages: messages,
            toolChoice: 'required',
             maxSteps: 30,
             onError: (error) => {
                 console.error(`
❌ Error during AI processing: ${error}`);
             },
        });

        // Stream the AI's thinking process (text parts) to the console
        let fullResponse = "";
        process.stdout.write(`
AI Thinking:
---`);
        for await (const textPart of textStream) {
            process.stdout.write(textPart);
            fullResponse += textPart;
        }
         process.stdout.write(`
---
`);


        // Wait for all tool calls to complete and process results
        // const finalToolResults = await toolResults;
        // console.log(`
// 🛠️ Final Tool Results:", finalToolResults); // Optional: log all results at the end

        console.log(`

✅ AI interaction complete.`);


    } catch (error) {
        console.error(`
❌ An error occurred during the main AI interaction loop:`);
        console.error(error);
    } finally {
         // Decide whether to close the client or keep it open
         // For a single-run script, closing might make sense, but could be omitted.
         // await desktopClient?.close(); // Assuming a close method exists
         console.log(`
👋 AI Automator session finished.`);
    }
}

main().catch(error => {
     console.error("Unhandled error in main:", error);
     process.exit(1);
}); 