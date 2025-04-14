const DEFAULT_BASE_URL = "http://127.0.0.1:3000";

// --- Interfaces for API Payloads and Responses matching server.rs --- //

// General Responses
export interface BasicResponse {
  message: string;
}

export interface BooleanResponse {
  result: boolean;
}

export interface TextResponse {
  text: string;
}

// Element Related Responses
export interface ElementResponse {
  role: string;
  label?: string | null;
  id?: string | null;
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

interface OpenUrlRequest {
  url: string;
  browser?: string | null;
}

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
              "Invalid JSON response from server",
              response.status
            );
          }
          // If it was 204, we already returned success, this catch shouldn't be hit for JSON error
          // If somehow it's 204 and parsing empty string fails, return success anyway
          return { message: "Operation successful (204)" } as T;
        }
      } else {
        // Handle HTTP errors (status code >= 300)
        let errorMessage = `Server returned status ${response.status}`;
        try {
          // Attempt to parse error response for more detail
          const errorData = JSON.parse(responseText);
          errorMessage = errorData?.message || errorMessage;
        } catch (e) {
          // Ignore JSON parse error, use the raw responseText as detail
        }
        console.error(
          `[DesktopUseClient] Error: ${errorMessage}, Detail: ${responseText}`
        );
        throw new ApiError(errorMessage, response.status);
      }
    } catch (error) {
      // Handle network errors or errors thrown above
      console.error(`[DesktopUseClient] Request error: ${error}`);
      if (error instanceof ApiError) {
        // Re-throw ApiError instances directly
        throw error;
      } else if (
        error instanceof TypeError &&
        error.message.includes("fetch")
      ) {
        // Network errors often manifest as TypeErrors in fetch
        // Try to provide a more specific message for connection refused
        // Note: Differentiating specific network errors (like ECONNREFUSED) is harder with fetch than 'http'
        throw new ApiError(
          `Network error connecting to ${url}. Is the server running? (${error.message})`
        );
      } else {
        // Wrap other errors
        throw new ApiError(
          `Request failed: ${
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
   * Opens a URL, optionally specifying the browser.
   * @param url - The URL to open.
   * @param browser - Optional name or path of the browser to use. Uses system default if null/undefined.
   * @returns A promise resolving to a basic response on success.
   */
  async openUrl(url: string, browser?: string | null): Promise<BasicResponse> {
    const payload: OpenUrlRequest = { url, browser };
    return await this._makeRequest<BasicResponse>("/open_url", payload);
  }
}

export class Locator {
  // Marked internal, though technically public for access
  /** @internal */
  public readonly _client: DesktopUseClient;
  /** @internal */
  public readonly _selector_chain: string[];

  /** @internal */
  constructor(client: DesktopUseClient, selectorChain: string[]) {
    this._client = client;
    this._selector_chain = selectorChain;
  }

  /**
   * Creates a new Locator instance scoped to the current locator.
   * @param selector - The selector string to append to the current chain.
   * @returns A new Locator instance representing the nested element.
   */
  locator(selector: string): Locator {
    if (!selector || typeof selector !== "string") {
      throw new Error("Nested selector must be a non-empty string.");
    }
    const newChain = [...this._selector_chain, selector];
    return new Locator(this._client, newChain);
  }

  // --- Action Methods based on server.rs endpoints --- //

  /**
   * Finds the first element matching the locator chain.
   * Waits for the element to appear if it's not immediately available (handled server-side).
   * @returns A promise resolving to the element's basic details.
   */
  async first(): Promise<ElementResponse> {
    const payload: ChainedRequest = { selector_chain: this._selector_chain };
    return await this._client._makeRequest<ElementResponse>("/first", payload);
  }

  /**
   * Finds all elements matching the last selector in the chain, within the context
   * established by the preceding selectors.
   * @returns A promise resolving to an array of element details.
   */
  async all(): Promise<ElementsResponse> {
    const payload: ChainedRequest = { selector_chain: this._selector_chain };
    return await this._client._makeRequest<ElementsResponse>("/all", payload);
  }

  /**
   * Clicks the element identified by the locator chain.
   * Waits for the element to be actionable (handled server-side).
   * @returns A promise resolving to details about the click action.
   */
  async click(): Promise<ClickResponse> {
    const payload: ChainedRequest = { selector_chain: this._selector_chain };
    return await this._client._makeRequest<ClickResponse>("/click", payload);
  }

  /**
   * Types the given text into the element identified by the locator chain.
   * Waits for the element to be actionable (handled server-side).
   * @param text - The text to type.
   * @returns A promise resolving to a basic response on success.
   */
  async typeText(text: string): Promise<BasicResponse> {
    const payload: TypeTextRequest = {
      selector_chain: this._selector_chain,
      text,
    };
    return await this._client._makeRequest<BasicResponse>(
      "/type_text",
      payload
    );
  }

  /**
   * Retrieves the text content of the element identified by the locator chain.
   * @param maxDepth - Optional maximum depth to search for text within child elements (defaults to server-side default, e.g., 5).
   * @returns A promise resolving to the element's text content.
   */
  async getText(maxDepth?: number | null): Promise<TextResponse> {
    const payload: GetTextRequest = {
      selector_chain: this._selector_chain,
      max_depth: maxDepth,
    };
    return await this._client._makeRequest<TextResponse>("/get_text", payload);
  }

  /**
   * Retrieves the attributes of the element identified by the locator chain.
   * @returns A promise resolving to the element's attributes.
   */
  async getAttributes(): Promise<AttributesResponse> {
    const payload: ChainedRequest = { selector_chain: this._selector_chain };
    return await this._client._makeRequest<AttributesResponse>(
      "/get_attributes",
      payload
    );
  }

  /**
   * Retrieves the bounding rectangle (position and size) of the element identified by the locator chain.
   * @returns A promise resolving to the element's bounds.
   */
  async getBounds(): Promise<BoundsResponse> {
    const payload: ChainedRequest = { selector_chain: this._selector_chain };
    return await this._client._makeRequest<BoundsResponse>(
      "/get_bounds",
      payload
    );
  }

  /**
   * Checks if the element identified by the locator chain is currently visible.
   * Note: Visibility determination is platform-dependent.
   * @returns A promise resolving to true if the element is visible, false otherwise.
   */
  async isVisible(): Promise<boolean> {
    const payload: ChainedRequest = { selector_chain: this._selector_chain };
    // The server returns BooleanResponse, we extract the boolean result here
    const response = await this._client._makeRequest<BooleanResponse>(
      "/is_visible",
      payload
    );
    return response?.result ?? false; // Safely access result, default to false if missing
  }

  /**
   * Sends keyboard key presses to the element identified by the locator chain.
   * Use syntax expected by the target platform (e.g., "Enter", "Ctrl+A", "%fx" for Alt+Fx).
   * @param key - The key or key combination to press.
   * @returns A promise resolving to a basic response on success.
   */
  async pressKey(key: string): Promise<BasicResponse> {
    const payload: PressKeyRequest = {
      selector_chain: this._selector_chain,
      key,
    };
    return await this._client._makeRequest<BasicResponse>(
      "/press_key",
      payload
    );
  }

  /**
   * Activates the application window associated with the element identified by the locator chain.
   * This typically brings the window to the foreground.
   * Waits for the element first (handled server-side).
   * @returns The current Locator instance to allow for method chaining.
   * @throws {ApiError} If the server fails to activate the application window.
   */
  async activateApp(): Promise<this> {
    const payload: ChainedRequest = { selector_chain: this._selector_chain };
    await this._client._makeRequest<BasicResponse>("/activate_app", payload);
    return this;
  }

  // --- Expectation Methods --- //

  /**
   * Waits for the element identified by the locator chain to be visible.
   * Throws an error if the element is not visible within the specified timeout.
   * @param timeout - Optional timeout in milliseconds to override the default.
   * @returns A promise resolving to the element's details if it becomes visible.
   */
  async expectVisible(timeout?: number | null): Promise<ElementResponse> {
    const payload: ExpectRequest = {
      selector_chain: this._selector_chain,
      timeout_ms: timeout,
    };
    return await this._client._makeRequest<ElementResponse>(
      "/expect_visible",
      payload
    );
  }

  /**
   * Waits for the element identified by the locator chain to be enabled.
   * Throws an error if the element is not enabled within the specified timeout.
   * @param timeout - Optional timeout in milliseconds to override the default.
   * @returns A promise resolving to the element's details if it becomes enabled.
   */
  async expectEnabled(timeout?: number | null): Promise<ElementResponse> {
    const payload: ExpectRequest = {
      selector_chain: this._selector_chain,
      timeout_ms: timeout,
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
      timeout_ms: options?.timeout,
    };
    return await this._client._makeRequest<ElementResponse>(
      "/expect_text_equals",
      payload
    );
  }
}

// Utility function (optional, could be part of a separate utils file)
export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
