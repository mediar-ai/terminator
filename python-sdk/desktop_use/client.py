"""Main client and locator logic for the Terminator SDK."""

import requests
import json
import logging
import time
from typing import List, Optional, Dict, Any, Type, TypeVar, Tuple
from dataclasses import asdict, dataclass, field

from .exceptions import ApiError, ConnectionError
from .models import (
    BasicResponse, BooleanResponse, TextResponse, ElementResponse, ElementsResponse,
    ClickResponse, AttributesResponse, BoundsResponse, ChainedRequest,
    TypeTextRequest, GetTextRequest, PressKeyRequest, OpenApplicationRequest,
    OpenUrlRequest, ExpectRequest, ExpectTextRequest,
    OpenFileRequest, RunCommandRequest, CaptureMonitorRequest, OcrImagePathRequest,
    CommandOutputResponse, ScreenshotResponse, OcrResponse, OcrScreenshotRequest,
    FindWindowRequest, ExploreRequest, ExploredElementDetail, ExploreResponse,
    ActivateApplicationRequest
)

logger = logging.getLogger(__name__)

DEFAULT_BASE_URL = "http://127.0.0.1:9375"

# Generic type variable for response models
T = TypeVar('T')

# Add this dataclass
@dataclass
class ActivateBrowserWindowRequest:
    title: str

# Make sure ChainedRequest and others used by Locator have timeout_ms
@dataclass
class ChainedRequestWithTimeout(ChainedRequest):
    timeout_ms: Optional[int] = None

@dataclass
class TypeTextRequestWithTimeout(TypeTextRequest):
    timeout_ms: Optional[int] = None

@dataclass
class GetTextRequestWithTimeout(GetTextRequest):
    timeout_ms: Optional[int] = None

@dataclass
class PressKeyRequestWithTimeout(PressKeyRequest):
    timeout_ms: Optional[int] = None

# ExpectRequest already has timeout_ms, ensure it's used correctly
# ExpectTextRequest already has timeout_ms, ensure it's used correctly

@dataclass
class ExploreRequestWithTimeout(ExploreRequest): # Assuming ExploreRequest is defined in models.py
    timeout_ms: Optional[int] = None

