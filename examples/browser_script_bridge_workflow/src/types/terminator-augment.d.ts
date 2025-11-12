// Local type augmentation to ensure Desktop has helper APIs available in examples
// This helps editors that may not pick up the full union from the monorepo build.
declare module '@mediar-ai/terminator' {
  interface Desktop {
    openUrl(url: string, browser?: string | null): import('@mediar-ai/terminator').Element;
    navigateBrowser(url: string, browser?: string | null): import('@mediar-ai/terminator').Element;
    delay(delayMs: number): Promise<void>;
  }
}
