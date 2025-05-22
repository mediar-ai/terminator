const DEFAULT_BASE_URL = "http://127.0.0.1:9375";

// --- Interfaces for API Payloads and Responses matching server.rs --- //

// General Responses
export interface BasicResponse {
  message: string;
  title: string;
}

export interface BooleanResponse {
  result: boolean;
}

export interface TextResponse {
  text: string;
}

// Command Output Response
export interface CommandOutputResponse {
  stdout: string;
  stderr: string;
  exit_code: number | null;
}

// Screenshot Response
export interface ScreenshotResponse {
  image_base64: string; // Base64 encoded image data
  width: number;
  height: number;
}

// OCR Response
export interface OcrResponse {
  text: string;
}

// Element Related Responses
export interface ElementResponse {
  role: string;
  label?: string | null;
  id?: string | null;
  text?: string | null;
  bounds?: [number, number, number, number] | null;
  visible?: boolean | null;
  enabled?: boolean | null;
  focused?: boolean | null;
  children?: ElementResponse[] | null;
}

export interface ElementsResponse {
  elements: ElementResponse[];
}

export interface ClickResponse {
  method: string;
  coordinates?: [number, number] | null; // Changed from (f64, f64) to tuple
  details: string;
}

export interface AttributesResponse {
  role: string;
  label?: string | null;
  value?: string | null;
  description?: string | null;
  properties: { [key: string]: any | null }; // Using 'any' for simplicity with serde_json::Value
  id?: string | null;
  is_keyboard_focusable?: boolean | null;
}

export interface BoundsResponse {
  x: number;
  y: number;
  width: number;
  height: number;
}

// Base Request with Selector Chain
interface ChainedRequest {
  selector_chain: string[];
}

// Specific Action Requests
interface TypeTextRequest extends ChainedRequest {
  text: string;
}

interface GetTextRequest extends ChainedRequest {
  max_depth?: number | null;
}

interface PressKeyRequest extends ChainedRequest {
  key: string;
}

// App/URL Requests
interface OpenApplicationRequest {
  app_name: string;
}

interface ActivateApplicationRequest {
  app_name: string;
}

interface OpenUrlRequest {
  url: string;
  browser?: string | null;
}

// New Top-Level Requests
interface OpenFileRequest {
  file_path: string;
}

interface RunCommandRequest {
  windows_command?: string | null;
  unix_command?: string | null;
}

interface CaptureMonitorRequest {
  monitor_name: string;
}

interface OcrImagePathRequest {
  image_path: string;
}

// Request for OCR on raw screenshot data
// REMOVED OcrScreenshotRequest interface as it's no longer used
// interface OcrScreenshotRequest {
//   image_base64: string;
//   width: number;
//   height: number;
// }

// --- Expectation Requests --- //

// Base request for expect actions, includes timeout
interface ExpectRequest extends ChainedRequest {
  timeout_ms?: number | null; // Optional timeout override in milliseconds
}

// Specific request for expecting text
interface ExpectTextRequest extends ExpectRequest {
  // Inherits selector_chain and timeout_ms
  expected_text: string;
  max_depth?: number | null; // Optional depth for text comparison
}

// Add this interface
interface ActivateBrowserWindowRequest {
  title: string;
}

// --- NEW Interfaces for FindWindow and Explore ---

export interface FindWindowRequest {
  title_contains?: string | null;
  timeout_ms?: number | null;
}

// Note: FindWindow response uses existing ElementResponse

export interface ExploreRequest {
  selector_chain?: string[] | null; // Make selector chain optional
}

export interface ExploredElementDetail {
  role: string;
  name?: string | null; // Corresponds to label in ElementResponse
  id?: string | null;
  parent_id?: string | null;
  children_ids?: string[] | null;
  bounds?: BoundsResponse | null;
  value?: string | null;
  description?: string | null;
  suggested_selector: string;
  text?: string | null;
}

export interface ExploreResponse {
  parent: ElementResponse; // Details of the parent element explored
  children: ExploredElementDetail[]; // List of direct children details
}

// --- Custom Error --- //

