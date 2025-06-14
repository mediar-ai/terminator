use pyo3_stub_gen::create_exception;

// Custom Python exceptions for advanced error mapping
create_exception!(
    terminator,
    ElementNotFoundError,
    pyo3::exceptions::PyRuntimeError
);
create_exception!(terminator, TimeoutError, pyo3::exceptions::PyRuntimeError);
create_exception!(
    terminator,
    PermissionDeniedError,
    pyo3::exceptions::PyRuntimeError
);
create_exception!(terminator, PlatformError, pyo3::exceptions::PyRuntimeError);
create_exception!(
    terminator,
    UnsupportedOperationError,
    pyo3::exceptions::PyRuntimeError
);
create_exception!(
    terminator,
    UnsupportedPlatformError,
    pyo3::exceptions::PyRuntimeError
);
create_exception!(
    terminator,
    InvalidArgumentError,
    pyo3::exceptions::PyRuntimeError
);
create_exception!(terminator, InternalError, pyo3::exceptions::PyRuntimeError);

use ::terminator_core::errors::AutomationError;

// Advanced error mapping
pub fn automation_error_to_pyerr(e: AutomationError) -> pyo3::PyErr {
    let msg = format!("{e}");
    match e {
        AutomationError::ElementNotFound(_) => ElementNotFoundError::new_err(msg),
        AutomationError::Timeout(_) => TimeoutError::new_err(msg),
        AutomationError::PermissionDenied(_) => PermissionDeniedError::new_err(msg),
        AutomationError::PlatformError(_) => PlatformError::new_err(msg),
        AutomationError::UnsupportedOperation(_) => UnsupportedOperationError::new_err(msg),
        AutomationError::UnsupportedPlatform(_) => UnsupportedPlatformError::new_err(msg),
        AutomationError::InvalidArgument(_) => InvalidArgumentError::new_err(msg),
        AutomationError::Internal(_) => InternalError::new_err(msg),
    }
}