class DesktopUseClient:
    """Client for interacting with the Terminator desktop automation server."""

    def __init__(self, base_url: str = DEFAULT_BASE_URL):
        """
        Initializes the client.

        Args:
            base_url: The base URL of the Terminator server.
        """
        self.base_url = base_url.rstrip('/')
        self.session = requests.Session()
        self.session.headers.update({"Content-Type": "application/json"})
        logger.info(f"DesktopUseClient initialized for server at {self.base_url}")

    def _make_request(self, endpoint: str, payload: Any, response_model: Type[T]) -> T:
        """Internal helper to make POST requests and handle responses."""
        url = f"{self.base_url}/{endpoint.lstrip('/')}"
        # Convert dataclass payload to dict, handle non-dataclass payloads
        if hasattr(payload, '__dataclass_fields__'):
             # Filter out None values during conversion
             payload_dict = asdict(payload, dict_factory=lambda x: {k: v for (k, v) in x if v is not None})
        elif isinstance(payload, dict):
            payload_dict = payload # Assume dict is already prepared
        else:
            # Allow empty dict for requests like capture_screen
            if payload is None or payload == {}:
                 payload_dict = {}
            else:
                raise TypeError(f"Payload must be a dataclass instance or a dictionary, got {type(payload)}")

        json_payload = json.dumps(payload_dict) if payload_dict else "{}"
        logger.debug(f"Sending POST to {url} with payload: {json_payload}")

        try:
            response = self.session.post(url, data=json_payload)
            logger.debug(f"Response Status: {response.status_code}")
            # Only log first 500 chars to avoid large image data flooding logs
            log_data = response.text[:500] + "..." if len(response.text) > 500 else response.text
            logger.debug(f"Response Data: {log_data}")

            if 200 <= response.status_code < 300:
                try:
                    if not response.content or response.status_code == 204:
                        # Handle No Content or empty successful response
                        if response_model is BasicResponse or response_model is object: # Treat object as BasicResponse placeholder
                            return BasicResponse(message="Operation successful") # type: ignore
                        try:
                             # Try creating a default instance if possible
                             return response_model()
                        except TypeError:
                             # Fallback if default construction fails
                             logger.warning(f"Received empty success response for {response_model.__name__}, cannot create default instance. Returning BasicResponse.")
                             return BasicResponse(message="Operation successful (empty response)") # type: ignore

                    data = response.json()
                    # Check if response model expects specific fields or is generic
                    if response_model is BasicResponse and 'message' not in data:
                        # If server sends {} for success, create a basic message
                        return BasicResponse(message="Operation successful (no message field)") # type: ignore

                    # Use dictionary unpacking to create the dataclass instance
                    return response_model(**data)
                except (json.JSONDecodeError, TypeError) as e:
                    logger.error(f"Failed to decode JSON or create response model {response_model.__name__} from data: {response.text}. Error: {e}", exc_info=True)
                    raise ApiError(f"Invalid JSON response or model mismatch: {e}", response.status_code)
                except Exception as e: # Catch broader errors during model instantiation
                    logger.error(f"Failed to instantiate response model {response_model.__name__} from data: {response.text}. Error: {e}", exc_info=True)
                    raise ApiError(f"Error creating response model: {e}", response.status_code)
            else:
                # Attempt to parse error message from server
                try:
                    error_data = response.json()
                    error_message = error_data.get('message', response.text)
                except json.JSONDecodeError:
                    error_message = response.text
                logger.error(f"API Error ({response.status_code}): {error_message}")
                raise ApiError(error_message, response.status_code)

        except requests.exceptions.ConnectionError as e:
            logger.error(f"Connection Error connecting to {url}: {e}", exc_info=True)
            raise ConnectionError(f"Could not connect to Terminator server at {url}. Is it running? Details: {e}")
        except requests.exceptions.RequestException as e:
            logger.error(f"Request failed: {e}", exc_info=True)
            raise ApiError(f"An unexpected request error occurred: {e}")

    def locator(self, selector: str) -> 'Locator':
        """
        Creates a new Locator instance starting from the root.

        Args:
            selector: The initial selector string (e.g., 'window:"My App"', 'id:someId').

        Returns:
            A new Locator instance.
        """
        if not isinstance(selector, str) or not selector:
            raise ValueError("Initial selector must be a non-empty string.")
        return Locator(self, [selector])

    def open_application(self, app_name: str) -> BasicResponse:
        """
        Opens an application by its name or path.

        Args:
            app_name: The name or path of the application.

        Returns:
            BasicResponse indicating success.
        """
        payload = OpenApplicationRequest(app_name=app_name)
        return self._make_request("/open_application", payload, BasicResponse)

    def activate_application(self, app_name: str) -> BasicResponse:
        """
        Activates an application by its name or path, bringing its window to the foreground.

        Args:
            app_name: The name or path of the application to activate.

        Returns:
            BasicResponse indicating success.
        """
        payload = ActivateApplicationRequest(app_name=app_name)
        return self._make_request("/activate_application", payload, BasicResponse)

    def open_url(self, url: str, browser: Optional[str] = None) -> BasicResponse:
        """
        Opens a URL, optionally in a specific browser.

        Args:
            url: The URL to open.
            browser: Optional browser name/path.

        Returns:
            BasicResponse indicating success.
        """
        payload = OpenUrlRequest(url=url, browser=browser)
        return self._make_request("/open_url", payload, BasicResponse)

    # --- New Top-Level Methods --- #

    def open_file(self, file_path: str) -> BasicResponse:
        """
        Opens a file using its default application.

        Args:
            file_path: The path to the file.

        Returns:
            BasicResponse indicating success.
        """
        payload = OpenFileRequest(file_path=file_path)
        return self._make_request("/open_file", payload, BasicResponse)

    def run_command(self, windows_command: Optional[str] = None, unix_command: Optional[str] = None) -> CommandOutputResponse:
        """
        Executes a command, choosing the appropriate one based on the server's OS.

        Provide at least one of windows_command or unix_command.

        Args:
            windows_command: The command to run on Windows.
            unix_command: The command to run on Unix-like systems (Linux, macOS).

        Returns:
            CommandOutputResponse containing stdout, stderr, and exit code.
        """
        if not windows_command and not unix_command:
            raise ValueError("At least one of windows_command or unix_command must be provided.")
        payload = RunCommandRequest(windows_command=windows_command, unix_command=unix_command)
        return self._make_request("/run_command", payload, CommandOutputResponse)

    def capture_screen(self) -> ScreenshotResponse:
        """
        Captures a screenshot of the primary monitor.

        Returns:
            ScreenshotResponse containing base64 encoded image data, width, and height.
        """
        # No payload needed for capture_screen
        return self._make_request("/capture_screen", {}, ScreenshotResponse)

    def capture_monitor_by_name(self, monitor_name: str) -> ScreenshotResponse:
        """
        Captures a screenshot of a specific monitor by its name.

        Args:
            monitor_name: The name of the monitor (platform-specific).

        Returns:
            ScreenshotResponse containing base64 encoded image data, width, and height.
        """
        payload = CaptureMonitorRequest(monitor_name=monitor_name)
        return self._make_request("/capture_monitor", payload, ScreenshotResponse)

    def ocr_image_path(self, image_path: str) -> OcrResponse:
        """
        Performs OCR on an image file located at the given path.
        The server needs access to this path.

        Args:
            image_path: The path to the image file.

        Returns:
            OcrResponse containing the extracted text.
        """
        payload = OcrImagePathRequest(image_path=image_path)
        return self._make_request("/ocr_image_path", payload, OcrResponse)

    def ocr_screenshot(self, image_base64: str, width: int, height: int) -> OcrResponse:
        """
        Performs OCR directly on raw image data (e.g., from a previous screenshot).

        Args:
            image_base64: The base64 encoded string of the image data.
            width: The width of the image in pixels.
            height: The height of the image in pixels.

        Returns:
            OcrResponse containing the extracted text.
        """
        payload = OcrScreenshotRequest(
            image_base64=image_base64,
            width=width,
            height=height
        )
        return self._make_request("/ocr_screenshot", payload, OcrResponse)

    def activate_browser_window_by_title(self, title: str) -> BasicResponse:
        """
        Activates a browser window if it contains an element (like a tab) with the specified title.

        This brings the browser window to the foreground.
        Note: This does not guarantee the specific tab will be active within the window.

        Args:
            title: The title (or partial title) of the tab/element to search for within browser windows.

        Returns:
            BasicResponse indicating success.
        """
        payload = ActivateBrowserWindowRequest(title=title)
        return self._make_request("/activate_browser_window", payload, BasicResponse)

    # --- New Discovery/Setup Methods ---

    def find_window(self, title_contains: Optional[str] = None, timeout_ms: Optional[int] = None) -> 'Locator':
        """
        Finds a top-level window based on criteria (e.g., title).

        Args:
            title_contains: Substring to match in the window title.
            timeout_ms: Optional timeout in milliseconds to wait for the window.

        Returns:
            A Locator instance scoped to the found window.
        Raises:
            ApiError: If no matching window is found within the timeout.
            ValueError: If no criteria are provided.
        """
        if not title_contains: # Add more criteria checks (processName etc.) if implemented
             raise ValueError("At least one criterion (e.g., title_contains) must be provided to find_window.")

        # Ensure FindWindowRequest is defined in models.py
        payload = FindWindowRequest(title_contains=title_contains, timeout_ms=timeout_ms)
        window_element = self._make_request("/find_window", payload, ElementResponse)

        # Construct a reasonably stable selector for the found window
        window_selector: str
        if window_element.id:
            # Using ID is preferred if available and stable
            window_selector = f"id:{window_element.id}"
        elif window_element.label:
            # Fallback to Role + Name (Label)
            escaped_label = window_element.label.replace('"', '\\"') # Basic escape for quotes
            window_selector = f'{window_element.role}:"{escaped_label}"'
        else:
            # Fallback to Role only - might be ambiguous
            logger.warning(f"Found window (role: {window_element.role}) has no ID or Label. Creating locator with role only. Consider using more specific criteria or ensure server provides better identifiers.")
            window_selector = f"role:{window_element.role}"

        logger.info(f"Found window, creating locator with selector: {window_selector}")
        # Return a new Locator scoped *only* to this window
        return Locator(self, [window_selector])

    def explore(self) -> ExploreResponse:
        """
        Explores the children of the root element (e.g., desktop). Best to be used by an AI agent.

        Returns:
            ExploreResponse containing details of the root's direct children.
        """
        logger.info("Exploring screen (root element children)")
        # Ensure ExploreRequest is defined in models.py
        # Pass empty selector_chain (or null/None based on server expectation)
        payload = ExploreRequest(selector_chain=None)
        return self._make_request("/explore", payload, ExploreResponse)

    def mouse_drag(self, selector_chain, start_x, start_y, end_x, end_y, timeout_ms=None):
        """Drags the mouse from (start_x, start_y) to (end_x, end_y) on the element specified by selector_chain."""
        payload = {
            "selector_chain": selector_chain,
            "start_x": start_x,
            "start_y": start_y,
            "end_x": end_x,
            "end_y": end_y,
            "timeout_ms": timeout_ms,
        }
        return self._make_request("/mouse_drag", payload, BasicResponse)

    def mouse_click_and_hold(self, selector_chain, x, y, timeout_ms=None):
        """Moves mouse to (x, y) and presses down on the element specified by selector_chain."""
        payload = {
            "selector_chain": selector_chain,
            "x": x,
            "y": y,
            "timeout_ms": timeout_ms,
        }
        return self._make_request("/mouse_click_and_hold", payload, BasicResponse)

    def mouse_move(self, selector_chain, x, y, timeout_ms=None):
        """Moves mouse to (x, y) on the element specified by selector_chain."""
        payload = {
            "selector_chain": selector_chain,
            "x": x,
            "y": y,
            "timeout_ms": timeout_ms,
        }
        return self._make_request("/mouse_move", payload, BasicResponse)

    def mouse_release(self, selector_chain, timeout_ms=None):
        """Releases mouse button on the element specified by selector_chain."""
        payload = {
            "selector_chain": selector_chain,
            "timeout_ms": timeout_ms,
        }
        return self._make_request("/mouse_release", payload, BasicResponse)