export class ApiError extends Error {
  status?: number;
  constructor(message: string, status?: number) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    // Set the prototype explicitly for proper instanceof checks
    Object.setPrototypeOf(this, ApiError.prototype);
  }
}

// --- Client and Locator Classes --- //

export class DesktopUseClient {
  private host: string;
  private port: number;
  private protocol: string;

  constructor(baseUrl: string = DEFAULT_BASE_URL) {
    const [protocol, hostPort] = baseUrl.split("://");
    if (!hostPort) {
      throw new Error(
        `Invalid baseUrl format: ${baseUrl}. Expected format: 'protocol://host:port'`
      );
    }
    const [host, portStr] = hostPort.split(":");
    if (!host || !portStr) {
      throw new Error(
        `Invalid baseUrl format: ${baseUrl}. Expected format: 'protocol://host:port'`
      );
    }
    this.host = host;
    this.port = parseInt(portStr, 10);
    if (isNaN(this.port)) {
      throw new Error(`Invalid port number in baseUrl: ${portStr}`);
    }
    this.protocol = protocol || "http";
  }

  /**
   * Internal method to make requests to the Terminator server.
   * Exposed for use by the Locator class, but intended for internal use.
   * @internal
   */
  public async _makeRequest<T>(endpoint: string, payload: object): Promise<T> {
    const jsonPayload = JSON.stringify(payload);
    const url = `${this.protocol}://${this.host}:${this.port}${endpoint}`;
    console.log(
      `[DesktopUseClient] Sending POST to ${url} with payload: ${jsonPayload}`
    );

    const options: RequestInit = {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        // Content-Length is typically handled by fetch automatically
      },
      body: jsonPayload,
    };

