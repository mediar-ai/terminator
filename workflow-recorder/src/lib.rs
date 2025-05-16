//! Workflow Recorder crate for Windows
//!
//! This crate provides functionality to record user interactions with the Windows UI,
//! including mouse clicks, keyboard input, and window focus changes.
//! The recorded workflow can be saved as a JSON file for later playback or analysis.

#![cfg_attr(not(target_os = "windows"), allow(unused))]

mod events;
mod recorder;
mod error;
mod intent;

pub use events::*;
pub use recorder::*;
pub use error::*;
pub use intent::*;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
} 