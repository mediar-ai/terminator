use napi_derive::napi;
use terminator::Locator as TerminatorLocator;

use crate::map_error;
use crate::Element;

/// Locator for finding UI elements by selector.
#[napi(js_name = "Locator")]
pub struct Locator {
    inner: TerminatorLocator,
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
    /// @param {number} [timeoutMs] - Timeout in milliseconds.
    /// @returns {Promise<Element>} The first matching element.
    #[napi]
    pub async fn wait(&self, timeout_ms: Option<f64>) -> napi::Result<Element> {
        use std::time::Duration;
        let timeout = timeout_ms.map(|ms| Duration::from_millis(ms as u64));
        self.inner
            .wait(timeout)
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
    ///
    /// @param {string} selector - The selector string.
    /// @returns {Locator} A new locator with the chained selector.
    #[napi]
    pub fn locator(&self, selector: String) -> napi::Result<Locator> {
        let sel: terminator::selector::Selector = selector.as_str().into();
        let loc = self.inner.clone().locator(sel);
        Ok(Locator::from(loc))
    }

    #[napi]
    pub fn to_string(&self) -> String {
        format!("Locator({})", self.inner.selector_string())
    }
}
