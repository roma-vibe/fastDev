//! Detection of installed tools with the user's login-shell PATH.
//!
//! GUI apps on macOS do not inherit the PATH configured in `.zshrc`/`.zprofile`
//! (nvm, Homebrew, `~/.local/bin`…), so the PATH is read once from the login shell.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use regex::Regex;
use serde::Serialize;

use crate::util::home_dir;

const MARKER: &str = "__FASTDEV_PATH__";

/// Tools shown in the toolchain overview.
pub const KNOWN_TOOLS: &[&str] =
    &["node", "npm", "git", "docker", "php", "composer", "cargo", "python3", "uv", XCODE_CLT];

/// Pseudo-tool: Apple's Command Line Tools (or Xcode), needed to build native code on macOS.
pub const XCODE_CLT: &str = "xcode-clt";

/// Pseudo-tool: Google Chrome (the app bundle), e.g. for browser extensions.
pub const CHROME: &str = "chrome";

static LOGIN_PATH: OnceLock<String> = OnceLock::new();
static CACHE: OnceLock<Mutex<HashMap<String, (Instant, ToolInfo)>>> = OnceLock::new();
const CACHE_TTL: Duration = Duration::from_secs(30);

/// PATH of the user's login shell, with common tool folders appended.
pub fn login_path() -> &'static str {
    LOGIN_PATH.get_or_init(|| {
        let mut parts: Vec<String> = resolve_shell_path()
            .or_else(|| std::env::var("PATH").ok())
            .unwrap_or_default()
            .split(':')
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect();
        let home = home_dir();
        let extra = [
            home.join(".local/bin"),
            home.join(".cargo/bin"),
            home.join(".volta/bin"),
            home.join(".bun/bin"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
            PathBuf::from("/usr/sbin"),
            PathBuf::from("/sbin"),
            PathBuf::from("/Applications/Docker.app/Contents/Resources/bin"),
        ];
        for dir in extra {
            let dir = dir.to_string_lossy().into_owned();
            if !parts.contains(&dir) {
                parts.push(dir);
            }
        }
        parts.join(":")
    })
}

fn resolve_shell_path() -> Option<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let script = format!("printf '{MARKER}%s{MARKER}' \"$PATH\"");
    let mut child = Command::new(&shell)
        .args(["-ilc", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    let start = out.find(MARKER)? + MARKER.len();
    let end = start + out[start..].find(MARKER)?;
    Some(out[start..end].to_string()).filter(|p| !p.is_empty())
}

/// Environment for child processes: login PATH, without variables leaked from
/// the tools that launched fastDev (npm scripts, cargo, tauri dev).
pub fn child_env() -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = std::env::vars()
        .filter(|(key, _)| {
            !(key.starts_with("npm_")
                || key.starts_with("CARGO")
                || key.starts_with("TAURI_")
                || key.starts_with("RUSTUP_")
                || key == "INIT_CWD"
                || key == "NODE"
                || key == "PATH"
                || key == "FASTDEV_HOME")
        })
        .collect();
    env.push(("PATH".into(), login_path().to_string()));
    env
}

