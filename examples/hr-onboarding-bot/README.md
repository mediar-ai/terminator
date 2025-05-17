# HR Onboarding Bot

> ⚠️ **Warning:** Do not use Bun to run this project. Bun is not supported and will cause `fs` errors. Use Node.js only.

This project is an **automation suite** for HR onboarding, combining an Electron-based HR onboarding app and a TypeScript automation script that extracts candidate data from a resume PDF and fills the onboarding form automatically.

---

## Features

- **Extracts candidate info** (name, email, phone, job position, department) from a resume PDF using Google Gemini AI.
- **Launches the Electron HR onboarding app automatically.**
- **Fills out the onboarding form** in the Electron app using desktop automation.
- **Closes the Electron app** when automation is complete.

---

## Prerequisites

- Node.js (v18+ recommended)
- A Google Gemini API key (for AI extraction)
- Windows 10 or 11
- [Terminator desktop-use server](https://github.com/your-org/terminator) running (required for desktop automation)

---

## Setup

1. **Install dependencies:**
   ```bash
   npm install
   ```

2. **Add your Gemini API key:**
   - On first run, you will be prompted for your Gemini API key. It will be saved in `.env` as `GEMINI_API_KEY=...`.

3. **Place your resume PDF:**
   - Replace `resume.pdf` in the root with your own, or use the provided example.

---

## Usage

To run the full automation (extract, launch app, fill form, close app):

```bash
npm start
# or
npx tsx index.ts
```

**What happens:**
- The script extracts fields from `resume.pdf` using Gemini AI.
- It launches the Electron HR onboarding app.
- It fills the form with the extracted data.
- It closes the Electron app when done.

---

## Development

- To run only the Electron app (for development):
  ```bash
  cd hr-onboarding
  npm install
  npm start
  ```
