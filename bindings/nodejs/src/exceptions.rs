use napi::{self, Status};
use terminator::errors::AutomationError;

/// Map Terminator errors to NAPI errors
pub fn map_error(err: AutomationError) -> napi::Error {
    match err {
        AutomationError::ElementNotFound(msg) => {
            napi::Error::new(Status::InvalidArg, format!("ELEMENT_NOT_FOUND: {msg}"))
        }
        AutomationError::Timeout(msg) => napi::Error::new(
            Status::GenericFailure,
            format!("OPERATION_TIMED_OUT: {msg}"),
        ),
        AutomationError::PermissionDenied(msg) => {
            napi::Error::new(Status::GenericFailure, format!("PERMISSION_DENIED: {msg}"))
        }
        AutomationError::PlatformError(e) => {
            napi::Error::new(Status::GenericFailure, format!("PLATFORM_ERROR: {e}"))
        }
        AutomationError::UnsupportedOperation(msg) => {
            napi::Error::new(Status::InvalidArg, format!("UNSUPPORTED_OPERATION: {msg}"))
        }
        AutomationError::UnsupportedPlatform(msg) => {
            napi::Error::new(Status::InvalidArg, format!("UNSUPPORTED_PLATFORM: {msg}"))
        }
        AutomationError::InvalidArgument(e) => {
            napi::Error::new(Status::InvalidArg, format!("INVALID_ARGUMENT: {e}"))
        }
        AutomationError::Internal(e) => {
            napi::Error::new(Status::GenericFailure, format!("INTERNAL_ERROR: {e}"))
        }
        AutomationError::InvalidSelector(e) => {
            napi::Error::new(Status::InvalidArg, format!("INVALID_SELECTOR: {e}"))
        }
    }
}
