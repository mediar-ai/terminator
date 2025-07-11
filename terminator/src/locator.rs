use tracing::{debug, instrument};

use crate::element::UIElement;
use crate::errors::AutomationError;
use crate::platforms::AccessibilityEngine;
use crate::selector::{Selector, SpatialRelation};
use std::sync::Arc;
use std::time::Duration;
use tokio::task;

// Default timeout if none is specified on the locator itself
const DEFAULT_LOCATOR_TIMEOUT: Duration = Duration::from_secs(30);

/// A high-level API for finding and interacting with UI elements
///
/// For maximum precision, prefer role|name format (e.g., "button|Submit")
/// over broad selectors like "role:Button" that could match multiple elements.
#[derive(Clone)]
pub struct Locator {
    engine: Arc<dyn AccessibilityEngine>,
    selector: Selector,
    timeout: Duration, // Default timeout for this locator instance
    root: Option<UIElement>,
}

impl Locator {
    /// Create a new locator with the given selector
    pub(crate) fn new(engine: Arc<dyn AccessibilityEngine>, selector: Selector) -> Self {
        Self {
            engine,
            selector,
            timeout: DEFAULT_LOCATOR_TIMEOUT, // Use default
            root: None,
        }
    }

    /// Set a default timeout for waiting operations on this locator instance.
    /// This timeout is used if no specific timeout is passed to action/wait methods.
    pub fn set_default_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the root element for this locator
    pub fn within(mut self, element: UIElement) -> Self {
        self.root = Some(element);
        self
    }

    fn relation_ok(
        cand: (f64, f64, f64, f64),
        anch: (f64, f64, f64, f64),
        rel: SpatialRelation,
        max_px: u32,
    ) -> bool {
        match rel {
            SpatialRelation::Above => cand.1 + cand.3 <= anch.1,
            SpatialRelation::Below => cand.1 >= anch.1 + anch.3,
            SpatialRelation::RightOf => cand.0 >= anch.0 + anch.2,
            SpatialRelation::Near => {
                let (cx, cy) = (cand.0 + cand.2 / 2.0, cand.1 + cand.3 / 2.0);
                let (ax, ay) = (anch.0 + anch.2 / 2.0, anch.1 + anch.3 / 2.0);
                let dist = ((cx - ax).powi(2) + (cy - ay).powi(2)).sqrt();
                dist <= max_px as f64
            }
        }
    }

