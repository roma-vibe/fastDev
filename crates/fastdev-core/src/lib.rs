//! fastDev core: everything except the UI. Used by the Tauri app, the CLI and tests.

pub mod api;
pub mod authoring;
pub mod control;
pub mod edits;
pub mod envfile;
pub mod error;
pub mod events;
pub mod generator;
pub mod git;
pub mod index;
pub mod jobs;
pub mod library;
pub mod manifest;
pub mod project_meta;
pub mod registry;
pub mod render;
pub mod runner;
pub mod settings;
pub mod shell;
pub mod toolchain;
pub mod updates;
pub mod util;

pub use api::{Core, CoreConfig};
pub use error::{Error, ErrorCode, Result};