    try {
      const response = await fetch(url, options);
      const responseText = await response.text(); // Read body once

      console.log(`[DesktopUseClient] Response Status: ${response.status}`);
      console.log(`[DesktopUseClient] Response Data: ${responseText}`);

      if (response.ok) {
        // status in the range 200-299
        try {
          // Handle No Content (204) or empty body
          if (!responseText || response.status === 204) {
            return { message: "Operation successful" } as T; // Return generic success
          }
          return JSON.parse(responseText) as T; // Parse JSON
        } catch (e) {
          console.error(
            "[DesktopUseClient] Error: Could not decode JSON response despite success status.",
            e
          );
          // Don't reject here if the text response might be useful, but it's likely an error
          // If plain text is a valid success response, adjust logic
          // For now, treat as error if JSON parsing fails on success status > 204
          if (response.status !== 204) {
            throw new ApiError(
              `Invalid JSON response from server for endpoint ${url}`,
              response.status
            );
          }
          // If it was 204, we already returned success, this catch shouldn't be hit for JSON error
          // If somehow it's 204 and parsing empty string fails, return success anyway
          return { message: "Operation successful (204)" } as T;
        }
      } else {
        // Handle HTTP errors (status code >= 300)
        let errorMessage = `Server returned status ${response.status} for endpoint ${url}`;
        try {
          // Attempt to parse error response for more detail
          const errorData = JSON.parse(responseText);
          // Prepend the specific endpoint info if possible
          errorMessage = `${errorData?.message || `Error from ${url}`}`;
        } catch (e) {
          // Ignore JSON parse error, use the raw responseText as detail if available
           errorMessage += responseText ? ` - ${responseText}` : '';
        }
        console.error(
          `[DesktopUseClient] Error: ${errorMessage}, Detail: ${responseText}`
        );
        throw new ApiError(errorMessage, response.status);
      }
    } catch (error) {
      // Handle network errors or errors thrown above
      const errorMessagePrefix = `[DesktopUseClient] Request error for endpoint ${url}:`;
      console.error(`${errorMessagePrefix} ${error}`);
      if (error instanceof ApiError) {
        // Re-throw ApiError instances directly, potentially adding URL if missing
        if (!error.message.includes(url)) {
             error.message = `Error during request to ${url}: ${error.message}`;
        }
        throw error;
      } else if (
        error instanceof TypeError &&
        error.message.includes("fetch")
      ) {
        // Network errors often manifest as TypeErrors in fetch
        throw new ApiError(
          `Network error connecting to ${url}. Is the server running? (${error.message})`
        );
      } else {
        // Wrap other errors
        throw new ApiError(
          `Request failed for endpoint ${url}: ${
            error instanceof Error ? error.message : String(error)
          }`
        );
      }
    }
  }

  /**
   * Creates a new Locator instance starting from the root of the application hierarchy.
   * @param selector - The initial selector string (e.g., 'window:My App', 'Id:someId', 'Name:Click Me').
   * @returns A new Locator instance.
   */
  locator(selector: string): Locator {
    if (!selector || typeof selector !== "string") {
      throw new Error("Initial selector must be a non-empty string.");
    }
    return new Locator(this, [selector]);
  }

  /**
   * Opens an application by its name or path.
   * @param appName - The name or path of the application to open.
   * @returns A promise resolving to a basic response on success.
   */
  async openApplication(appName: string): Promise<BasicResponse> {
    const payload: OpenApplicationRequest = { app_name: appName };
    return await this._makeRequest<BasicResponse>("/open_application", payload);
  }

  /**
   * Activates an application by its name or path, bringing its window to the foreground.
   * @param appName - The name or path of the application to activate.
   * @returns A promise resolving to a basic response on success.
   */
  async activateApplication(appName: string): Promise<BasicResponse> {
    const payload: ActivateApplicationRequest = { app_name: appName };
    return await this._makeRequest<BasicResponse>("/activate_application", payload);
  }

  /**
   * Opens a URL, optionally specifying the browser.
   * @param url - The URL to open.
   * @param browser - Optional name or path of the browser to use. Uses system default if null/undefined.
   * @returns A promise resolving to a basic response on success.
   */
  async openUrl(url: string, browser?: string | null): Promise<BasicResponse> {
    const payload: OpenUrlRequest = { url, browser };
    return await this._makeRequest<BasicResponse>("/open_url", payload);
  }

  // --- New Top-Level Methods --- //

  /**
   * Opens a file using its default associated application.
   * @param filePath - The path to the file to open.
   * @returns A promise resolving to a basic response on success.
   */
  async openFile(filePath: string): Promise<BasicResponse> {
    const payload: OpenFileRequest = { file_path: filePath };
    return await this._makeRequest<BasicResponse>("/open_file", payload);
  }

  /**
   * Executes a command, choosing the appropriate one based on the server's OS.
   * Provide at least one of windowsCommand or unixCommand.
   * @param options - An object containing optional windowsCommand and/or unixCommand.
   * @returns A promise resolving to the command's output (stdout, stderr, exit code).
   */
  async runCommand(options: {
    windowsCommand?: string | null;
    unixCommand?: string | null;
  }): Promise<CommandOutputResponse> {
    if (!options.windowsCommand && !options.unixCommand) {
      throw new Error(
        "At least one of windowsCommand or unixCommand must be provided."
      );
    }
    const payload: RunCommandRequest = {
      windows_command: options.windowsCommand,
      unix_command: options.unixCommand,
    };
    return await this._makeRequest<CommandOutputResponse>(
      "/run_command",
      payload
    );
  }

  /**
   * Captures a screenshot of the primary monitor and performs OCR.
   * @returns A promise resolving to the OCR result containing the extracted text.
   */
  async captureScreen(): Promise<OcrResponse> {
    // No payload needed for the combined capture_screen endpoint
    console.log(`[DesktopUseClient] Requesting screen capture and OCR...`);
    const result = await this._makeRequest<OcrResponse>("/capture_screen", {});
    console.log(`[DesktopUseClient] Received OCR text (length: ${result.text.length})`);
    return result;
  }

  /**
   * Captures a screenshot of a specific monitor by its name.
   * Monitor names are platform-specific.
   * @param monitorName - The name of the monitor to capture.
   * @returns A promise resolving to the screenshot details (base64 image, width, height).
   */
  async captureMonitorByName(monitorName: string): Promise<ScreenshotResponse> {
    const payload: CaptureMonitorRequest = { monitor_name: monitorName };
    // This still returns ScreenshotResponse as the backend likely keeps this separate
    return await this._makeRequest<ScreenshotResponse>(
      "/capture_monitor",
      payload
    );
  }

  /**
   * Performs OCR on an image file located at the given path.
   * The server running the Terminator backend must have access to this path.
   * @param imagePath - The path to the image file.
   * @returns A promise resolving to the extracted text.
   */
  async ocrImagePath(imagePath: string): Promise<OcrResponse> {
    const payload: OcrImagePathRequest = { image_path: imagePath };
    return await this._makeRequest<OcrResponse>("/ocr_image_path", payload);
  }

  /**
   * Activates a browser window if it contains an element (like a tab) with the specified title.
   * This brings the browser window to the foreground.
   * Note: This does not guarantee the specific tab will be active within the window.
   * @param title - The title (or partial title) of the tab/element to search for within browser windows.
   * @returns A promise resolving to a basic response on success.
   */
  async activateBrowserWindowByTitle(title: string): Promise<BasicResponse> {
    const payload: ActivateBrowserWindowRequest = { title };
    return await this._makeRequest<BasicResponse>("/activate_browser_window", payload);
  }

  // --- NEW findWindow Method ---
  /**
   * Finds a top-level window based on specified criteria.
   * This is often the first step in locating elements within a specific application.
   * @param criteria - An object containing search criteria like `titleContains` or `processName`.
   * @param options - Optional settings like `timeout` in milliseconds.
   * @returns A new Locator instance scoped to the found window element.
   * @throws {ApiError} If no window is found matching the criteria within the timeout.
   */
  async findWindow(
    criteria: { titleContains?: string | null },
    options?: { timeout?: number | null }
  ): Promise<Locator> {
    if (!criteria.titleContains) {
      throw new Error("At least one criterion (titleContains) must be provided to findWindow.");
    }
    const payload: FindWindowRequest = {
      title_contains: criteria.titleContains,
      timeout_ms: options?.timeout,
    };
    // Call the backend endpoint to find the window
    const windowElement = await this._makeRequest<ElementResponse>("/find_window", payload);

    // Construct a "stable" selector for this specific window to create the Locator.
    // Using the returned ID is the most reliable if available.
    // Otherwise, fallback to role and name (label).
    let windowSelector: string;
    if (windowElement.id) {
      windowSelector = `#${windowElement.id}`;
    } else if (windowElement.label) {
      // Use role:Name format if label exists but ID doesn't
      windowSelector = `${windowElement.role}:\"${windowElement.label.replace(/"/g, '\\"')}\"`; // Escape quotes
    } else {
      // Fallback, though less ideal - might require more specific criteria
      // Or the backend could return a more specific identifier
      console.warn(`[DesktopUseClient] Found window (role: ${windowElement.role}) has no ID or Label. Creating locator with role only. Consider using more specific criteria in findWindow.`);
      windowSelector = `role:${windowElement.role}`; // Less specific fallback
    }

    console.log(`[DesktopUseClient] Found window, creating locator with selector: ${windowSelector}`);
    // Return a new Locator instance targeting *only* this specific window
    return new Locator(this, [windowSelector]);
  }

  /**
   * Explores the children of the root element (e.g., the main desktop or default window).
   * Provides detailed information about each child, useful for discovering the initial structure.
   * @returns A promise resolving to the exploration results.
   */
  async exploreScreen(): Promise<ExploreResponse> {
    console.log(`[DesktopUseClient] Exploring screen (root element children)`);
    // Send an empty payload or explicitly null selector_chain
    const payload: ExploreRequest = { selector_chain: null };
    return await this._makeRequest<ExploreResponse>("/explore", payload);
  }

  /**
   * Drags the mouse from (startX, startY) to (endX, endY) on the element specified by selectorChain.
   * @param selectorChain - The selector chain identifying the element.
   * @param startX - Starting X coordinate (relative to element or screen).
   * @param startY - Starting Y coordinate.
   * @param endX - Ending X coordinate.
   * @param endY - Ending Y coordinate.
   * @param timeoutMs - Optional timeout in milliseconds.
   * @returns A promise resolving to a basic response on success.
   */
  async mouseDrag(
    selectorChain: string[],
    startX: number,
    startY: number,
    endX: number,
    endY: number,
    timeoutMs?: number | null
  ): Promise<BasicResponse> {
    const payload = {
      selector_chain: selectorChain,
      start_x: startX,
      start_y: startY,
      end_x: endX,
      end_y: endY,
      timeout_ms: timeoutMs,
    };
    return await this._makeRequest<BasicResponse>("/mouse_drag", payload);
  }

  /**
   * Moves mouse to (x, y) and presses down on the element specified by selectorChain.
   */
  async mouseClickAndHold(
    selectorChain: string[],
    x: number,
    y: number,
    timeoutMs?: number | null
  ): Promise<BasicResponse> {
    const payload = {
      selector_chain: selectorChain,
      x,
      y,
      timeout_ms: timeoutMs,
    };
    return await this._makeRequest<BasicResponse>("/mouse_click_and_hold", payload);
  }

  /**
   * Moves mouse to (x, y) on the element specified by selectorChain.
   */
  async mouseMove(
    selectorChain: string[],
    x: number,
    y: number,
    timeoutMs?: number | null
  ): Promise<BasicResponse> {
    const payload = {
      selector_chain: selectorChain,
      x,
      y,
      timeout_ms: timeoutMs,
    };
    return await this._makeRequest<BasicResponse>("/mouse_move", payload);
  }

  /**
   * Releases mouse button on the element specified by selectorChain.
   */
  async mouseRelease(
    selectorChain: string[],
    timeoutMs?: number | null
  ): Promise<BasicResponse> {
    const payload = {
      selector_chain: selectorChain,
      timeout_ms: timeoutMs,
    };
    return await this._makeRequest<BasicResponse>("/mouse_release", payload);
  }
}