class Locator:
    """
    Represents a UI element locator, allowing chained selections and actions.
    Locators are immutable; methods like `locator()` or `timeout()` return new instances.
    """

    def __init__(self, client: DesktopUseClient, selector_chain: List[str], timeout_ms: Optional[int] = None):
        """Internal constructor. Use client.locator() or locator.locator()."""
        self._client = client
        self._selector_chain = selector_chain
        self._timeout_ms = timeout_ms # Default timeout for actions on this locator

    def timeout(self, ms: int) -> 'Locator':
        """
        Sets a timeout for subsequent actions or expectations on this locator chain.
        This timeout overrides the default timeout for the *next* operation initiated
        from the returned Locator instance.

        Args:
            ms: The timeout duration in milliseconds. Must be positive.

        Returns:
            A new Locator instance with the specified timeout set.
        """
        if not isinstance(ms, int) or ms <= 0:
            raise ValueError("Timeout must be a positive integer in milliseconds.")
        # Return a new instance with the timeout applied
        return Locator(self._client, self._selector_chain, ms)

    def locator(self, selector: str) -> 'Locator':
        """
        Creates a new Locator scoped to the current one by appending a selector.
        Inherits the timeout from the parent locator.

        Args:
            selector: The selector to append to the chain (e.g., 'button:"OK"', 'id:loginButton').

        Returns:
            A new Locator instance representing the nested element.
        """
        if not isinstance(selector, str) or not selector:
            raise ValueError("Nested selector must be a non-empty string.")
        new_chain = self._selector_chain + [selector]
        # Inherit the timeout from the current locator
        return Locator(self._client, new_chain, self._timeout_ms)

    # --- Action Methods --- #

    def first(self) -> ElementResponse:
        """
        Finds the first element matching the locator chain.
        Waits up to the configured timeout (`locator.timeout()`).
        """
        # Use ChainedRequestWithTimeout or similar that includes timeout_ms
        payload = ChainedRequestWithTimeout(
            selector_chain=self._selector_chain,
            timeout_ms=self._timeout_ms
        )
        return self._client._make_request("/first", payload, ElementResponse)

    def all(self) -> ElementsResponse:
        """
        Finds all elements matching the last selector in the chain, within the context
        of the preceding selectors. Waits up to the configured timeout for the context.
        """
        # Use ChainedRequestWithTimeout or similar
        payload = ChainedRequestWithTimeout(
            selector_chain=self._selector_chain,
            timeout_ms=self._timeout_ms
        )
        return self._client._make_request("/all", payload, ElementsResponse)

    def click(self) -> ClickResponse:
        """
        Clicks the element. Waits for the element to be actionable up to the configured timeout.
        """
         # Use ChainedRequestWithTimeout or similar
        payload = ChainedRequestWithTimeout(
            selector_chain=self._selector_chain,
            timeout_ms=self._timeout_ms
        )
        return self._client._make_request("/click", payload, ClickResponse)

    def type_text(self, text: str) -> BasicResponse:
        """
        Types text into the element. Waits for the element up to the configured timeout.
        """
        # Use TypeTextRequestWithTimeout or similar
        payload = TypeTextRequestWithTimeout(
            selector_chain=self._selector_chain,
            text=text,
            timeout_ms=self._timeout_ms
        )
        return self._client._make_request("/type_text", payload, BasicResponse)

    def get_text(self, max_depth: Optional[int] = None) -> TextResponse:
        """
        Gets the text content. Waits for the element up to the configured timeout.
        """
         # Use GetTextRequestWithTimeout or similar
        payload = GetTextRequestWithTimeout(
            selector_chain=self._selector_chain,
            max_depth=max_depth,
            timeout_ms=self._timeout_ms
        )
        return self._client._make_request("/get_text", payload, TextResponse)

    def get_attributes(self) -> AttributesResponse:
        """
        Gets element attributes. Waits for the element up to the configured timeout.
        """
         # Use ChainedRequestWithTimeout or similar
        payload = ChainedRequestWithTimeout(
            selector_chain=self._selector_chain,
            timeout_ms=self._timeout_ms
        )
        return self._client._make_request("/get_attributes", payload, AttributesResponse)

    def get_bounds(self) -> BoundsResponse:
        """
        Gets the element's bounding rectangle. Waits up to the configured timeout.
        """
        # Use ChainedRequestWithTimeout or similar
        payload = ChainedRequestWithTimeout(
            selector_chain=self._selector_chain,
            timeout_ms=self._timeout_ms
        )
        return self._client._make_request("/get_bounds", payload, BoundsResponse)

    def is_visible(self) -> bool:
        """
        Checks if the element is visible. Waits up to the configured timeout.
        """
         # Use ChainedRequestWithTimeout or similar
        payload = ChainedRequestWithTimeout(
            selector_chain=self._selector_chain,
            timeout_ms=self._timeout_ms
        )
        response = self._client._make_request("/is_visible", payload, BooleanResponse)
        return response.result if response else False

    def press_key(self, key: str) -> BasicResponse:
        """
        Sends key presses to the element. Waits up to the configured timeout.
        """
        # Use PressKeyRequestWithTimeout or similar
        payload = PressKeyRequestWithTimeout(
            selector_chain=self._selector_chain,
            key=key,
            timeout_ms=self._timeout_ms
        )
        return self._client._make_request("/press_key", payload, BasicResponse)

    # --- Expectation Methods --- #

    def expect_visible(self, timeout: Optional[int] = None) -> ElementResponse:
        """
        Waits for the element to be visible.

        Args:
            timeout: Optional timeout in milliseconds, overrides locator's timeout.

        Returns:
            ElementResponse if visible within timeout.
        Raises:
            ApiError: If timeout occurs or another error happens.
        """
        # Prioritize explicit timeout > locator timeout > server default
        effective_timeout = timeout if timeout is not None else self._timeout_ms
        # ExpectRequest should already have timeout_ms field in models.py
        payload = ExpectRequest(
            selector_chain=self._selector_chain,
            timeout_ms=effective_timeout
        )
        return self._client._make_request("/expect_visible", payload, ElementResponse)

    def expect_enabled(self, timeout: Optional[int] = None) -> ElementResponse:
        """
        Waits for the element to be enabled.

        Args:
            timeout: Optional timeout in milliseconds, overrides locator's timeout.

        Returns:
            ElementResponse if enabled within timeout.
        Raises:
            ApiError: If timeout occurs or another error happens.
        """
        effective_timeout = timeout if timeout is not None else self._timeout_ms
        payload = ExpectRequest(
            selector_chain=self._selector_chain,
            timeout_ms=effective_timeout
        )
        return self._client._make_request("/expect_enabled", payload, ElementResponse)

    def expect_text_equals(
        self,
        expected_text: str,
        max_depth: Optional[int] = None,
        timeout: Optional[int] = None
    ) -> ElementResponse:
        """
        Waits for the element's text to equal the expected value.

        Args:
            expected_text: The exact text to match.
            max_depth: Optional depth for text retrieval.
            timeout: Optional timeout in milliseconds, overrides locator's timeout.

        Returns:
            ElementResponse if text matches within timeout.
        Raises:
            ApiError: If timeout occurs, text doesn't match, or another error happens.
        """
        effective_timeout = timeout if timeout is not None else self._timeout_ms
        # ExpectTextRequest should already have timeout_ms field in models.py
        payload = ExpectTextRequest(
            selector_chain=self._selector_chain,
            expected_text=expected_text,
            max_depth=max_depth,
            timeout_ms=effective_timeout
        )
        return self._client._make_request("/expect_text_equals", payload, ElementResponse)

    # --- NEW explore Method ---
    def explore(self) -> ExploreResponse:
        """
        Explores the direct children of the element identified by this locator.
        Waits for the parent element up to the configured timeout.
        Best to be used by an AI agent.

        Returns:
            ExploreResponse containing parent details and list of children details.
        """
        logger.info(f"Exploring element with chain: {self._selector_chain} and timeout: {self._timeout_ms}")
        # Use ExploreRequestWithTimeout or ensure ExploreRequest has timeout_ms
        payload = ExploreRequestWithTimeout(
            selector_chain=self._selector_chain,
            timeout_ms=self._timeout_ms
        )
        response = self._client._make_request("/explore", payload, ExploreResponse)
        logger.info(f"Exploration found {len(response.children) if response and response.children else 0} children.")
        return response

    def mouse_drag(self, start_x, start_y, end_x, end_y):
        """Drags the mouse from (start_x, start_y) to (end_x, end_y) relative to the element."""
        return self._client.mouse_drag(self._selector_chain, start_x, start_y, end_x, end_y, self._timeout_ms)

    def mouse_click_and_hold(self, x, y):
        """Moves mouse to (x, y) and presses down relative to the element."""
        return self._client.mouse_click_and_hold(self._selector_chain, x, y, self._timeout_ms)

    def mouse_move(self, x, y):
        """Moves mouse to (x, y) relative to the element."""
        return self._client.mouse_move(self._selector_chain, x, y, self._timeout_ms)

    def mouse_release(self):
        """Releases mouse button relative to the element."""
        return self._client.mouse_release(self._selector_chain, self._timeout_ms)

# Helper function (can be part of the SDK or used externally)
def sleep(seconds: float) -> None:
    """Pauses execution for a specified number of seconds."""
    if seconds < 0:
        raise ValueError("Sleep duration must be non-negative.")
    time.sleep(seconds) 