use pyo3::prelude::*;
use pyo3_stub_gen::derive::*;
use pyo3_async_runtimes::tokio as pyo3_tokio;
use pyo3_async_runtimes::TaskLocals;
use ::terminator_core::locator::Locator as TerminatorLocator;
use crate::exceptions::automation_error_to_pyerr;
use crate::types::{UIElementAttributes, Bounds, ClickResult};
use crate::element::UIElement;

/// Locator for finding UI elements by selector.
#[gen_stub_pyclass]
#[pyclass(name = "Locator")]
pub struct Locator {
    pub inner: TerminatorLocator,
}

#[gen_stub_pymethods]
#[pymethods]
impl Locator {
    #[pyo3(name = "first", text_signature = "($self)")]
    /// (async) Get the first matching element.
    /// 
    /// Returns:
    ///     UIElement: The first matching element.
    pub fn first<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let element = locator.first(None).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(UIElement { inner: element })
        })
    }

    #[pyo3(name = "all", signature = (timeout_ms=None, depth=None))]
    #[pyo3(text_signature = "($self, timeout_ms, depth)")]
    /// (async) Get all matching elements.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///     depth (Optional[int]): Maximum depth to search.
    /// 
    /// Returns:
    ///     List[UIElement]: List of matching elements.
    pub fn all<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>, depth: Option<usize>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let elements = locator.all(timeout_ms.map(std::time::Duration::from_millis), depth).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(elements.into_iter().map(|e| UIElement { inner: e }).collect::<Vec<_>>())
        })
    }

    #[pyo3(name = "wait", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Wait for the first matching element.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     UIElement: The first matching element.
    pub fn wait<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let element = locator.wait(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(UIElement { inner: element })
        })
    }

    #[pyo3(name = "timeout", text_signature = "($self, timeout_ms)")]
    /// Set a default timeout for this locator.
    /// 
    /// Args:
    ///     timeout_ms (int): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     Locator: A new locator with the specified timeout.
    pub fn timeout(&self, timeout_ms: u64) -> Locator {
        Locator { inner: self.inner.clone().set_default_timeout(std::time::Duration::from_millis(timeout_ms)) }
    }

    #[pyo3(name = "locator", text_signature = "($self, selector)")]
    /// Chain another selector.
    /// 
    /// Args:
    ///     selector (str): The selector string.
    /// 
    /// Returns:
    ///     Locator: A new locator with the chained selector.
    pub fn locator(&self, selector: &str) -> PyResult<Locator> {
        let locator = self.inner.locator(selector);
        Ok(Locator { inner: locator })
    }

    #[pyo3(name = "click", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Click on the first matching element.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     ClickResult: Result of the click operation.
    pub fn click<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let result = locator.click(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(ClickResult::from(result))
        })
    }

    #[pyo3(name = "type_text", signature = (text, use_clipboard=None, timeout_ms=None))]
    #[pyo3(text_signature = "($self, text, use_clipboard, timeout_ms)")]
    /// (async) Type text into the first matching element.
    /// 
    /// Args:
    ///     text (str): The text to type.
    ///     use_clipboard (Optional[bool]): Whether to use clipboard for pasting.
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     None
    pub fn type_text<'py>(&self, py: Python<'py>, text: &str, use_clipboard: Option<bool>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        let text = text.to_string();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.type_text(&text, use_clipboard.unwrap_or(false), timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "press_key", signature = (key, timeout_ms=None))]
    #[pyo3(text_signature = "($self, key, timeout_ms)")]
    /// (async) Press a key on the first matching element.
    /// 
    /// Args:
    ///     key (str): The key to press.
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     None
    pub fn press_key<'py>(&self, py: Python<'py>, key: &str, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        let key = key.to_string();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.press_key(&key, timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "text", signature = (max_depth=None, timeout_ms=None))]
    #[pyo3(text_signature = "($self, max_depth, timeout_ms)")]
    /// (async) Get text from the first matching element.
    /// 
    /// Args:
    ///     max_depth (Optional[int]): Maximum depth to search for text.
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     str: The element's text content.
    pub fn text<'py>(&self, py: Python<'py>, max_depth: Option<usize>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let text = locator.text(max_depth.unwrap_or(1), timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(text)
        })
    }

    #[pyo3(name = "attributes", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Get attributes from the first matching element.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     UIElementAttributes: The element's attributes.
    pub fn attributes<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let attrs = locator.attributes(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(UIElementAttributes {
                role: attrs.role,
                name: attrs.name,
                label: attrs.label,
                value: attrs.value,
                description: attrs.description,
                properties: attrs.properties.into_iter()
                    .map(|(k, v)| (k, v.map(|v| v.to_string())))
                    .collect(),
                is_keyboard_focusable: attrs.is_keyboard_focusable,
            })
        })
    }

    #[pyo3(name = "bounds", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Get bounds from the first matching element.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     Bounds: The element's bounds.
    pub fn bounds<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let (x, y, width, height) = locator.bounds(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(Bounds { x, y, width, height })
        })
    }

    #[pyo3(name = "is_visible", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Check if the element is visible.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     bool: True if the element is visible.
    pub fn is_visible<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let visible = locator.is_visible(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(visible)
        })
    }

    #[pyo3(name = "expect_enabled", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Wait for the element to be enabled.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     UIElement: The enabled element.
    pub fn expect_enabled<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let element = locator.expect_enabled(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(UIElement { inner: element })
        })
    }

    #[pyo3(name = "expect_visible", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Wait for the element to be visible.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     UIElement: The visible element.
    pub fn expect_visible<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let element = locator.expect_visible(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(UIElement { inner: element })
        })
    }

    #[pyo3(name = "expect_text_equals", signature = (expected_text, max_depth=None, timeout_ms=None))]
    #[pyo3(text_signature = "($self, expected_text, max_depth, timeout_ms)")]
    /// (async) Wait for the element's text to equal the expected text.
    /// 
    /// Args:
    ///     expected_text (str): The expected text.
    ///     max_depth (Optional[int]): Maximum depth to search for text.
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     UIElement: The element with matching text.
    pub fn expect_text_equals<'py>(&self, py: Python<'py>, expected_text: &str, max_depth: Option<usize>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        let expected_text = expected_text.to_string();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let element = locator.expect_text_equals(&expected_text, max_depth.unwrap_or(1), timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(UIElement { inner: element })
        })
    }

    #[pyo3(name = "within", text_signature = "($self, element)")]
    /// Set the root element for this locator.
    /// 
    /// Args:
    ///     element (UIElement): The root element.
    /// 
    /// Returns:
    ///     Locator: A new locator with the specified root element.
    pub fn within(&self, element: &UIElement) -> Locator {
        Locator { inner: self.inner.clone().within(element.inner.clone()) }
    }

    #[pyo3(name = "double_click", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Double click on the first matching element.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     ClickResult: Result of the click operation.
    pub fn double_click<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let result = locator.double_click(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(ClickResult::from(result))
        })
    }

    #[pyo3(name = "right_click", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Right click on the first matching element.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     None
    pub fn right_click<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.right_click(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "hover", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Hover over the first matching element.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     None
    pub fn hover<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.hover(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "explore", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Explore the first matching element and its direct children.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     ExploreResponse: Details about the element and its children.
    pub fn explore<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let response = locator.explore(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(crate::types::ExploreResponse {
                parent: UIElement { inner: response.parent },
                children: response.children.into_iter().map(|child| crate::types::ExploredElementDetail {
                    role: child.role,
                    name: child.name,
                    id: child.id,
                    bounds: child.bounds.map(|(x, y, width, height)| Bounds { x, y, width, height }),
                    value: child.value,
                    description: child.description,
                    text: child.text,
                    parent_id: child.parent_id,
                    children_ids: child.children_ids,
                    suggested_selector: child.suggested_selector,
                }).collect(),
            })
        })
    }

    #[pyo3(name = "id", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Get the id of the first matching element.
    ///
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     Optional[str]: The element's id, or None if not present.
    pub fn id<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.id(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))
        })
    }

    #[pyo3(name = "role", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Get the role of the first matching element.
    ///
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     str: The element's role.
    pub fn role<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.role(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))
        })
    }

    #[pyo3(name = "children", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Get the children of the first matching element.
    ///
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     List[UIElement]: The element's children.
    pub fn children<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let children = locator.children(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(children.into_iter().map(|e| UIElement { inner: e }).collect::<Vec<_>>())
        })
    }

    #[pyo3(name = "parent", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Get the parent of the first matching element.
    ///
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     Optional[UIElement]: The element's parent, or None if not present.
    pub fn parent<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let parent = locator.parent(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(parent.map(|e| UIElement { inner: e }))
        })
    }

    #[pyo3(name = "set_value", signature = (value, timeout_ms=None))]
    #[pyo3(text_signature = "($self, value, timeout_ms)")]
    /// (async) Set value of the first matching element.
    ///
    /// Args:
    ///     value (str): The value to set.
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     None
    pub fn set_value<'py>(&self, py: Python<'py>, value: &str, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        let value = value.to_string();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.set_value(&value, timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "is_focused", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Check if the first matching element is focused.
    ///
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     bool: True if the element is focused.
    pub fn is_focused<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.is_focused(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))
        })
    }

    #[pyo3(name = "perform_action", signature = (action, timeout_ms=None))]
    #[pyo3(text_signature = "($self, action, timeout_ms)")]
    /// (async) Perform a named action on the first matching element.
    ///
    /// Args:
    ///     action (str): The action name.
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     None
    pub fn perform_action<'py>(&self, py: Python<'py>, action: &str, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        let action = action.to_string();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.perform_action(&action, timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "scroll", signature = (direction, amount, timeout_ms=None))]
    #[pyo3(text_signature = "($self, direction, amount, timeout_ms)")]
    /// (async) Scroll the first matching element in a given direction.
    ///
    /// Args:
    ///     direction (str): The scroll direction (e.g., "up", "down").
    ///     amount (float): The amount to scroll.
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     None
    pub fn scroll<'py>(&self, py: Python<'py>, direction: &str, amount: f64, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        let direction = direction.to_string();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.scroll(&direction, amount, timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "activate_window", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Activate the window containing the first matching element.
    ///
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     None
    pub fn activate_window<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.activate_window(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "name", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Get the name of the first matching element.
    ///
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     Optional[str]: The element's name, or None if not present.
    pub fn name<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.name(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))
        })
    }

    #[pyo3(name = "is_keyboard_focusable", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Check if the first matching element is keyboard focusable.
    ///
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     bool: True if the element is keyboard focusable.
    pub fn is_keyboard_focusable<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.is_keyboard_focusable(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))
        })
    }

    #[pyo3(name = "mouse_drag", signature = (start_x, start_y, end_x, end_y, timeout_ms=None))]
    #[pyo3(text_signature = "($self, start_x, start_y, end_x, end_y, timeout_ms)")]
    /// (async) Drag mouse from start to end coordinates on the first matching element.
    ///
    /// Args:
    ///     start_x (float): Starting x coordinate.
    ///     start_y (float): Starting y coordinate.
    ///     end_x (float): Ending x coordinate.
    ///     end_y (float): Ending y coordinate.
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     None
    pub fn mouse_drag<'py>(&self, py: Python<'py>, start_x: f64, start_y: f64, end_x: f64, end_y: f64, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.mouse_drag(start_x, start_y, end_x, end_y, timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "mouse_click_and_hold", signature = (x, y, timeout_ms=None))]
    #[pyo3(text_signature = "($self, x, y, timeout_ms)")]
    /// (async) Press and hold mouse at (x, y) on the first matching element.
    ///
    /// Args:
    ///     x (float): X coordinate.
    ///     y (float): Y coordinate.
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     None
    pub fn mouse_click_and_hold<'py>(&self, py: Python<'py>, x: f64, y: f64, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.mouse_click_and_hold(x, y, timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "mouse_move", signature = (x, y, timeout_ms=None))]
    #[pyo3(text_signature = "($self, x, y, timeout_ms)")]
    /// (async) Move mouse to (x, y) on the first matching element.
    ///
    /// Args:
    ///     x (float): X coordinate.
    ///     y (float): Y coordinate.
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     None
    pub fn mouse_move<'py>(&self, py: Python<'py>, x: f64, y: f64, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.mouse_move(x, y, timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "mouse_release", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Release mouse button on the first matching element.
    ///
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     None
    pub fn mouse_release<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            locator.mouse_release(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "application", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Get the containing application element of the first matching element.
    ///
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     Optional[UIElement]: The application element, or None if not present.
    pub fn application<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let app = locator.application(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(app.map(|e| UIElement { inner: e }))
        })
    }

    #[pyo3(name = "window", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Get the containing window element of the first matching element.
    ///
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     Optional[UIElement]: The window element, or None if not present.
    pub fn window<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let win = locator.window(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(win.map(|e| UIElement { inner: e }))
        })
    }

    #[pyo3(name = "process_id", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Get the process ID of the application containing the first matching element.
    ///
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    ///
    /// Returns:
    ///     int: The process ID of the application.
    pub fn process_id<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let pid = locator.process_id(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(pid)
        })
    }

    #[pyo3(name = "highlight", signature = (color=None, duration_ms=None, timeout_ms=None))]
    #[pyo3(text_signature = "($self, color, duration_ms, timeout_ms)")]
    /// (async) Highlights the first matching element with a colored border.
    /// 
    /// Args:
    ///     color (Optional[int]): BGR color code (32-bit integer). Default: 0x0000FF (red)
    ///     duration_ms (Optional[int]): Duration in milliseconds.
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     None
    pub fn highlight<'py>(&self, py: Python<'py>, color: Option<u32>, duration_ms: Option<u64>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let duration = duration_ms.map(std::time::Duration::from_millis);
            locator.highlight(color, duration, timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(())
        })
    }

    #[pyo3(name = "capture", signature = (timeout_ms=None))]
    #[pyo3(text_signature = "($self, timeout_ms)")]
    /// (async) Capture a screenshot of the first matching element.
    /// 
    /// Args:
    ///     timeout_ms (Optional[int]): Timeout in milliseconds.
    /// 
    /// Returns:
    ///     ScreenshotResult: The screenshot result containing the image data.
    pub fn capture<'py>(&self, py: Python<'py>, timeout_ms: Option<u64>) -> PyResult<Bound<'py, PyAny>> {
        let locator = self.inner.clone();
        pyo3_tokio::future_into_py_with_locals(py, TaskLocals::with_running_loop(py)?, async move {
            let screenshot = locator.capture(timeout_ms.map(std::time::Duration::from_millis)).await.map_err(|e| automation_error_to_pyerr(e))?;
            Ok(crate::types::ScreenshotResult::from(screenshot))
        })
    }
} 