export class Locator {
  // Marked internal, though technically public for access
  /** @internal */
  public readonly _client: DesktopUseClient;
  /** @internal */
  public readonly _selector_chain: string[];
  /** @internal */
  private _timeoutMs?: number | null; // Added timeout field

  /** @internal */
  constructor(client: DesktopUseClient, selectorChain: string[], timeoutMs?: number | null) { // Added timeout to constructor
    this._client = client;
    this._selector_chain = selectorChain;
    this._timeoutMs = timeoutMs; // Initialize timeout
  }

  /**
   * Sets a timeout for the *next* action or expectation on this locator.
   * This timeout overrides the default timeout settings for the subsequent operation.
   * @param ms - The timeout duration in milliseconds.
   * @returns A new Locator instance with the specified timeout applied.
   */
  timeout(ms: number): Locator {
    if (typeof ms !== 'number' || ms <= 0) {
      throw new Error("Timeout must be a positive number in milliseconds.");
    }
    // Return a new Locator with the timeout set
    return new Locator(this._client, this._selector_chain, ms);
  }

  /**
   * Creates a new Locator instance scoped to the current locator.
   * Inherits the timeout from the parent locator if set via .timeout().
   * @param selector - The selector string to append to the current chain.
   * @returns A new Locator instance representing the nested element.
   */
  locator(selector: string): Locator {
    if (!selector || typeof selector !== "string") {
      throw new Error("Nested selector must be a non-empty string.");
    }
    const newChain = [...this._selector_chain, selector];
    // Pass the current timeout to the new nested locator
    return new Locator(this._client, newChain, this._timeoutMs);
  }

