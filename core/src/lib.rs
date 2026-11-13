//! WotsLog AI core: Tauri-independent so tests and benchmarks run the production code paths.
pub mod ai;
pub mod approval;
pub mod credentials;
pub mod error;
pub mod excerpt;
pub mod grouping;
pub mod linereader;
pub mod parser;
pub mod redaction;
pub mod scan;
pub mod search;
pub mod session;
pub mod settings;
pub mod types;

pub use error::{AppError, Result};
