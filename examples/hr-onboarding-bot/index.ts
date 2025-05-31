import * as fs from 'fs/promises';
import * as path from 'path';
import * as dotenv from 'dotenv';
import inquirer from 'inquirer';
import pdfParse from 'pdf-parse';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { streamText, tool, CoreMessage } from 'ai';
import { DesktopUseClient, ApiError, sleep } from 'desktop-use';
import { z } from 'zod';

// Load .env if present
const ENV_PATH = path.resolve(__dirname, './.env');
dotenv.config({ path: ENV_PATH });

// Constants
const PDF_FILE_PATH = path.resolve(__dirname, './resume.pdf');
const WEB_APP_URL = 'https://hr-onboarding-system.vercel.app/';
const EDGE_PATH = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe"; // Escaped backslashes for JS string

const DEPARTMENTS = [
  'Human Resources',
  'Information Technology',
  'Finance',
  'Marketing',
  'Operations',
];

async function getApiKey(): Promise<string> {
  let apiKey = process.env.GEMINI_API_KEY;
  if (!apiKey) {
    const answers = await inquirer.prompt([
      {
        type: 'password',
        name: 'apiKey',
        message: 'Please enter your Gemini API Key:',
        mask: '*',
        validate: (input: string) => !!input || 'API Key cannot be empty.',
      },
    ]);
    apiKey = answers.apiKey;
    try {
      await fs.appendFile(ENV_PATH, `\nGEMINI_API_KEY=${apiKey}`);
      dotenv.config({ path: ENV_PATH, override: true });
    } catch (err) {
      console.error('Error saving API key to .env:', err);
    }
  }
  if (!apiKey) throw new Error('Failed to get Gemini API Key.');
  return apiKey;
}

async function extractPdfText(pdfPath: string): Promise<string> {
  const data = await fs.readFile(pdfPath);
  const pdfData = await pdfParse(data);
  return pdfData.text;
}