  // --- Action Methods based on server.rs endpoints --- //

  /**
   * Finds the first element matching the locator chain.
   * Waits for the element to appear if it's not immediately available.
   * Uses the timeout specified by .timeout() if called previously.
   * @returns A promise resolving to the element's basic details.
   */
  async first(): Promise<ElementResponse> {
    const payload: ChainedRequest & { timeout_ms?: number | null } = { // Add timeout_ms to payload type
        selector_chain: this._selector_chain
    };
    if (this._timeoutMs != null) {
        payload.timeout_ms = this._timeoutMs;
    }
    return await this._client._makeRequest<ElementResponse>("/first", payload);
  }

  /**
   * Finds all elements matching the last selector in the chain, within the context
   * established by the preceding selectors.
   * Uses the timeout specified by .timeout() if called previously (applies to finding the parent context).
   * @returns A promise resolving to an array of element details.
   */
  async all(): Promise<ElementsResponse> {
     const payload: ChainedRequest & { timeout_ms?: number | null } = { // Add timeout_ms to payload type
        selector_chain: this._selector_chain
    };
    if (this._timeoutMs != null) {
        payload.timeout_ms = this._timeoutMs;
    }
    return await this._client._makeRequest<ElementsResponse>("/all", payload);
  }

  /**
   * Clicks the element identified by the locator chain.
   * Waits for the element to be actionable.
   * Uses the timeout specified by .timeout() if called previously.
   * @returns A promise resolving to details about the click action.
   */
  async click(): Promise<ClickResponse> {
     const payload: ChainedRequest & { timeout_ms?: number | null } = { // Add timeout_ms to payload type
        selector_chain: this._selector_chain
    };
    if (this._timeoutMs != null) {
        payload.timeout_ms = this._timeoutMs;
    }
    return await this._client._makeRequest<ClickResponse>("/click", payload);
  }

