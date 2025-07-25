# HR Onboarding Bot

> ⚠️ **Warning:** Do not use Bun to run this project. Bun is not supported and will cause `fs` errors. Use Node.js only.

This project is an **automation suite** for HR onboarding, featuring a TypeScript automation script that extracts candidate data from a resume PDF and fills out a web-based HR onboarding form automatically.

---

## Features

- **Extracts candidate info** (name, email, phone, job position, department) from a resume PDF using Google Gemini AI.
- **Works with a web-based HR onboarding system**
- **Fills out the onboarding form** in the web application using desktop automation.
- **Submits the form** when automation is complete.

---

## Prerequisites

- Node.js (v18+ recommended)
- A Google Gemini API key (for AI extraction)
- Windows 10 or 11
- [Terminator desktop-use server](https://github.com/your-org/terminator) running (required for desktop automation)
- Access to the HR Onboarding web application (https://hr-onboarding-system.vercel.app/)

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

4. **Open the HR Onboarding web application:**
   - Open the HR Onboarding web application in your browser: https://hr-onboarding-system.vercel.app/
   - Position it side-by-side with the PDF viewer for best results.

---

## Usage

To run the automation (extract from PDF, fill web form, submit):

```bash
npm start
# or
npx tsx index.ts
```

**What happens:**
- The script extracts fields from `resume.pdf` using Gemini AI.
- It identifies the HR Onboarding web application in your browser.
- It fills the form with the extracted data using desktop automation.
- It submits the form when complete.

---

## Development

  ```bash
  ```

- The project uses `desktop-use` SDK for UI automation and `pdf-parse` for direct PDF text extraction.
