//! Unix-socket control server used by the `fastdev-mcp` bridge (SPEC §8.1).
//!
//! One JSON request per line, one JSON response per line. Every connection is treated
//! as an MCP caller: the socket never grants UI-only methods.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fastdev_protocol::{Caller, ControlRequest, ControlResponse, PROTOCOL_VERSION};
use serde_json::json;

use crate::api::Core;
use crate::error::{Error, IoContext, Result};

pub struct ControlServer {
    path: PathBuf,
}

impl ControlServer {
    /// Binds the socket (removing a stale one) and serves requests on background threads.
    pub fn start(core: Arc<Core>, path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).at(dir)?;
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
        if path.exists() {
            if UnixStream::connect(path).is_ok() {
                return Err(Error::conflict("another fastDev instance is already listening on the control socket"));
            }
            std::fs::remove_file(path).at(path)?;
        }
        let listener = UnixListener::bind(path).at(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).at(path)?;
        std::thread::Builder::new()
            .name("fastdev-control".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let core = core.clone();
                    std::thread::spawn(move || serve(core, stream));
                }
            })
            .map_err(|err| Error::internal(err.to_string()))?;
        Ok(Self { path: path.to_path_buf() })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn serve(core: Arc<Core>, stream: UnixStream) {
    let Ok(mut writer) = stream.try_clone() else { return };
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<ControlRequest>(&line) {
            Ok(request) => handle(&core, request),
            Err(err) => ControlResponse::err(0, "invalid_input", format!("invalid request: {err}")),
        };
        let Ok(mut text) = serde_json::to_string(&response) else { break };
        text.push('\n');
        if writer.write_all(text.as_bytes()).and_then(|()| writer.flush()).is_err() {
            break;
        }
    }
}

fn handle(core: &Arc<Core>, request: ControlRequest) -> ControlResponse {
    if request.method == "hello" {
        return ControlResponse::ok(
            request.id,
            json!({ "protocolVersion": PROTOCOL_VERSION, "app": "fastDev", "appVersion": core.app_version() }),
        );
    }
    match core.call(&request.method, request.arguments, Caller::Mcp) {
        Ok(result) => ControlResponse::ok(request.id, result),
        Err(err) => ControlResponse::err(request.id, err.code.as_str(), err.message),
    }
}