    fn query_spatial(
        &self,
        relation: SpatialRelation,
        anchor_sel: &Selector,
        max_px: u32,
    ) -> Result<Vec<UIElement>, AutomationError> {
        // Resolve anchor elements
        let anchors = self
            .engine
            .find_elements(anchor_sel, self.root.as_ref(), None, None)?;
        if anchors.is_empty() {
            return Err(AutomationError::ElementNotFound(
                "Anchor selector returned no elements".to_string(),
            ));
        }

        // Collect all candidates under root
        let mut stack = vec![if let Some(r) = &self.root {
            r.clone()
        } else {
            self.engine.get_root_element()
        }];
        let mut candidates = Vec::new();
        while let Some(el) = stack.pop() {
            candidates.push(el.clone());
            if let Ok(children) = el.children() {
                stack.extend(children);
            }
        }

        let mut result = Vec::new();
        for cand in &candidates {
            let cb = match cand.bounds() {
                Ok(b) => b,
                Err(_) => continue,
            };
            for anchor in &anchors {
                if let Ok(ab) = anchor.bounds() {
                    if Self::relation_ok(cb, ab, relation, max_px) {
                        result.push(cand.clone());
                        break;
                    }
                }
            }
        }

        if relation == SpatialRelation::Near {
            // sort by distance to first anchor center
            let (ax, ay) = {
                let b = anchors[0].bounds()?;
                (b.0 + b.2 / 2.0, b.1 + b.3 / 2.0)
            };
            result.sort_by(|e1, e2| {
                let d = |e: &UIElement| {
                    if let Ok(b) = e.bounds() {
                        let (cx, cy) = (b.0 + b.2 / 2.0, b.1 + b.3 / 2.0);
                        ((cx - ax).powi(2) + (cy - ay).powi(2)).sqrt()
                    } else {
                        f64::MAX
                    }
                };
                d(e1)
                    .partial_cmp(&d(e2))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Ok(result)
    }

    /// Get all elements matching this locator, waiting up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    pub async fn all(
        &self,
        timeout: Option<Duration>,
        depth: Option<usize>,
    ) -> Result<Vec<UIElement>, AutomationError> {
        if let Selector::Spatial {
            relation,
            anchor,
            max_px,
        } = &self.selector
        {
            return self.query_spatial(*relation, anchor, *max_px);
        }
        // find_elements itself handles the timeout now
        self.engine.find_elements(
            &self.selector,
            self.root.as_ref(),
            Some(timeout.unwrap_or(self.timeout)),
            depth,
        )
    }

    pub async fn first(&self, timeout: Option<Duration>) -> Result<UIElement, AutomationError> {
        let element = self.wait(timeout).await?;
        Ok(element)
    }

    /// Wait for an element matching the locator to appear, up to the specified timeout.
    /// If no timeout is provided, uses the locator's default timeout.
    #[instrument(level = "debug", skip(self, timeout))]
    pub async fn wait(&self, timeout: Option<Duration>) -> Result<UIElement, AutomationError> {
        debug!("Waiting for element matching selector: {:?}", self.selector);

        if let Selector::Invalid(reason) = &self.selector {
            return Err(AutomationError::InvalidSelector(reason.clone()));
        }

        let effective_timeout = timeout.unwrap_or(self.timeout);

        // Since the underlying engine's find_element is a blocking call that
        // already handles polling and timeouts, we should not wrap it in another async loop.
        // Instead, we run it in a blocking-safe thread to avoid stalling the async runtime.
        let engine = self.engine.clone();
        let selector = self.selector.clone();
        let root = self.root.clone();

        task::spawn_blocking(move || {
            engine.find_element(&selector, root.as_ref(), Some(effective_timeout))
        })
        .await
        .map_err(|e| AutomationError::PlatformError(format!("Task join error: {e}")))?
        .map_err(|e| {
            // The engine returns ElementNotFound on timeout. We convert it to a more specific Timeout error here.
            if let AutomationError::ElementNotFound(inner_msg) = e {
                AutomationError::Timeout(format!(
                    "Timed out after {effective_timeout:?} waiting for element {}. Original error: {inner_msg}",
                    self.selector_string()
                ))
            } else {
                e
            }
        })
    }

    pub async fn nth(
        &self,
        index: isize,
        timeout: Option<Duration>,
    ) -> Result<UIElement, AutomationError> {
        // Fetch all elements matching this locator
        let elements = self.all(timeout, None).await?;
        if elements.is_empty() {
            return Err(AutomationError::ElementNotFound(format!(
                "No elements found for selector {}",
                self.selector_string()
            )));
        }

        let positive_index: usize = if index >= 0 {
            index as usize
        } else {
            let abs = index.abs() as usize;
            if abs > elements.len() {
                return Err(AutomationError::InvalidArgument(format!(
                    "nth index {} is out of bounds for {} elements",
                    index,
                    elements.len()
                )));
            }
            elements.len() - abs
        };

        elements.get(positive_index).cloned().ok_or_else(|| {
            AutomationError::InvalidArgument(format!(
                "nth index {} is out of bounds for {} elements",
                index,
                elements.len()
            ))
        })
    }

    fn append_selector(&self, selector_to_append: Selector) -> Locator {
        let mut new_chain = match self.selector.clone() {
            Selector::Chain(existing_chain) => existing_chain,
            s if s != Selector::Path("/".to_string()) => vec![s], // Assuming root path is default
            _ => vec![],
        };

        // Append the new selector, flattening if it's also a chain
        match selector_to_append {
            Selector::Chain(mut next_chain_parts) => {
                new_chain.append(&mut next_chain_parts);
            }
            s => new_chain.push(s),
        }

        Locator {
            engine: self.engine.clone(),
            selector: Selector::Chain(new_chain),
            timeout: self.timeout,
            root: self.root.clone(),
        }
    }

    /// Adds a filter to find elements based on their visibility.
    pub fn visible(&self, is_visible: bool) -> Locator {
        self.append_selector(Selector::Visible(is_visible))
    }

    /// Get a nested locator
    pub fn locator(&self, selector: impl Into<Selector>) -> Locator {
        self.append_selector(selector.into())
    }

    pub fn selector_string(&self) -> String {
        format!("{:?}", self.selector)
    }
}
