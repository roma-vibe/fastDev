//! Types shared by the fastDev app and the `fastdev-mcp` bridge:
//! well-known paths, control-socket messages and MCP tool definitions.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod tools;

/// Version of the control-socket protocol. Bumped on incompatible changes.
pub const PROTOCOL_VERSION: u32 = 1;

/// Returned by the bridge for every tool call while the app is closed.
pub const NOT_RUNNING_MESSAGE: &str = "fastDev is not running. Ask the user to start the fastDev app, then retry. \
Do not try to launch the app yourself.";

/// Folder with settings, the project registry and the control socket.
///
/// `FASTDEV_HOME` overrides it (used by tests and for isolated runs).
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("FASTDEV_HOME") {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("fastDev")
}

/// Unix socket the running app listens on.
///
/// Socket paths are limited to 104 bytes on macOS; for long data folders (tests, custom
/// `FASTDEV_HOME`) a per-user temp path derived from the data folder is used instead.
pub fn socket_path() -> PathBuf {
    let dir = data_dir();
    let path = dir.join("fastdev.sock");
    if path.as_os_str().len() < 100 {
        return path;
    }
    // FNV-1a keeps the name stable for the same data folder.
    let hash = dir
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3));
    std::env::temp_dir().join(format!("fastdev-{hash:016x}.sock"))
}

/// Who is calling the app API. MCP callers (everything arriving on the control socket)
/// are limited to [`tools::definitions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Caller {
    #[default]
    Mcp,
    Cli,
    Ui,
}

/// One request line on the control socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub arguments: Value,
}

/// One response line on the control socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlResponse {
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ControlError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlError {
    pub code: String,
    pub message: String,
}

impl ControlResponse {
    pub fn ok(id: u64, result: Value) -> Self {
        Self { id, result: Some(result), error: None }
    }

    pub fn err(id: u64, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { id, result: None, error: Some(ControlError { code: code.into(), message: message.into() }) }
    }
}