  /**
   * Types the given text into the element identified by the locator chain.
   * Waits for the element to be actionable.
   * Uses the timeout specified by .timeout() if called previously.
   * @param text - The text to type.
   * @returns A promise resolving to a basic response on success.
   */
  async typeText(text: string): Promise<BasicResponse> {
    const payload: TypeTextRequest & { timeout_ms?: number | null } = { // Add timeout_ms to payload type
      selector_chain: this._selector_chain,
      text,
    };
     if (this._timeoutMs != null) {
        payload.timeout_ms = this._timeoutMs;
    }
    return await this._client._makeRequest<BasicResponse>(
      "/type_text",
      payload
    );
  }

  /**
   * Retrieves the text content of the element identified by the locator chain.
   * Waits for the element first.
   * Uses the timeout specified by .timeout() if called previously.
   * @param maxDepth - Optional maximum depth to search for text within child elements (defaults to server-side default, e.g., 5).
   * @returns A promise resolving to the element's text content.
   */
  async getText(maxDepth?: number | null): Promise<TextResponse> {
    const payload: GetTextRequest & { timeout_ms?: number | null } = { // Add timeout_ms to payload type
      selector_chain: this._selector_chain,
      max_depth: maxDepth,
    };
     if (this._timeoutMs != null) {
        payload.timeout_ms = this._timeoutMs;
    }
    return await this._client._makeRequest<TextResponse>("/get_text", payload);
  }

  /**
   * Retrieves the attributes of the element identified by the locator chain.
   * Waits for the element first.
   * Uses the timeout specified by .timeout() if called previously.
   * @returns A promise resolving to the element's attributes.
   */
  async getAttributes(): Promise<AttributesResponse> {
     const payload: ChainedRequest & { timeout_ms?: number | null } = { // Add timeout_ms to payload type
        selector_chain: this._selector_chain
    };
     if (this._timeoutMs != null) {
        payload.timeout_ms = this._timeoutMs;
    }
    return await this._client._makeRequest<AttributesResponse>(
      "/get_attributes",
      payload
    );
  }

  /**
   * Retrieves the bounding rectangle (position and size) of the element identified by the locator chain.
   * Waits for the element first.
   * Uses the timeout specified by .timeout() if called previously.
   * @returns A promise resolving to the element's bounds.
   */
  async getBounds(): Promise<BoundsResponse> {
     const payload: ChainedRequest & { timeout_ms?: number | null } = { // Add timeout_ms to payload type
        selector_chain: this._selector_chain
    };
     if (this._timeoutMs != null) {
        payload.timeout_ms = this._timeoutMs;
    }
    return await this._client._makeRequest<BoundsResponse>(
      "/get_bounds",
      payload
    );
  }

  /**
   * Checks if the element identified by the locator chain is currently visible.
   * Waits for the element first.
   * Uses the timeout specified by .timeout() if called previously.
   * Note: Visibility determination is platform-dependent.
   * @returns A promise resolving to true if the element is visible, false otherwise.
   */
  async isVisible(): Promise<boolean> {
     const payload: ChainedRequest & { timeout_ms?: number | null } = { // Add timeout_ms to payload type
        selector_chain: this._selector_chain
    };
     if (this._timeoutMs != null) {
        payload.timeout_ms = this._timeoutMs;
    }
    // The server returns BooleanResponse, we extract the boolean result here
    const response = await this._client._makeRequest<BooleanResponse>(
      "/is_visible",
      payload
    );
    return response?.result ?? false; // Safely access result, default to false if missing
  }

