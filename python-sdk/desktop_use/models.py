"""Dataclasses for API request/response models."""

from dataclasses import dataclass, field
from typing import Optional, List, Tuple, Dict, Any

# --- General Responses --- #
@dataclass
class BasicResponse:
    message: str

@dataclass
class BooleanResponse:
    result: bool

@dataclass
class TextResponse:
    text: str

# --- Element Related Responses --- #
@dataclass
class ElementResponse:
    role: str
    label: Optional[str] = None
    id: Optional[str] = None
    text: Optional[str] = None
    bounds: Optional[Tuple[float, float, float, float]] = None
    visible: Optional[bool] = None
    enabled: Optional[bool] = None
    focused: Optional[bool] = None

@dataclass
class ElementsResponse:
    elements: List[ElementResponse]

@dataclass
class ClickResponse:
    method: Optional[str] = None
    details: Optional[str] = None
    coordinates: Optional[Tuple[float, float]] = None
    message: Optional[str] = None

@dataclass
class AttributesResponse:
    role: str
    properties: Dict[str, Optional[Any]] # Corresponds to HashMap<String, Option<serde_json::Value>>
    label: Optional[str] = None
    value: Optional[str] = None
    description: Optional[str] = None
    id: Optional[str] = None
    is_keyboard_focusable: Optional[bool] = None

@dataclass
class BoundsResponse:
    x: float
    y: float
    width: float
    height: float

# --- Base Request Structures --- #
@dataclass
class ChainedRequest:
    selector_chain: List[str]

# --- Specific Action Requests --- #
@dataclass
class TypeTextRequest(ChainedRequest):
    text: str

@dataclass
class GetTextRequest(ChainedRequest):
    max_depth: Optional[int] = None

@dataclass
class PressKeyRequest(ChainedRequest):
    key: str

# --- App/URL Requests --- #
@dataclass
class OpenApplicationRequest:
    app_name: str

@dataclass
class OpenUrlRequest:
    url: str
    browser: Optional[str] = None

@dataclass
class OpenFileRequest:
    file_path: str

@dataclass
class RunCommandRequest:
    windows_command: Optional[str] = None
    unix_command: Optional[str] = None

@dataclass
class CaptureMonitorRequest:
    monitor_name: str

@dataclass
class OcrImagePathRequest:
    image_path: str

@dataclass
class OcrScreenshotRequest:
    image_base64: str
    width: int
    height: int

# --- Expectation Requests --- #
@dataclass
class ExpectRequest(ChainedRequest):
    timeout_ms: Optional[int] = None

@dataclass
class ExpectTextRequest(ChainedRequest): # Inherit directly from ChainedRequest
    expected_text: str             # Non-default field
    timeout_ms: Optional[int] = None # Default field
    max_depth: Optional[int] = None  # Default field

# --- Response Types --- #
@dataclass
class CommandOutputResponse:
    stdout: str
    stderr: str
    exit_code: Optional[int] # Match server response (was exit_status in core)

@dataclass
class ScreenshotResponse:
    image_base64: str # Base64 encoded image data
    width: int
    height: int

@dataclass
class OcrResponse:
    text: str

@dataclass
class FindWindowRequest:
    title_contains: Optional[str] = None
    timeout_ms: Optional[int] = None

@dataclass
class ExploredElementDetail:
    role: str
    name: Optional[str] = None
    id: Optional[str] = None
    bounds: Optional[Dict[str, Any]] = None
    value: Optional[str] = None
    description: Optional[str] = None
    text: Optional[str] = None
    parent_id: Optional[str] = None
    children_ids: List[str] = field(default_factory=list)
    suggested_selector: str = ""

@dataclass
class ExploreRequest(ChainedRequest):
    timeout_ms: Optional[int] = None

@dataclass
class ExploreResponse:
    parent: ElementResponse
    children: List[ExploredElementDetail]

@dataclass
class ActivateApplicationRequest:
    app_name: str 