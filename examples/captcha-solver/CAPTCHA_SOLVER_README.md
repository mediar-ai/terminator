# CAPTCHA Solver Workflow

A Terminator MCP workflow that automatically detects and solves CAPTCHA challenges using AI vision models.

## Quick Start

```bash
# Test with Google's reCAPTCHA demo (default)
terminator mcp run examples/captcha-solver/captcha-cloudflare-solver.yml

# Test with custom URL
terminator mcp run examples/captcha-solver/captcha-cloudflare-solver.yml \
  --inputs target_url="https://your-site-with-captcha.com"
```

## What It Does

1. **Opens Chrome browser** and navigates to target URL
2. **Detects CAPTCHA challenges** using JavaScript DOM analysis
3. **Captures CAPTCHA images** and sends them to OpenAI Vision API
4. **Automatically solves** the CAPTCHA using AI
5. **Submits the solution** and verifies success
6. **Reports results** with detailed metrics

## Requirements

- **Chrome browser** installed
- **OpenAI API key** (replace `YOUR_OPENAI_API_KEY_HERE` in the workflow)
- **Terminator CLI** installed

## Supported Challenges

- **reCAPTCHA** (v2, v3, Enterprise)
- **hCaptcha** (standard, invisible)
- **Cloudflare Turnstile**
- **Generic image CAPTCHAs**

## Configuration

The workflow includes these configurable parameters:

- `target_url`: URL to solve CAPTCHA on (default: Google reCAPTCHA demo)
- `ai_provider`: AI service to use (default: "openai")
- `max_attempts`: Maximum solving attempts (default: 3)
- `challenge_timeout`: Seconds to wait for challenges (default: 30)

## Test URLs

```bash
# Google reCAPTCHA demo (default in workflow)
https://www.google.com/recaptcha/api2/demo

# hCaptcha demo
https://accounts.hcaptcha.com/demo

# Cloudflare Turnstile test
https://challenges.cloudflare.com/turnstile-test/
```

## Output

The workflow returns JSON with results:

```json
{
  "target_url": "https://www.google.com/recaptcha/api2/demo",
  "challenge_detected": true,
  "challenge_type": "recaptcha",
  "attempts_made": 1,
  "overall_success": true,
  "ai_provider_used": "openai",
  "solution_text": "ABC123",
  "timestamp": "2025-01-28T12:00:00.000Z"
}
```

## Setup

1. **Get OpenAI API key** from https://platform.openai.com/api-keys
2. **Edit the workflow file** and replace `YOUR_OPENAI_API_KEY_HERE` with your actual API key
3. **Run the command** above

## Troubleshooting

- **"AI API failed"**: Check your OpenAI API key and billing
- **"No CAPTCHA images found"**: Site may not have compatible CAPTCHAs
- **Browser doesn't open**: Ensure Chrome is installed and accessible

## Security Note

This tool is for educational and accessibility purposes. Always respect website terms of service and robots.txt when using automation tools.
