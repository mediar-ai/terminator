// src/types/terminator.augment.d.ts
declare module "@mediar-ai/terminator" {
  interface Desktop {
    navigateBrowser(
      url: string,
      browser?: string | null
    ): import("@mediar-ai/terminator").Element;
    delay(delayMs: number): Promise<void>;
  }

  interface Element {
    executeBrowserScript(
      script: string | ((env: any) => any) | { file: string; env?: any }
    ): Promise<string>;
    focus(): Promise<void>;
  }
}
