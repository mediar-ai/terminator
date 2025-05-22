#![allow(unsafe_op_in_unsafe_fn)]

pub mod python;
use pyo3::prelude::*;

#[pymodule]
fn terminator(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<python::PyDesktop>()?;
    m.add_class::<python::PyUIElement>()?;
    m.add_class::<python::PyLocator>()?;
    m.add_class::<python::PyScreenshotResult>()?;
    m.add_class::<python::PyClickResult>()?;
    m.add_class::<python::PyCommandOutput>()?;

    m.add("ElementNotFoundError", _py.get_type::<python::ElementNotFoundError>())?;
    m.add("TimeoutError", _py.get_type::<python::TimeoutError>())?;
    m.add("PermissionDeniedError", _py.get_type::<python::PermissionDeniedError>())?;
    m.add("PlatformError", _py.get_type::<python::PlatformError>())?;
    m.add("UnsupportedOperationError", _py.get_type::<python::UnsupportedOperationError>())?;
    m.add("UnsupportedPlatformError", _py.get_type::<python::UnsupportedPlatformError>())?;
    m.add("InvalidArgumentError", _py.get_type::<python::InvalidArgumentError>())?;
    m.add("InternalError", _py.get_type::<python::InternalError>())?;
    Ok(())
}
