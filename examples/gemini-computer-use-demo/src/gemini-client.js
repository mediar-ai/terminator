/**
 * Gemini Computer Use Client
 * 
 * Integrates with Gemini's Computer Use model for vision-based automation.
 * Uses Terminator MCP for screenshots and desktop control.
 */

import { GoogleGenerativeAI } from '@google/generative-ai';
import fs from 'fs/promises';
import path from 'path';

export class GeminiComputerUseClient {
    constructor(apiKey, terminatorClient, options = {}) {
        this.apiKey = apiKey;
        this.model = options.model || 'gemini-2.5-flash-latest';
        this.genAI = new GoogleGenerativeAI(apiKey);
        this.terminator = terminatorClient; // Terminator MCP client for screenshots
        this.chat = null;
        this.history = [];
        this.screenshotDir = options.screenshotDir || './output';
    }

    /**
     * Initialize the Gemini client
     */
    async initialize() {
        console.log('Initializing Gemini Computer Use client...');

        // Ensure screenshot directory exists
        await fs.mkdir(this.screenshotDir, { recursive: true });

        console.log('✓ Gemini client initialized');
    }

    /**
     * Start a new chat session
     */
    startChat(systemInstruction = null) {
        const model = this.genAI.getGenerativeModel({
            model: this.model,
            systemInstruction: systemInstruction || this.getDefaultSystemInstruction()
        });

        this.chat = model.startChat({
            history: this.history
        });

        return this.chat;
    }

    /**
     * Get default system instruction for Computer Use
     */
    getDefaultSystemInstruction() {
        return `You are an AI assistant with computer use capabilities. You can analyze desktop screenshots and help with tasks like:
- Reading and extracting data from PDFs and documents shown on screen
- Understanding UI layouts and element positions
- Planning sequences of actions for automation tasks

When extracting data, always return it in a structured JSON format.
When describing UI elements, provide accessible element descriptions (role, name, etc.) not coordinates.`;
    }

    /**
     * Capture a screenshot using PowerShell (via Terminator run_command)
     * This is simpler than using Terminator's complex screenshot APIs
     */
    async captureScreenshot(filename = null) {
        const timestamp = Date.now();
        const screenshotPath = filename ||
            path.join(this.screenshotDir, `screenshot-${timestamp}.png`);

        console.log('   Capturing screenshot...');

        // Use PowerShell to take a screenshot
        const absolutePath = path.resolve(screenshotPath);
        const powershellScript = `
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$screen = [System.Windows.Forms.SystemInformation]::VirtualScreen
$bitmap = New-Object System.Drawing.Bitmap $screen.Width, $screen.Height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($screen.Left, $screen.Top, 0, 0, $bitmap.Size)
$bitmap.Save('${absolutePath.replace(/\\/g, '\\\\')}')
$graphics.Dispose()
$bitmap.Dispose()
        `.trim();

        await this.terminator.callTool('run_command', {
            run: `powershell -Command "${powershellScript.replace(/"/g, '\\"')}"`
        });

        // Wait for file to be written
        await new Promise(resolve => setTimeout(resolve, 1000));

        return screenshotPath;
    }

    /**
     * Open a file using Windows default application via Terminator
     */
    async openFile(filePath) {
        console.log(`   Opening file: ${filePath}`);

        // Use Windows 'start' command to open file with default app
        // Resolve to absolute path
        const absolutePath = path.resolve(filePath);

        const result = await this.terminator.callTool('run_command', {
            run: `Start-Process "${absolutePath}"`
        });

        // Wait for application to open and render
        await new Promise(resolve => setTimeout(resolve, 3000));

        return result;
    }

    /**
     * Capture screenshot of a specific window
     */
    async captureWindowScreenshot(windowTitle, filename = null) {
        const timestamp = Date.now();
        const screenshotPath = filename ||
            path.join(this.screenshotDir, `window-${timestamp}.png`);

        console.log(`   Capturing window "${windowTitle}" via Terminator...`);

        // Use Terminator to capture specific window
        const result = await this.terminator.callTool('capture_element_screenshot', {
            selector: `window:${windowTitle}`,
            output_path: path.resolve(screenshotPath)
        });

        await new Promise(resolve => setTimeout(resolve, 500));

        return screenshotPath;
    }

    /**
     * Read an image file and convert to base64
     */
    async readImageAsBase64(imagePath) {
        const imageBuffer = await fs.readFile(imagePath);
        return imageBuffer.toString('base64');
    }

    /**
    }

    /**
     * Extract data from a PDF document using Gemini vision
     * Sends the PDF directly to Gemini without screenshots
     */
    async extractDataFromDocument(filePath, extractionPrompt) {
        console.log('Reading PDF document...');

        // Read the PDF file directly
        const pdfBuffer = await fs.readFile(filePath);
        const pdfBase64 = pdfBuffer.toString('base64');

        console.log('Sending PDF to Gemini for extraction...');

        if (!this.chat) {
            this.startChat();
        }

        const result = await this.chat.sendMessage([
            {
                inlineData: {
                    mimeType: 'application/pdf',
                    data: pdfBase64
                }
            },
            { text: extractionPrompt }
        ]);

        const response = result.response;
        const text = response.text();

        // Try to parse as JSON
        try {
            // Look for JSON in the response
            const jsonMatch = text.match(/\{[\s\S]*\}/);
            if (jsonMatch) {
                return JSON.parse(jsonMatch[0]);
            }
            return { rawText: text };
        } catch (error) {
            console.warn('Failed to parse JSON from response, returning raw text');
            return { rawText: text };
        }
    }

    /**
     * Analyze a screenshot and get action suggestions
     */
    async analyzeScreenshot(screenshotPath, question) {
        const imageBase64 = await this.readImageAsBase64(screenshotPath);

        if (!this.chat) {
            this.startChat();
        }

        const result = await this.chat.sendMessage([
            {
                inlineData: {
                    mimeType: 'image/png',
                    data: imageBase64
                }
            },
            { text: question }
        ]);

        return result.response.text();
    }

    /**
     * Get a simple text response from Gemini
     */
    async ask(question) {
        if (!this.chat) {
            this.startChat();
        }

        const result = await this.chat.sendMessage(question);
        return result.response.text();
    }

    /**
     * Close and cleanup
     */
    async close() {
        // Nothing to close - Terminator client is managed externally
    }
}

// Test mode
if (process.argv.includes('--test')) {
    import('dotenv').then(dotenv => {
        dotenv.config();

        console.log('Testing Gemini Computer Use Client...');

        const apiKey = process.env.GEMINI_API_KEY;
        if (!apiKey) {
            console.error('✗ GEMINI_API_KEY not set in environment');
            process.exit(1);
        }

        // For testing, we need a mock Terminator client
        const mockTerminator = {
            callTool: async (tool, args) => {
                console.log(`   Mock Terminator: ${tool}`);
                return { content: [{ text: 'OK' }] };
            }
        };

        const client = new GeminiComputerUseClient(apiKey, mockTerminator);

        client.initialize()
            .then(async () => {
                console.log('\n✓ Gemini client initialized successfully');

                // Test: Simple question
                console.log('\nTesting basic query...');
                const response = await client.ask('What is 2+2? Answer with just the number.');
                console.log('✓ Response:', response);

                console.log('\n✓ All tests passed!');
                await client.close();
                process.exit(0);
            })
            .catch((error) => {
                console.error('\n✗ Test failed:', error);
                process.exit(1);
            });
    });
}
