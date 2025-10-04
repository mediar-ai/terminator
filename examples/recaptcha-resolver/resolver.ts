import { GoogleGenAI } from "@google/genai";
import jpeg from "jpeg-js";
import { Buffer } from "buffer";
import { Desktop, type ScreenshotResult, type Locator } from "terminator.js";

const sleep = async (ms: number) => {
  return new Promise(resolve => setTimeout(resolve, ms));
};

async function runResolverAutomation(): Promise<string> {
  try {
    const desktop = new Desktop();
    const chrome_app = desktop.application("chrome").window()!;
    chrome_app?.focus();

    const browser_webview = chrome_app.locator("classname:BrowserRootView >> nativeid:RootWebArea");
    await browser_webview.wait(5000)

    const solve_captcha_btn = await browser_webview.locator("nativeid:recaptcha-anchor").first();

    if (solve_captcha_btn.isVisible()) {
      solve_captcha_btn.click();

      let recaptcha_webview_sel_clone: Locator;
      try {
        const recaptcha_webview_sel = browser_webview.locator("nativeid:rc-imageselect");
        await recaptcha_webview_sel.wait(30000)
        recaptcha_webview_sel_clone = recaptcha_webview_sel;
      } catch (e) {
        const result = {
          status: 'unknown',
          message: "recaptcha webview doesn't exist, it seems recaptcha might have solved by just clicking",
        };
        return JSON.stringify(result);
      }

      const recaptcha_webview = await recaptcha_webview_sel_clone.first();
      await sleep(10000)

      const recaptcha_image: ScreenshotResult = recaptcha_webview.capture();

      const jpegImageData = {
        data: Buffer.from(recaptcha_image.imageData),
        width: recaptcha_image.width,
        height: recaptcha_image.height,
      };

      const jpegBuffer = jpeg.encode(jpegImageData, 90).data;
      const base64Jpeg = Buffer.from(jpegBuffer).toString('base64');

      const ai = new GoogleGenAI({
        apiKey: process.env.GEMINI_API_KEY || "" 
      });

      const res_from_browser = recaptcha_webview?.executeBrowserScript(`
        const iframe = document.querySelector('iframe[title^="recaptcha challenge"]');
        const iframeDoc = iframe.contentDocument || iframe.contentWindow.document;
        const table = iframeDoc.querySelector("#rc-imageselect-target > table")
        return table ? table.outerHTML : null;
      `);

      const image_block_html_table = await res_from_browser;
      const htmlTableTag = image_block_html_table?.replace(/https?:\/\/[^\s"'<>]+/gi, '');

      let prompt = `
        here is an image and an html table. your task is to resolve the captcha from the image.
        based on the image, identify the correct blocks and then select the corresponding row and column from the following html table:
        ${htmlTableTag}
        NOTE: **ALWAYS RETURN THE HTML ID IN A ARRAY AT THE END OF RESPONSE TEXT**
      `;
      const contents = [
        {
          inlineData: {
            mimeType: "image/jpeg",
            data: base64Jpeg,
          },
        },
        { text: prompt },
      ];
      const response = await ai.models.generateContent({
        model: "gemini-2.5-flash",
        contents: contents,
      });

      const aiResponseText = response.text!;
      console.log("Response", aiResponseText);
      const arrayStringMatch = aiResponseText.match(/(\[.*?\])/);
      if (!arrayStringMatch) {
        throw new Error("Could not find an array of IDs in the AI response.");
      }

      const arrayString = arrayStringMatch[0];
      let idsToClick: string[];
      try {
        idsToClick = JSON.parse(arrayString);
      } catch (parseError) {
        console.error("Failed to parse the array from AI response:", arrayString);
        throw new Error("AI response did not contain a valid JSON array.");
      }

      if (!Array.isArray(idsToClick) || idsToClick.length === 0) {
        throw new Error("No valid IDs were parsed from the AI response.");
      }

      console.log("parsed ids to click:", idsToClick);

      const clickResult = await recaptcha_webview?.executeBrowserScript(`
        let errors = [];
        let clickedCount = 0;
        const iframe = document.querySelector('iframe[title^="recaptcha challenge"]');
        const iframeDoc = iframe.contentDocument || iframe.contentWindow.document;
        const ids = ${JSON.stringify(idsToClick)};

        ids.forEach(id => {
          const element = iframeDoc.getElementById(id);
          if (element) {
            element.click();
            clickedCount++;
          } else {
            errors.push('element with id "' + id + '" not found.');
          }
        });
        setTimeout(() => {
          const verifyBtn = iframeDoc.querySelector('button[id^="recaptcha-verify-button"]');
          verifyBtn.click();
        }, 3000);

        JSON.stringify({
          success: errors.length === 0,
          clicked: clickedCount,
          total: ids.length,
          errors: errors
        })
      `);

      if (clickResult) {
        const result = {
          status: 'success',
          message: `successfully resolved imgaed based recaptcha: ${clickResult}`,
        }
        return JSON.stringify(result);
      } else {
        const result = {
          status: 'failed',
          message: `failed to resolved imgaed based recaptcha, browser script execution failed`,
        }
        return JSON.stringify(result);
      }
    } else {
      const result = {
        status: 'failed',
        message: 'Captcha checkbox button is not visible.',
      };
      return JSON.stringify(result);
    }
  } catch(e) {
    const result = {
      status: 'failed',
      message: `failed to find root of captcha, make sure your opened site have captcha visible:\n ${e instanceof Error ? e.message : String(e)}`,
    };
    return JSON.stringify(result);
  }
}

runResolverAutomation().then(result => {
  console.log(result);
});

