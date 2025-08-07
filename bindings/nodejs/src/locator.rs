use napi_derive::napi;
use terminator::Locator as TerminatorLocator;

use crate::map_error;
use crate::Element;
use crate::Selector;
use napi::bindgen_prelude::Either;

/// Locator for finding UI elements by selector.
#[napi(js_name = "Locator")]
pub struct Locator {
    inner: TerminatorLocator,
}

impl std::fmt::Display for Locator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Locator({})", self.inner.selector_string())
    }
}

impl From<TerminatorLocator> for Locator {
    fn from(l: TerminatorLocator) -> Self {
        Locator { inner: l }
    }
}

#[napi]
impl Locator {
    /// (async) Get the first matching element.
    ///
    /// @returns {Promise<Element>} The first matching element.
    #[napi]
    pub async fn first(&self) -> napi::Result<Element> {
        self.inner
            .first(None)
            .await
            .map(Element::from)
            .map_err(map_error)
    }

    /// (async) Get all matching elements.
    ///
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @param {number} [depth] - Maximum depth to search.
    /// @returns {Promise<Array<Element>>} List of matching elements.
    #[napi]
    pub async fn all(
        &self,
        timeout_ms: Option<f64>,
        depth: Option<u32>,
    ) -> napi::Result<Vec<Element>> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        let depth = depth.map(|d| d as usize);
        self.inner
            .all(timeout, depth)
            .await
            .map(|els| els.into_iter().map(Element::from).collect())
            .map_err(map_error)
    }

    /// (async) Wait for the first matching element.
    ///
    /// @param {number} [timeoutMs] [depth] - Timeout in milliseconds.
    /// @returns {Promise<Element>} The first matching element.
    #[napi]
    pub async fn wait(&self, timeout_ms: Option<f64>, depth: Option<u32>) -> napi::Result<Element> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner
            .wait(timeout, Some(depth.unwrap_or(50) as usize))
            .await
            .map(Element::from)
            .map_err(map_error)
    }

    /// Set a default timeout for this locator.
    ///
    /// @param {number} timeoutMs - Timeout in milliseconds.
    /// @returns {Locator} A new locator with the specified timeout.
    #[napi]
    pub fn timeout(&self, timeout_ms: f64) -> Locator {
        let loc = self
            .inner
            .clone()
            .set_default_timeout(std::time::Duration::from_millis(timeout_ms as u64));
        Locator::from(loc)
    }

    /// Set the root element for this locator.
    ///
    /// @param {Element} element - The root element.
    /// @returns {Locator} A new locator with the specified root element.
    #[napi]
    pub fn within(&self, element: &Element) -> Locator {
        let loc = self.inner.clone().within(element.inner.clone());
        Locator::from(loc)
    }

    /// Chain another selector.
    /// Accepts either a selector string or a Selector object.
    ///
    /// @param {string | Selector} selector - The selector.
    /// @returns {Locator} A new locator with the chained selector.
    #[napi]
    pub fn locator(
        &self,
        #[napi(ts_arg_type = "string | Selector")] selector: Either<String, &Selector>,
    ) -> napi::Result<Locator> {
        use napi::bindgen_prelude::Either::*;
        let sel_rust: terminator::selector::Selector = match selector {
            A(sel_str) => sel_str.as_str().into(),
            B(sel_obj) => sel_obj.inner.clone(),
        };
        let loc = self.inner.clone().locator(sel_rust);
        Ok(Locator::from(loc))
    }
}