/// Absolute path of an executable on the login PATH.
pub fn which(tool: &str) -> Option<PathBuf> {
    login_path().split(':').map(|dir| PathBuf::from(dir).join(tool)).find(|p| p.is_file())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub name: String,
    pub found: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

pub fn detect(tool: &str) -> ToolInfo {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((at, info)) = cache.lock().expect("toolchain cache").get(tool)
        && at.elapsed() < CACHE_TTL
    {
        return info.clone();
    }
    let info = detect_uncached(tool);
    cache.lock().expect("toolchain cache").insert(tool.to_string(), (Instant::now(), info.clone()));
    info
}

pub fn clear_cache() {
    if let Some(cache) = CACHE.get() {
        cache.lock().expect("toolchain cache").clear();
    }
}

fn detect_uncached(tool: &str) -> ToolInfo {
    if tool == XCODE_CLT {
        return detect_xcode_clt();
    }
    let path = if tool == CHROME { chrome_binary() } else { which(tool) };
    let Some(path) = path else {
        return ToolInfo { name: tool.into(), found: false, version: None, path: None };
    };
    let arg = match tool {
        "go" => "version",
        "zip" | "unzip" => "-v",
        _ => "--version",
    };
    let output = Command::new(&path).arg(arg).envs(child_env()).stdin(Stdio::null()).output().ok();
    let text = output
        .map(|o| format!("{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
        .unwrap_or_default();
    ToolInfo {
        name: tool.into(),
        found: true,
        version: parse_version(&text),
        path: Some(path.to_string_lossy().into_owned()),
    }
}

fn chrome_binary() -> Option<PathBuf> {
    let bundle = "Google Chrome.app/Contents/MacOS/Google Chrome";
    [PathBuf::from("/Applications").join(bundle), home_dir().join("Applications").join(bundle)]
        .into_iter()
        .find(|p| p.is_file())
}

fn detect_xcode_clt() -> ToolInfo {
    let run = |program: &str, args: &[&str]| {
        Command::new(program)
            .args(args)
            .envs(child_env())
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let dir = run("/usr/bin/xcode-select", &["-p"]).filter(|dir| Path::new(dir).is_dir());
    let Some(dir) = dir else {
        return ToolInfo { name: XCODE_CLT.into(), found: false, version: None, path: None };
    };
    let version = run("/usr/sbin/pkgutil", &["--pkg-info=com.apple.pkg.CLTools_Executables"])
        .and_then(|text| text.lines().find_map(|l| l.strip_prefix("version:").and_then(parse_version)))
        .or_else(|| run("/usr/bin/xcodebuild", &["-version"]).and_then(|text| parse_version(&text)));
    ToolInfo { name: XCODE_CLT.into(), found: true, version, path: Some(dir) }
}

fn parse_version(text: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(\d+)\.(\d+)(?:\.(\d+))?").expect("valid regex"));
    let caps = re.captures(text)?;
    Some(format!("{}.{}.{}", &caps[1], &caps[2], caps.get(3).map_or("0", |m| m.as_str())))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementStatus {
    pub tool: String,
    /// Human name, e.g. "Node.js".
    pub label: String,
    pub requirement: String,
    pub found: bool,
    pub version: Option<String>,
    pub satisfied: bool,
    /// Why it is not satisfied, e.g. "Docker is installed but not running".
    pub problem: Option<String>,
    /// How to install or fix it.
    pub hint: String,
}

impl RequirementStatus {
    /// "Node.js >=24 (found 22.1.0): install from … " — for logs, warnings and MCP errors.
    pub fn describe(&self) -> String {
        let found = match (&self.problem, &self.version) {
            (Some(problem), _) => problem.clone(),
            (None, Some(version)) => format!("found {version}"),
            (None, None) => "not installed".into(),
        };
        format!("{} {} ({found}). {}", self.label, self.requirement, self.hint)
    }
}

/// Human name and install hint of well-known tools.
pub fn tool_help(tool: &str) -> (String, String) {
    let (label, hint) = match tool {
        "node" => ("Node.js", "Install from https://nodejs.org (or with fnm/nvm)."),
        "npm" => ("npm", "npm comes with Node.js: install Node.js from https://nodejs.org."),
        "pnpm" => ("pnpm", "Run `corepack enable pnpm` (Node.js 20+)."),
        "bun" => ("Bun", "Install from https://bun.sh."),
        "deno" => ("Deno", "Install from https://deno.com."),
        "docker" => {
            ("Docker", "Install Docker Desktop from https://www.docker.com/products/docker-desktop/ and start it.")
        }
        "git" => ("Git", "Run `xcode-select --install` or install from https://git-scm.com."),
        "php" => ("PHP", "Install with Homebrew (`brew install php`), or choose “Everything in Docker”."),
        "composer" => ("Composer", "Install from https://getcomposer.org/download/, or choose “Everything in Docker”."),
        "cargo" | "rustc" => ("Rust", "Install with rustup from https://rustup.rs."),
        "python3" | "python" => {
            ("Python", "Install from https://www.python.org/downloads/ or with `uv python install`.")
        }
        "uv" => ("uv", "Run `curl -LsSf https://astral.sh/uv/install.sh | sh`."),
        "go" => ("Go", "Install from https://go.dev/dl/."),
        XCODE_CLT => ("Xcode Command Line Tools", "Run `xcode-select --install`."),
        CHROME => ("Google Chrome", "Install from https://www.google.com/chrome/."),
        "zip" => ("zip", "zip comes with macOS; if it is missing, run `brew install zip`."),
        other => (other, ""),
    };
    let hint = if hint.is_empty() { format!("Install {tool} and make sure it is on your PATH.") } else { hint.into() };
    (label.to_string(), hint)
}

/// Whether the Docker daemon answers (cached like versions). `None` when docker is missing.
fn docker_running() -> Option<bool> {
    static CHECK: OnceLock<Mutex<Option<(Instant, bool)>>> = OnceLock::new();
    let cache = CHECK.get_or_init(|| Mutex::new(None));
    if let Some((at, running)) = *cache.lock().expect("docker check")
        && at.elapsed() < CACHE_TTL
    {
        return Some(running);
    }
    let path = which("docker")?;
    let mut child = Command::new(path)
        .args(["version", "--format", "{{.Server.Version}}"])
        .envs(child_env())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(4);
    let running = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    *cache.lock().expect("docker check") = Some((Instant::now(), running));
    Some(running)
}

pub fn check_requirements(requirements: &BTreeMap<String, String>) -> Vec<RequirementStatus> {
    requirements
        .iter()
        .map(|(tool, requirement)| {
            let info = detect(tool);
            let (label, mut hint) = tool_help(tool);
            let mut satisfied = info.found && satisfies(info.version.as_deref(), requirement);
            let mut problem = None;
            if info.found && !satisfied {
                problem = Some(format!(
                    "found {}, needs {requirement}",
                    info.version.as_deref().unwrap_or("an unknown version")
                ));
            }
            if satisfied && tool == "docker" && docker_running() == Some(false) {
                satisfied = false;
                problem = Some("Docker is installed but not running".into());
                hint = "Start Docker Desktop and try again.".into();
            }
            RequirementStatus {
                tool: tool.clone(),
                label,
                requirement: requirement.clone(),
                found: info.found,
                version: info.version,
                satisfied,
                problem,
                hint,
            }
        })
        .collect()
}

pub fn satisfies(version: Option<&str>, requirement: &str) -> bool {
    let requirement = requirement.trim();
    if requirement.is_empty() || requirement == "*" {
        return true;
    }
    let (Some(version), Ok(req)) = (version, semver::VersionReq::parse(requirement)) else {
        return false;
    };
    semver::Version::parse(version).map(|v| req.matches(&v)).unwrap_or(false)
}

pub fn overview() -> Vec<ToolInfo> {
    KNOWN_TOOLS.iter().map(|tool| detect(tool)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("v26.7.0\n").as_deref(), Some("26.7.0"));
        assert_eq!(parse_version("Docker version 29.7.2, build a7dcaa6").as_deref(), Some("29.7.2"));
        assert_eq!(parse_version("Python 3.9").as_deref(), Some("3.9.0"));
        assert_eq!(parse_version("nothing"), None);
    }

    #[test]
    fn checks_requirements() {
        assert!(satisfies(Some("26.7.0"), ">=24"));
        assert!(!satisfies(Some("22.1.0"), ">=24"));
        assert!(satisfies(None, ""));
        assert!(!satisfies(None, ">=1"));
    }
}
