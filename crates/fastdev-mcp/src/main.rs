//! `fastdev-mcp` — stdio MCP server for fastDev.
//!
//! Lists all fastDev tools and forwards every call to the running app over its Unix socket.
//! It never launches the app: when the app is closed, calls fail with a clear message.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use fastdev_protocol::{ControlRequest, ControlResponse, NOT_RUNNING_MESSAGE, socket_path, tools};
use serde_json::{Value, json};

const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "fastDev is the owner's local app with a versioned library of project skeletons. \
Use it to create new projects (create_project), run/stop/build them (run_project_command) and to author, \
update and fork skeletons. Start with fastdev_status. Before creating or changing skeletons, read \
get_authoring_guide. Skeleton files are edited directly on disk in the draft folder returned by \
create_skeleton_draft; never edit published versions. If a call says fastDev is not running, ask the user \
to start the app — do not launch it yourself.";

fn main() {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("--version" | "-V") => {
            println!("fastdev-mcp {SERVER_VERSION}");
            return;
        }
        Some("--check") => {
            match forward("fastdev_status", json!({})) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default()),
                Err(message) => {
                    eprintln!("{message}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Some("--help" | "-h") => {
            println!(
                "fastdev-mcp — stdio MCP server for fastDev\n\n\
                 Register with Claude Code:\n  claude mcp add --scope user fastdev -- \"{}\"\n\n\
                 Options:\n  --check    call fastdev_status on the running app\n  --version  print the version",
                std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "fastdev-mcp".into())
            );
            return;
        }
        _ => {}
    }

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let responses: Vec<Value> = match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(batch)) => batch.into_iter().filter_map(handle_message).collect(),
            Ok(message) => handle_message(message).into_iter().collect(),
            Err(err) => vec![error_response(Value::Null, -32700, &format!("parse error: {err}"))],
        };
        for response in responses {
            let Ok(text) = serde_json::to_string(&response) else { continue };
            if writeln!(stdout, "{text}").and_then(|()| stdout.flush()).is_err() {
                return;
            }
        }
    }
}

/// Returns a response for requests; `None` for notifications.
fn handle_message(message: Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let method = message.get("method").and_then(Value::as_str).unwrap_or_default().to_string();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let id = id?; // notifications (initialized, cancelled…) need no answer
    let result = match method.as_str() {
        "initialize" => {
            let version = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(DEFAULT_PROTOCOL);
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "fastdev", "title": "fastDev", "version": SERVER_VERSION },
                "instructions": INSTRUCTIONS,
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools::definitions() }),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
            let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            if !tools::names().contains(&name) {
                return Some(error_response(id, -32602, &format!("unknown tool: {name}")));
            }
            match forward(name, arguments) {
                Ok(result) => tool_result(&result, false),
                Err(message) => tool_result(&Value::String(message), true),
            }
        }
        "resources/list" => json!({ "resources": [] }),
        "prompts/list" => json!({ "prompts": [] }),
        _ => return Some(error_response(id, -32601, &format!("method not found: {method}"))),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn tool_result(value: &Value, is_error: bool) -> Value {
    let text = match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    };
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Sends one request to the app and waits for its answer.
fn forward(method: &str, arguments: Value) -> Result<Value, String> {
    let path = socket_path();
    let stream = UnixStream::connect(&path).map_err(|_| NOT_RUNNING_MESSAGE.to_string())?;
    let wait = arguments.get("wait_seconds").and_then(Value::as_u64).unwrap_or(300).min(600);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(wait + 120)));
    let mut writer = stream.try_clone().map_err(|err| err.to_string())?;
    let request = ControlRequest { id: 1, method: method.to_string(), arguments };
    let mut text = serde_json::to_string(&request).map_err(|err| err.to_string())?;
    text.push('\n');
    writer.write_all(text.as_bytes()).map_err(|_| NOT_RUNNING_MESSAGE.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|err| format!("no answer from fastDev ({err}); the operation may still be running, check get_job"))?;
    if line.trim().is_empty() {
        return Err("fastDev closed the connection without answering".into());
    }
    let response: ControlResponse = serde_json::from_str(&line).map_err(|err| format!("invalid answer: {err}"))?;
    match (response.result, response.error) {
        (_, Some(error)) => Err(format!("[{}] {}", error.code, error.message)),
        (Some(result), None) => Ok(result),
        (None, None) => Ok(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_initialize_and_lists_tools() {
        let init = handle_message(
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}),
        )
        .unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert!(handle_message(json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
        let list = handle_message(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().len() > 15);
        let bad = handle_message(json!({"jsonrpc":"2.0","id":3,"method":"nope"})).unwrap();
        assert_eq!(bad["error"]["code"], -32601);
    }
}
