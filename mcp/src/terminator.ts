import {
  Desktop,
  Element,
  Locator,
  ExploreResponse,
  ClickResult,
  CommandOutput,
  UIElementAttributes
} from "terminator.js";
import { z } from "zod";

// --- Zod Schemas for Tool Parameters (used in index.ts) ---

export const FindWindowSchema = z.object({
  titleContains: z
    .string()
    .describe(
      "A substring of the window title to search for (case-insensitive)."
    ),
  timeoutMs: z
    .number()
    .optional()
    .describe("Optional timeout in milliseconds."),
});

export const LocatorSchema = z.object({
  selectorChain: z
    .array(z.string())
    .describe(
      "An array of selector strings to locate the element (e.g., ['window:My App', 'button:OK'])."
    ),
    timeoutMs: z
    .number()
    .optional()
    .describe("Optional timeout in milliseconds for the action.")
});

export const GetElementTextSchema = LocatorSchema.extend({
  maxDepth: z
    .number()
    .optional()
    .describe("Maximum depth to search for text within child elements."),
});

export const TypeIntoElementSchema = LocatorSchema.extend({
  textToType: z.string().describe("The text to type into the element."),
});

export const PressKeySchema = LocatorSchema.extend({
    key: z.string().describe("The key or key combination to press (e.g., 'Enter', 'Ctrl+A').")
});

export const RunCommandSchema = z.object({
  windowsCommand: z
    .string()
    .optional()
    .describe("The command to run on Windows."),
  unixCommand: z
    .string()
    .optional()
    .describe("The command to run on Linux/macOS."),
});

export const ExploreSchema = z.object({
    selectorChain: z
    .array(z.string())
    .optional()
    .describe("Optional selector chain to explore from a specific element. Explores screen if omitted.")
})

// New schema for captureScreen (takes no arguments)
export const CaptureScreenSchema = z.object({});

// --- Terminator Tools Class ---

export class TerminatorTools {
  private desktop: Desktop;

  constructor(baseUrl?: string) {
    try {
      this.desktop = new Desktop(false, true, 'info'); // useBackgroundApps=false, activateApp=true, logLevel='info'
      console.log(JSON.stringify({
        type: "terminator_init",
        status: "success",
        config: { useBackgroundApps: false, activateApp: true, logLevel: 'info' },
        timestamp: new Date().toISOString()
      }));
    } catch (error: any) {
      console.error(JSON.stringify({
        type: "terminator_init",
        status: "error",
        error: error.message,
        timestamp: new Date().toISOString()
      }));
      throw error;
    }
  }

  /** Helper to handle errors */
  private handleError(error: unknown, operation: string): { error: string } {
    let errorMessage = `Terminator Error during ${operation}: Unknown error`;
    if (error instanceof Error) {
      errorMessage = `Error during ${operation}: ${error.message}`;
    }
    console.error(JSON.stringify({
      type: "terminator_error",
      operation: operation,
      error: errorMessage,
      timestamp: new Date().toISOString()
    }));
    return { error: errorMessage };
  }

  /** Finds a window and returns its details */
  async findWindow(
    args: z.infer<typeof FindWindowSchema>
  ): Promise<{ windowElement: any } | { error: string }> {
    try {
      console.log(JSON.stringify({
        type: "terminator_operation",
        operation: "findWindow",
        args: { titleContains: args.titleContains, timeoutMs: args.timeoutMs },
        timestamp: new Date().toISOString()
      }));
      const windowElement = await this.desktop.findWindowByCriteria(args.titleContains, args.timeoutMs);
      const attrs = windowElement.attributes();
      const id = windowElement.id();
      console.log(JSON.stringify({
        type: "terminator_result",
        operation: "findWindow", 
        status: "success",
        result: { role: attrs.role, name: attrs.name, id: id },
        timestamp: new Date().toISOString()
      }));
      return { 
        windowElement: {
          role: attrs.role,
          name: attrs.name,
          label: attrs.label,
          suggested_selector: id ? `#${id}` : `name:"${attrs.name || attrs.label}"`
        }
      };
    } catch (error) {
      return this.handleError(error, `findWindow(titleContains=${args.titleContains})`);
    }
  }

