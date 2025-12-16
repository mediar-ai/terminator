pub mod cancellation;
pub mod duration_parser;
pub mod event_pipe;
pub mod execution_logger;
pub mod expression_eval;
pub mod helpers;
pub mod log_pipe;
pub mod mcp_types;
pub mod omniparser;
pub mod output_parser;
pub mod prompt;
pub mod scripting_engine;
pub mod sentry;
pub mod server;
pub mod server_sequence;
pub mod telemetry;
pub mod tool_logging;
pub mod tree_formatter;
pub mod utils;
pub mod vision;
pub mod workflow_format;
pub mod workflow_typescript;

// Re-export ui_tree_diff from terminator crate (single source of truth)
pub use terminator::ui_tree_diff;

// Re-export window_manager from terminator crate (single source of truth)
pub use terminator::{WindowCache, WindowInfo, WindowManager, WindowPlacement};

// Re-export the extract_content_json function for testing
pub use server::extract_content_json;