  /**
   * Sends keyboard key presses to the element identified by the locator chain.
   * Waits for the element to be actionable.
   * Uses the timeout specified by .timeout() if called previously.
   * Use syntax expected by the target platform (e.g., "Enter", "Ctrl+A", "%fx" for Alt+Fx).
   * @param key - The key or key combination to press.
   * @returns A promise resolving to a basic response on success.
   */
  async pressKey(key: string): Promise<BasicResponse> {
    const payload: PressKeyRequest & { timeout_ms?: number | null } = { // Add timeout_ms to payload type
      selector_chain: this._selector_chain,
      key,
    };
     if (this._timeoutMs != null) {
        payload.timeout_ms = this._timeoutMs;
    }
    return await this._client._makeRequest<BasicResponse>(
      "/press_key",
      payload
    );
  }

  /**
   * Activates the application window associated with the element identified by the locator chain.
   * This typically brings the window to the foreground.
   * Waits for the element first.
   * Uses the timeout specified by .timeout() if called previously.
   * @returns The current Locator instance to allow for method chaining.
   * @throws {ApiError} If the server fails to activate the application window.
   */
  async activateApp(): Promise<this> {
     const payload: ChainedRequest & { timeout_ms?: number | null } = { // Add timeout_ms to payload type
        selector_chain: this._selector_chain
    };
     if (this._timeoutMs != null) {
        payload.timeout_ms = this._timeoutMs;
    }
    // Endpoint /activate_app doesn't exist in server.rs, assuming it should be added
    // or maybe it should call activateApplication on the client?
    // For now, assuming an endpoint '/activate_app' needs to be created.
    // If it should activate the app containing the *located element*, the backend needs logic for that.
    console.warn("activateApp() called, but endpoint '/activate_app' needs implementation in server.rs based on the locator chain.");
    await this._client._makeRequest<BasicResponse>("/activate_app", payload); // Placeholder endpoint
    return this;
  }

  // --- Expectation Methods --- //

  /**
   * Waits for the element identified by the locator chain to be visible.
   * Throws an error if the element is not visible within the specified timeout.
   * Uses the timeout specified by .timeout() if called previously, otherwise uses the timeout provided here or the default.
   * @param timeout - Optional timeout in milliseconds to override the one set by .timeout() or the default.
   * @returns A promise resolving to the element's details if it becomes visible.
   */
  async expectVisible(timeout?: number | null): Promise<ElementResponse> {
    const payload: ExpectRequest = {
      selector_chain: this._selector_chain,
      // Prioritize the timeout argument, then the locator's timeout, then null (default)
      timeout_ms: timeout ?? this._timeoutMs ?? null,
    };
    return await this._client._makeRequest<ElementResponse>(
      "/expect_visible",
      payload
    );
  }

  /**
   * Waits for the element identified by the locator chain to be enabled.
   * Throws an error if the element is not enabled within the specified timeout.
   * Uses the timeout specified by .timeout() if called previously, otherwise uses the timeout provided here or the default.
   * @param timeout - Optional timeout in milliseconds to override the one set by .timeout() or the default.
   * @returns A promise resolving to the element's details if it becomes enabled.
   */
  async expectEnabled(timeout?: number | null): Promise<ElementResponse> {
    const payload: ExpectRequest = {
      selector_chain: this._selector_chain,
       // Prioritize the timeout argument, then the locator's timeout, then null (default)
      timeout_ms: timeout ?? this._timeoutMs ?? null,
    };
    return await this._client._makeRequest<ElementResponse>(
      "/expect_enabled",
      payload
    );
  }