   /** Creates a locator with chained selectors */
   private getLocator(selectorChain: string[]): Locator {
        let locator = this.desktop.locator(selectorChain[0]);
        for (let i = 1; i < selectorChain.length; i++) {
            locator = locator.locator(selectorChain[i]);
        }
        return locator;
    }

  /** Gets text from an element */
  async getElementText(
    args: z.infer<typeof GetElementTextSchema>
  ): Promise<{ text: string } | { error: string }> {
    try {
       console.log(JSON.stringify({
         type: "terminator_operation",
         operation: "getElementText",
         args: { selectorChain: args.selectorChain, maxDepth: args.maxDepth, timeoutMs: args.timeoutMs },
         timestamp: new Date().toISOString()
       }));
       const locator = this.getLocator(args.selectorChain);
       const text = await locator.text(args.maxDepth, args.timeoutMs || 5000);
       console.log(JSON.stringify({
         type: "terminator_result",
         operation: "getElementText",
         status: "success",
         result: { text_length: text.length, text_preview: text.substring(0, 100) },
         timestamp: new Date().toISOString()
       }));
       return { text };
    } catch (error) {
         return this.handleError(error, `getElementText(${JSON.stringify(args.selectorChain)})`);
    }
  }

  /** Types text into an element */
  async typeIntoElement(
    args: z.infer<typeof TypeIntoElementSchema>
  ): Promise<{ success: boolean } | { error: string }> {
    try {
       console.log(JSON.stringify({
         type: "terminator_operation",
         operation: "typeIntoElement",
         args: { selectorChain: args.selectorChain, textLength: args.textToType.length, timeoutMs: args.timeoutMs },
         timestamp: new Date().toISOString()
       }));
       const locator = this.getLocator(args.selectorChain);
       await locator.typeText(args.textToType, false, args.timeoutMs || 5000);
       console.log(JSON.stringify({
         type: "terminator_result",
         operation: "typeIntoElement",
         status: "success",
         timestamp: new Date().toISOString()
       }));
       return { success: true };
    } catch (error) {
        return this.handleError(error, `typeIntoElement(${JSON.stringify(args.selectorChain)})`);
    }
  }

  /** Clicks an element */
  async clickElement(
    args: z.infer<typeof LocatorSchema>
  ): Promise<{ success: boolean, details?: string } | { error: string }> {
    try {
      console.log(JSON.stringify({
        type: "terminator_operation",
        operation: "clickElement",
        args: { selectorChain: args.selectorChain, timeoutMs: args.timeoutMs },
        timestamp: new Date().toISOString()
      }));
      const locator = this.getLocator(args.selectorChain);
      const result = await locator.click(args.timeoutMs || 5000);
      console.log(JSON.stringify({
        type: "terminator_result",
        operation: "clickElement",
        status: "success",
        result: { details: result.details },
        timestamp: new Date().toISOString()
      }));
      return { success: true, details: result.details };
    } catch (error) {
       return this.handleError(error, `clickElement(${JSON.stringify(args.selectorChain)})`);
    }
  }

   /** Presses a key or key combination on an element */
    async pressKey(
        args: z.infer<typeof PressKeySchema>
    ): Promise<{ success: boolean } | { error: string }> {
        try {
            console.log(JSON.stringify({
              type: "terminator_operation",
              operation: "pressKey",
              args: { selectorChain: args.selectorChain, key: args.key, timeoutMs: args.timeoutMs },
              timestamp: new Date().toISOString()
            }));
            const locator = this.getLocator(args.selectorChain);
            await locator.pressKey(args.key, args.timeoutMs || 5000);
            console.log(JSON.stringify({
              type: "terminator_result",
              operation: "pressKey",
              status: "success",
              timestamp: new Date().toISOString()
            }));
            return { success: true };
        } catch (error) {
            return this.handleError(error, `pressKey(${JSON.stringify(args.selectorChain)}, key=${args.key})`);
        }
    }

