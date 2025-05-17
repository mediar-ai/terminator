import * as fs from 'fs';
import * as path from 'path';
import * as dotenv from 'dotenv';
import inquirer from 'inquirer';
import pdfParse from 'pdf-parse';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { streamText } from 'ai';
import { DesktopUseClient, ApiError, sleep } from 'desktop-use';
import { spawn, execSync } from 'child_process';

// Load .env if present
const ENV_PATH = path.resolve(__dirname, './.env');
dotenv.config({ path: ENV_PATH });

const PDF_FILE_PATH = path.resolve(__dirname, './resume.pdf');

const DEPARTMENTS = [
  'Human Resources',
  'Information Technology',
  'Finance',
  'Marketing',
  'Operations',
];

const APP_TITLE = 'HR Onboarding System'; // Adjust this to match the actual window title of your app

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
      fs.appendFileSync(ENV_PATH, `\nGEMINI_API_KEY=${apiKey}`);
      dotenv.config({ path: ENV_PATH, override: true });
    } catch (err) {
      console.error('Error saving API key to .env:', err);
    }
  }
  if (!apiKey) throw new Error('Failed to get Gemini API Key.');
  return apiKey;
}

async function extractPdfText(pdfPath: string): Promise<string> {
  const data = fs.readFileSync(pdfPath);
  const pdfData = await pdfParse(data);
  return pdfData.text;
}

async function askGeminiForFields(pdfText: string, model: any): Promise<any> {
  const prompt = `Extract the following fields from this resume text. If not found, return null for that field.\n\nResume Text:\n"""\n${pdfText}\n"""\n\nReturn a JSON object with these keys:\n- fullName\n- email\n- phone\n- jobPosition\n- department (must be one of: ${DEPARTMENTS.map(d => '"' + d + '"').join(', ')})\n`;

  const { textStream } = streamText({
    model,
    messages: [
      { role: 'system', content: 'You are a helpful assistant that extracts structured data from resumes.' },
      { role: 'user', content: prompt },
    ],
    maxTokens: 1024,
  });

  let fullText = '';
  for await (const part of textStream) {
    process.stdout.write(part);
    fullText += part;
  }
  // Try to parse JSON from the response
  let json: any = null;
  try {
    // Find first {...} block
    const match = fullText.match(/\{[\s\S]*\}/);
    if (match) {
      json = JSON.parse(match[0]);
    } else {
      throw new Error('No JSON found in Gemini response.');
    }
  } catch (err) {
    console.error('Failed to parse Gemini response as JSON:', err);
    console.error('Gemini raw response:', fullText);
    throw err;
  }
  return json;
}

async function fillFormWithDesktopUse(data: any) {
  const client = new DesktopUseClient();
  try {
    await sleep(3000);
    const app = client.locator(`Name:${APP_TITLE}`);
    await app.click();

    const fullName = app.locator('Name:Full Name');
    await fullName.click();
    await fullName.typeText(data.fullName);

    const email = app.locator('Name:Email Address');
    await email.click();
    await email.typeText(data.email);

    const phone = app.locator('Name:Phone Number');
    await phone.click();
    await phone.typeText(data.phone);

    const jobPosition = app.locator('Name:Job Position');
    await jobPosition.click();
    await jobPosition.typeText(data.jobPosition);

    await app.locator('ComboBox:Department').click();
    const combobox = client.locator('role:List');
    await combobox.locator(`Name:${data.department}`).click();

    const date = app.locator('Name:Start Date');
    await date.typeText(new Date().toISOString().slice(0, 10).replace(/-/g, ''));

    await app.locator('Name:Submit Onboarding Form').locator('role:Text').click();
    console.log('Form submitted!');
  } catch (e) {
    if (e instanceof ApiError) {
      console.error('API Status:', e);
    } else {
      console.error('An unexpected error occurred:', e);
    }
  }
}

function killProcess(proc: any) {
  if (!proc) return;
  const pid = proc.pid;
  if (!pid) return;
  if (process.platform === 'win32') {
    try {
      execSync(`taskkill /PID ${pid} /T /F`);
    } catch (e) { /* ignore */ }
  } else {
    try {
      process.kill(-pid);
    } catch (e) { /* ignore */ }
  }
}

async function main() {
  console.log('--- PDF Resume to HR Onboarding Form ---');
  let appProcess: any = null;
  try {
    const apiKey = await getApiKey();
    const google = createGoogleGenerativeAI({ apiKey });
    const model = google('models/gemini-2.0-flash');

    // 1. Extract text from PDF
    console.log('Reading PDF:', PDF_FILE_PATH);
    const pdfText = await extractPdfText(PDF_FILE_PATH);
    console.log('PDF text extracted. Sending to Gemini...');

    // 2. Ask Gemini for fields
    const fields = await askGeminiForFields(pdfText, model);
    console.log('\n--- Extracted Fields ---');
    console.log(JSON.stringify(fields, null, 2));

    // 3. Start the Electron app just before filling the form
    const electronPath = require('electron');
    appProcess = spawn(electronPath, ['.'], {
      cwd: path.resolve(__dirname, 'hr-onboarding'),
      stdio: 'ignore',
      detached: true,
    });
    console.log('Electron app started. Waiting for it to be ready...');
    await sleep(2000); // Wait for the app to be ready (adjust as needed)

    // 4. Fill the HR onboarding form using desktop-use
    await fillFormWithDesktopUse(fields);
  } finally {
    await sleep(2000);
    // Kill the Electron app when done
    killProcess(appProcess);
    console.log('Electron app closed.');
  }
}

main().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
}); 