  /**
   * Waits for the text content of the element identified by the locator chain
   * to equal the expected text.
   * Throws an error if the text does not match within the specified timeout.
   * Uses the timeout specified by .timeout() if called previously, otherwise uses the timeout provided here or the default.
   * @param expectedText - The exact text string to wait for.
   * @param options - Optional parameters including maxDepth for text retrieval and timeout.
   * @returns A promise resolving to the element's details if the text matches.
   */
  async expectTextEquals(
    expectedText: string,
    options?: { maxDepth?: number | null; timeout?: number | null }
  ): Promise<ElementResponse> {
    const payload: ExpectTextRequest = {
      selector_chain: this._selector_chain,
      expected_text: expectedText,
      max_depth: options?.maxDepth,
       // Prioritize the options.timeout argument, then the locator's timeout, then null (default)
      timeout_ms: options?.timeout ?? this._timeoutMs ?? null,
    };
    return await this._client._makeRequest<ElementResponse>(
      "/expect_text_equals",
      payload
    );
  }

  // --- NEW explore Method ---
  /**
   * Explores the direct children of the element identified by the current locator chain.
   * Provides detailed information about each child, useful for discovering the structure
   * and finding selectors for subsequent actions.
   * Waits for the parent element first.
   * Uses the timeout specified by .timeout() if called previously.
   * @returns A promise resolving to the exploration results, including parent details and a list of detailed children.
   */
  async explore(): Promise<ExploreResponse> {
     const payload: ExploreRequest & { timeout_ms?: number | null } = { // Add timeout_ms to payload type
       selector_chain: this._selector_chain
    };
     if (this._timeoutMs != null) {
        payload.timeout_ms = this._timeoutMs;
    }
    console.log(`[Locator] Exploring element with chain: ${JSON.stringify(this._selector_chain)} and timeout: ${this._timeoutMs}`);
    const response = await this._client._makeRequest<ExploreResponse>("/explore", payload);
    console.log(`[Locator] Exploration found ${response.children.length} children.`);
    return response;
  }

  /**
   * Recursively retrieves all descendant elements of the element identified by the locator chain.
   * Waits for the initial element first.
   * Uses the timeout specified by .timeout() if called previously.
   * @returns A promise resolving to a flat list of all descendant element details.
   */
  async getFullTree(): Promise<ElementsResponse> {
    const payload: ChainedRequest & { timeout_ms?: number | null } = { // Add timeout_ms to payload type
      selector_chain: this._selector_chain
    };
    if (this._timeoutMs != null) {
      payload.timeout_ms = this._timeoutMs;
    }
    console.log(`[Locator] Getting full tree for element with chain: ${JSON.stringify(this._selector_chain)} and timeout: ${this._timeoutMs}`);
    const response = await this._client._makeRequest<ElementsResponse>("/get_full_tree", payload);
    console.log(`[Locator] Full tree retrieval found ${response.elements.length} descendant elements.`);
    return response;
  }

  /**
   * Drags the mouse from (startX, startY) to (endX, endY) relative to the element.
   * @param startX - Starting X coordinate (relative to element or screen).
   * @param startY - Starting Y coordinate.
   * @param endX - Ending X coordinate.
   * @param endY - Ending Y coordinate.
   * @returns A promise resolving to a basic response on success.
   */
  async mouseDrag(
    startX: number,
    startY: number,
    endX: number,
    endY: number
  ): Promise<BasicResponse> {
    return await this._client.mouseDrag(
      this._selector_chain,
      startX,
      startY,
      endX,
      endY,
      this._timeoutMs
    );
  }

  /**
   * Moves mouse to (x, y) and presses down on the element specified by selectorChain.
   */
  async clickAndHold(x: number, y: number): Promise<BasicResponse> {
    return await this._client.mouseClickAndHold(this._selector_chain, x, y, this._timeoutMs);
  }

  /**
   * Moves mouse to (x, y) on the element specified by selectorChain.
   */
  async mouseMove(x: number, y: number): Promise<BasicResponse> {
    return await this._client.mouseMove(this._selector_chain, x, y, this._timeoutMs);
  }

  /**
   * Releases mouse button on the element specified by selectorChain.
   */
  async releaseMouse(): Promise<BasicResponse> {
    return await this._client.mouseRelease(this._selector_chain, this._timeoutMs);
  }
}

// Utility function (optional, could be part of a separate utils file)
export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