    /** Runs a shell command */
    async runCommand(
        args: z.infer<typeof RunCommandSchema>
    ): Promise<CommandOutput | { error: string }> {
        try {
            console.log(JSON.stringify({
              type: "terminator_operation",
              operation: "runCommand",
              args: { windowsCommand: args.windowsCommand, unixCommand: args.unixCommand },
              timestamp: new Date().toISOString()
            }));
            const result = await this.desktop.runCommand(args.windowsCommand, args.unixCommand);
            console.log(JSON.stringify({
              type: "terminator_result",
              operation: "runCommand",
              status: "success",
              result: { exitStatus: result.exitStatus, stdoutLength: result.stdout.length, stderrLength: result.stderr.length },
              timestamp: new Date().toISOString()
            }));
            return result;
        } catch (error) {
            return this.handleError(error, `runCommand(windows=${args.windowsCommand}, unix=${args.unixCommand})`);
        }
    }

    /** Explores elements on screen or within a parent */
    async explore(
        args: z.infer<typeof ExploreSchema>
    ): Promise<ExploreResponse | { error: string }> {
        try {
            console.log(JSON.stringify({
              type: "terminator_operation",
              operation: "explore",
              args: { selectorChain: args.selectorChain || ["screen"] },
              timestamp: new Date().toISOString()
            }));
            
            let exploreResult: ExploreResponse;
            if (args.selectorChain && args.selectorChain.length > 0) {
                const locator = this.getLocator(args.selectorChain);
                exploreResult = await locator.explore(5000);
            } else {
                exploreResult = this.desktop.root().explore();
            }
            
            console.log(JSON.stringify({
              type: "terminator_result",
              operation: "explore",
              status: "success",
              result: { children_count: exploreResult.children.length },
              timestamp: new Date().toISOString()
            }));
            return exploreResult;
        } catch (error) {
            return this.handleError(error, `explore(${args.selectorChain ? JSON.stringify(args.selectorChain) : 'screen'})`);
        }
    }

    /** Activates an app/window */
    async activateApp(
        args: z.infer<typeof LocatorSchema>
    ): Promise<{ success: boolean } | { error: string }> {
        try {
            console.log(JSON.stringify({
              type: "terminator_operation",
              operation: "activateApp",
              args: { selectorChain: args.selectorChain, timeoutMs: args.timeoutMs },
              timestamp: new Date().toISOString()
            }));
            const locator = this.getLocator(args.selectorChain);
            const element = await locator.first();
            element.activateWindow();
            console.log(JSON.stringify({
              type: "terminator_result",
              operation: "activateApp",
              status: "success",
              timestamp: new Date().toISOString()
            }));
            return { success: true };
        } catch (error) {
            return this.handleError(error, `activateApp(${JSON.stringify(args.selectorChain)})`);
        }
    }

    /** Captures screen and performs OCR */
    async captureScreen(): Promise<{ text: string } | { error: string }> {
        try {
            console.log(JSON.stringify({
              type: "terminator_operation",
              operation: "captureScreen",
              args: {},
              timestamp: new Date().toISOString()
            }));
            const screenshot = await this.desktop.captureScreen();
            const text = await this.desktop.ocrScreenshot(screenshot);
            console.log(JSON.stringify({
              type: "terminator_result",
              operation: "captureScreen",
              status: "success",
              result: { text_length: text.length, screenshot_size: `${screenshot.width}x${screenshot.height}` },
              timestamp: new Date().toISOString()
            }));
            return { text };
        } catch (error) {
            return this.handleError(error, 'captureScreen()');
        }
    }

    /** Lists all open windows */
    async listOpenWindows(): Promise<{ windows: { title: string | null, selector: string }[] } | { error: string }> {
        try {
            console.log(JSON.stringify({
              type: "terminator_operation",
              operation: "listOpenWindows",
              args: {},
              timestamp: new Date().toISOString()
            }));
            const apps = this.desktop.applications();
            const windows = apps.map(app => {
                const attrs = app.attributes();
                const id = app.id();
                return {
                    title: attrs.name || attrs.label || null,
                    selector: id ? `#${id}` : `name:"${attrs.name || attrs.label}"`
                };
            });
            console.log(JSON.stringify({
              type: "terminator_result",
              operation: "listOpenWindows",
              status: "success",
              result: { windows_count: windows.length },
              timestamp: new Date().toISOString()
            }));
            return { windows };
        } catch (error) {
            return this.handleError(error, 'listOpenWindows()');
        }
    }
}