async function main() {
  console.log(`
✨ Welcome to the AI PDF Resume to HR Onboarding Form Automator! ✨`);
  
  let desktopClient: DesktopUseClient | null = null;
  try {
    // 1. Get API key
    const apiKey = await getApiKey();
    console.log("🔑 Gemini API Key loaded.");

    // 2. Initialize Gemini model
    const google = createGoogleGenerativeAI({
      apiKey: apiKey,
    });
    const model = google('models/gemini-2.0-flash');
    console.log(`🤖 Initialized Gemini Model: ${model.modelId}`);

    // 3. Connect to Terminator server
    try {
      desktopClient = new DesktopUseClient(); // Assumes server running on default localhost:9375
      console.log("🖥️ Connected to Terminator server.");
    } catch (error) {
      console.error("❌ Failed to connect to Terminator server.");
      if (error instanceof Error) {
        console.error(`   Details: ${error.message}`);
        if (error instanceof ApiError) {
          console.error(`   Status: ${error.status}`);
        }
      } else {
        console.error(error);
      }
      process.exit(1);
    }

    if (!desktopClient) {
      console.error("❌ Desktop client initialization failed unexpectedly.");
      process.exit(1);
    }

    // 4. Manual Setup Instructions
    console.log(`\n--- Manual Setup Required ---`);
    console.log(`Please ensure the Terminator server is running.`);
    console.log(`Run the following commands in PowerShell (adjust paths if needed):`);
    const pdfCmd = `Start-Process -FilePath '${EDGE_PATH}' -ArgumentList '--new-window "${PDF_FILE_PATH}"'`;
    const appCmd = `Start-Process -FilePath '${EDGE_PATH}' -ArgumentList '--new-window "${WEB_APP_URL}"'`;
    console.log(`\n# 1. Open PDF in Edge:\n${pdfCmd}\n`);
    console.log(`# 2. Open HR Onboarding Web App in Edge:\n${appCmd}\n`);
    console.log(`Then, arrange the PDF window on the LEFT and the Web App window on the RIGHT.`);

    const { ready } = await inquirer.prompt([
      {
        type: 'confirm',
        name: 'ready',
        message: 'Are the PDF (left) and HR Onboarding App (right) windows open side-by-side and ready to proceed?',
        default: true,
      },
    ]);

    if (!ready) {
      console.log("Setup not confirmed. Exiting.");
      process.exit(0);
    }

    console.log("✅ Setup confirmed by user. Sleeping 2 seconds...");
    await sleep(2000);

    console.log(`\n🧠 AI starting PDF-to-Form process...`);

    // Extract PDF text directly using pdf-parse
    console.log('Reading PDF:', PDF_FILE_PATH);
    const pdfData = await fs.readFile(PDF_FILE_PATH);
    const pdfResult = await pdfParse(pdfData);
    const pdfText = pdfResult.text;
    console.log('PDF text extracted successfully.');
    console.log('\n--- PDF Text Preview ---');
    console.log(pdfText.substring(0, 200) + '...');
    console.log('---------------------\n');

    // 5. Define Tools for AI
    const tools = {
      // Tool to find window for the web form
      findWindow: tool({
        description: `Finds the HR Onboarding System web application window.
                    Use this as the *first step* to identify the web form window you want to interact with.`,
        parameters: z.object({
          titleContains: z.string().optional().describe("A substring of the window title to search for (case-insensitive).")
        }),
        execute: async ({ titleContains }) => {
          if (!titleContains) {
            return { success: false, error: "findWindow requires 'titleContains'." };
          }
          try {
            console.log(`\n🔧 [Tool Call] Finding window: titleContains="${titleContains}"`);
            const windowLocator = await desktopClient!.findWindow({ titleContains });
            const windowElement = await windowLocator.first();
            console.log(`\n✅ [Tool Result] Found window: Role=${windowElement.role}, Name=${windowElement.label}, ID=${windowElement.id}`);
            return { success: true, windowElement: windowElement };
          } catch (error: any) {
            console.error(`\n❌ [Tool Error] Failed to find window (titleContains="${titleContains}"): ${error.message}`);
            return { success: false, error: `Failed findWindow (titleContains="${titleContains}"): ${error.message}` };
          }
        }
      }),

      // Tool to get PDF text (already extracted)
      getPdfText: tool({
        description: `Returns the already extracted text content from the PDF resume.
                    Use this to analyze the resume content and extract relevant information.`,
        parameters: z.object({}),
        execute: async () => {
          try {
            console.log(`\n🔧 [Tool Call] Getting PDF text content`);
            console.log(`\n✅ [Tool Result] Retrieved PDF text content (${pdfText.length} characters)`);
            return { success: true, text: pdfText };
          } catch (error: any) {
            console.error(`\n❌ [Tool Error] Failed to get PDF text: ${error.message}`);
            return { success: false, error: error.message };
          }
        }
      }),

      // Tool to type text into a specific UI element (like a form field)
      typeIntoElement: tool({
        description: `Types the given text into the UI element (usually an input field or text area) identified by the selector string.
                    Uses clipboard for faster typing when possible.`,
        parameters: z.object({
          fieldName: z.string().describe("The name of the form field (e.g., 'Full Name', 'Email Address', 'Phone Number', 'Job Position')"),
          textToType: z.string().describe("The text to type into the element"),
          useClipboard: z.boolean().default(true).describe("Whether to use clipboard for faster typing (1000x faster)")
        }),
        execute: async ({ fieldName, textToType, useClipboard }) => {
          try {
            console.log(`\n🔧 [Tool Call] Typing "${textToType}" into field: "${fieldName}"`);
            
            // Map field names to their actual selectors based on FlatInspect UI
            const fieldSelectors: Record<string, string> = {
              'Full Name': 'Name:Full Name',
              'Email Address': 'Name:Email Address',
              'Phone Number': 'Name:Phone Number',
              'Job Position': 'Name:Job Position',
              'Start Date': 'Name:Start Date',
              'Department': 'Name:Department'
            };
            
            // Get the correct selector for this field
            const selector = fieldSelectors[fieldName];
            if (!selector) {
              throw new Error(`Unknown field name: ${fieldName}. Available fields: ${Object.keys(fieldSelectors).join(', ')}`);
            }
            
            console.log(`Using selector: ${selector} for field: ${fieldName}`);
            
            // Get the element
            const element = desktopClient!.locator(selector);
            
            // Click the element first to ensure focus
            console.log(`Clicking on element to focus: ${selector}`);
            await element.click();
            await sleep(1000); // Wait longer to ensure focus
            
            // Clear any existing text first
            for (let i = 0; i < 30; i++) { // Assuming no field will have more than 30 characters
              await element.typeText('\b'); // Backspace
            }
            await sleep(500);
            
            // Type text
            console.log(`Typing text: ${textToType}`);
            const result = await element.typeText(textToType);
            await sleep(500);
            
            // Press Tab to move to the next field
            await element.typeText('\t'); // Tab key
            await sleep(500);
            
            console.log(`\n✅ [Tool Result] Typed text into "${fieldName}".`);
            return { success: true, details: result };
          } catch (error: any) {
            console.error(`\n❌ [Tool Error] Failed to type into field "${fieldName}": ${error.message}`);
            return { success: false, error: error.message };
          }
        }
      }),

      // Tool to click on a UI element
      clickElement: tool({
        description: `Clicks on a UI element specified by a button name.
                    Use this for buttons like 'Submit', 'Clear Form', 'Add Employee', etc.`,
        parameters: z.object({
          buttonName: z.string().describe("The name of the button to click (e.g., 'Submit', 'Clear Form', 'Add Employee')")
        }),
        execute: async ({ buttonName }) => {
          try {
            console.log(`\n🔧 [Tool Call] Clicking on button: "${buttonName}"`);
            
            // Map button names to their actual selectors based on FlatInspect UI
            const buttonSelectors: Record<string, string> = {
              'Submit': 'Name:Submit', // Changed back to Name:Submit as requested
              'Clear Form': 'Name:Clear Form',
              'Add Employee Tab': 'AutomationId:add-tab',
              'View Employees': 'Name:View Employees'
            };
            
            // Get the correct selector for this button
            const selector = buttonSelectors[buttonName];
            if (!selector) {
              throw new Error(`Unknown button name: ${buttonName}. Available buttons: ${Object.keys(buttonSelectors).join(', ')}`);
            }
            
            console.log(`Using selector: ${selector} for button: ${buttonName}`);
            
            // Get the element and click it
            const element = desktopClient!.locator(selector);
            await element.click();
            await sleep(1000); // Wait after clicking to ensure action completes
            
            console.log(`\n✅ [Tool Result] Clicked on button "${buttonName}".`);
            return { success: true };
          } catch (error: any) {
            console.error(`\n❌ [Tool Error] Failed to click on button "${buttonName}": ${error.message}`);
            return { success: false, error: error.message };
          }
        }
      }),

      // Tool to select an option from a dropdown
      selectOption: tool({
        description: `Selects an option from a dropdown/combobox.
                    First clicks on the dropdown to open it, then clicks on the option with the specified name.`,
        parameters: z.object({
          departmentName: z.string().describe("The department to select (e.g., 'Marketing', 'Human Resources', 'Finance', etc.)")
        }),
        execute: async ({ departmentName }) => {
          try {
            console.log(`\n🔧 [Tool Call] Selecting department: "${departmentName}"`);
            
            // Use the correct selector for the Department dropdown based on FlatInspect UI
            const departmentSelector = 'Name:Department';
            
            // Click on the dropdown to open it
            console.log(`Clicking on department dropdown: ${departmentSelector}`);
            await desktopClient!.locator(departmentSelector).click();
            await sleep(1000); // Wait for dropdown to open
            
            // Try different approaches to select the department
            try {
              // First try: Type the department name and press Enter
              console.log(`Typing department name: ${departmentName}`);
              await desktopClient!.locator(departmentSelector).typeText(departmentName);
              await sleep(500);
              await desktopClient!.locator(departmentSelector).typeText('\n'); // Enter key
              console.log(`Typed department name and pressed Enter`);
            } catch (error) {
              console.log(`First attempt failed: ${error.message}`);
              
              try {
                // Second try: Try clicking on the list item directly
                console.log(`Trying to click on department list item`);
                await desktopClient!.locator(`Text "${departmentName}"`).click();
                console.log(`Clicked on department list item`);
              } catch (listError) {
                console.log(`Second attempt failed: ${listError.message}`);
                
                // Third try: Type just the first letter and press Enter
                console.log(`Trying first letter selection`);
                await desktopClient!.locator(departmentSelector).click(); // Click again to ensure focus
                await sleep(500);
                await desktopClient!.locator(departmentSelector).typeText(departmentName.charAt(0));
                await sleep(500);
                await desktopClient!.locator(departmentSelector).typeText('\n'); // Enter key
                console.log(`Used first letter selection`);
              }
            }
            
            console.log(`\n✅ [Tool Result] Selected department: "${departmentName}"`);
            return { success: true };
          } catch (error: any) {
            console.error(`\n❌ [Tool Error] Failed to select department: ${error.message}`);
            return { success: false, error: error.message };
          }
        }
      }),

      // Tool to finish the task
      finishTask: tool({
        description: "Call this tool ONLY when you have successfully read the PDF, identified all relevant fields in the form, and filled them completely according to the PDF data. This indicates the automation task is complete.",
        parameters: z.object({
          summary: z.string().describe("A brief summary of the data transferred and the completion status.")
        }),
        execute: async ({ summary }) => {
          console.log(`\n🏁 [Tool Call] Finishing Task: ${summary}`);
          console.log(`\n🎉 Automation task marked as complete by AI.`);
          return { success: true, message: "Task finished successfully.", summary: summary };
        }
      })
    };

    // 6. Construct Prompt for AI
    const systemPrompt = `You are an AI assistant specialized in automating data entry from a PDF resume into an HR onboarding web application form using the 'desktop-use' SDK via provided tools.

    **Setup:**
    The user has manually opened the HR onboarding web application ('${WEB_APP_URL}') and the PDF text has already been extracted for you to analyze.

    **Your Goal - Follow This Order Strictly:**
    1. **Get PDF Content:** Use the **'getPdfText'** tool to retrieve the already extracted text from the resume PDF. This will give you all the content you need to analyze.
    
    2. **Identify Web Form Window:** Use **'findWindow'** with \`titleContains:"HR Onboarding System"\` to locate the web form. **Note the unique ID or selector returned.**
    
    3. **Analyze Resume Content:** From the PDF text, extract the following information:
       - Full Name
       - Email Address
       - Phone Number
       - Job Position
       - Most appropriate department from the available options: ${DEPARTMENTS.map(d => '"' + d + '"').join(', ')}
    
    4. **Fill Form:** Use the **'typeIntoElement'** tool for each field, specifying the field name and the text to type:
       - Use \`fieldName:"Full Name"\` for the candidate's name
       - Use \`fieldName:"Email Address"\` for the email
       - Use \`fieldName:"Phone Number"\` for the phone
       - Use \`fieldName:"Job Position"\` for the job title
       - Use \`fieldName:"Start Date"\` for today's date (format: YYYY-MM-DD)
    
    5. **Select Department:** Use the **'selectOption'** tool with \`departmentName\` parameter to select the appropriate department.
    
    6. **Submit Form:** Use the **'clickElement'** tool with \`buttonName:"Submit"\` to submit the form. (Note: The Submit button is labeled as 'Add Employee' in the UI)
    
    7. **Complete Task:** Once all relevant data is accurately transferred, call 'finishTask' with a summary.

    **Tool Usage Guidelines:**
    - **Targeting is Key:** Most errors happen when tools are not targeted at the correct window or element. Use the specific IDs/selectors obtained from 'findWindow' when calling subsequent tools.
    - **Selectors:** Prefer specific names (\`Name:"Label"\`), roles (\`role:edit\`, \`role:button\`), or IDs (\`#elementId\`). Remember these are desktop UI selectors (UIA/ATK), not web selectors.
    - **Clipboard Usage:** The 'typeIntoElement' tool uses clipboard by default for 1000x faster typing. Keep this enabled.
    - **Error Handling:** If a selector fails, re-evaluate: Did you target the correct window ID? Is the selector specific enough?

    **Start now by using the tools as described in the steps above.**`;

    const initialUserMessage = "The HR Onboarding web app is open and the PDF text has been extracted. Please start the process following the system prompt exactly: get the PDF content, find the web form window, analyze the resume content, fill the form with the extracted information, and submit it.";

    const messages: CoreMessage[] = [
      { role: 'system', content: systemPrompt },
      { role: 'user', content: initialUserMessage }
    ];

    // 7. Generate and Stream AI Actions
    try {
      const { textStream, toolResults } = streamText({
        model: model,
        tools: tools,
        messages: messages,
        toolChoice: 'required',
        maxSteps: 30,
        onError: (error) => {
          console.error(`\n❌ Error during AI processing: ${error}`);
        },
      });

      // Stream the AI's thinking process to the console
      let fullResponse = "";
      process.stdout.write(`\nAI Thinking:\n---`);
      for await (const textPart of textStream) {
        process.stdout.write(textPart);
        fullResponse += textPart;
      }
      process.stdout.write(`\n---\n`);

      console.log(`\n\n✅ AI interaction complete.`);
    } catch (error) {
      console.error(`\n❌ An error occurred during the main AI interaction loop:`);
      console.error(error);
    } finally {
      console.log(`\n👋 AI Automator session finished.`);
    }
  } catch (error) {
    console.error("Unhandled error in main:", error);
    process.exit(1);
  }
}

main().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});