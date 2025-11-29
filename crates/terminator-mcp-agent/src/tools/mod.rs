//! Tool implementations split by domain.
//!
//! This module contains the business logic for MCP tools, extracted from server.rs
//! to improve maintainability. The #[tool] annotated methods remain in server.rs
//! as thin wrappers that delegate to these implementations.

pub mod element;
pub mod inspection;
pub mod screenshot;
pub mod visibility;

// Re-export commonly used items
pub use element::{build_element_info, build_action_result, perform_post_action_verification, VerificationOptions, attach_ui_diff_to_result};
pub use inspection::*;
pub use screenshot::*;
pub use visibility::{ensure_element_in_view, ensure_visible_and_apply_highlight};
