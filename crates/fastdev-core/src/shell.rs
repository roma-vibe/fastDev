//! Running shell commands with the login PATH, in their own process group.

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;

use crate::error::{Error, Result};
use crate::toolchain;

/// Process groups of the commands jobs run to completion (setup, verification, cleanup…), so
/// quitting can stop them. Project runs are tracked by the `Runner`.
static JOB_GROUPS: Mutex<BTreeSet<i32>> = Mutex::new(BTreeSet::new());

/// `/bin/sh -c <script>` in `cwd` with the sanitised environment and its own process group.
pub fn command(script: &str, cwd: &Path) -> Command {
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(script)
        .current_dir(cwd)
        .env_clear()
        .envs(toolchain::child_env())
        .env("FORCE_COLOR", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    cmd
}

/// Spawns reader threads for stdout and stderr; lines arrive on the returned channel
/// (ANSI escape codes removed). The channel closes when both streams end.
pub fn stream_output(child: &mut Child) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        spawn_reader(stdout, tx.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_reader(stderr, tx);
    }
    rx
}

fn spawn_reader(stream: impl Read + Send + 'static, tx: mpsc::Sender<String>) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let clean = strip_ansi_escapes::strip(&buf);
                    let text = String::from_utf8_lossy(&clean);
                    // Progress bars use \r; keep the last state of the line.
                    let line = text.trim_end_matches(['\n', '\r']).rsplit('\r').next().unwrap_or_default().to_string();
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            }
        }
    });
}

/// Runs `script` to completion (non-interactive, `CI=true`), passing every output line to `on_line`.
/// Returns the exit code.
pub fn run_streaming(script: &str, cwd: &Path, on_line: &mut dyn FnMut(String)) -> Result<i32> {
    run_to_end(command(script, cwd), script, on_line)
}

/// Runs a `cleanup` script (of an option or a preview) in `project` with the project's `.env` values
/// in its environment, so it can name what the project created (`$COMPOSE_PROJECT_NAME`, `$APP_SLUG`).
pub fn run_cleanup(script: &str, project: &Path, on_line: &mut dyn FnMut(String)) -> Result<()> {
    let mut cmd = command(script, project);
    for (key, value) in crate::envfile::read_values(&project.join(".env")) {
        if !matches!(key.as_str(), "PATH" | "HOME" | "SHELL" | "USER" | "LOGNAME" | "TMPDIR" | "CI") {
            cmd.env(key, value);
        }
    }
    match run_to_end(cmd, script, on_line)? {
        0 => Ok(()),
        code => Err(Error::command(format!("`{script}` exited with code {code}"))),
    }
}

fn run_to_end(mut cmd: Command, script: &str, on_line: &mut dyn FnMut(String)) -> Result<i32> {
    let mut child = cmd
        // Exactly "true": some tools (e.g. Tauri's DMG bundler) check for this value.
        .env("CI", "true")
        .spawn()
        .map_err(|err| Error::command(format!("cannot start `{script}`: {err}")))?;
    let pgid = i32::try_from(child.id()).map_err(|_| Error::internal("invalid pid"))?;
    JOB_GROUPS.lock().expect("job groups").insert(pgid);
    let lines = stream_output(&mut child);
    for line in lines {
        on_line(line);
    }
    let status = child.wait();
    JOB_GROUPS.lock().expect("job groups").remove(&pgid);
    let status = status.map_err(|err| Error::command(err.to_string()))?;
    Ok(status.code().unwrap_or(-1))
}

/// Whether any process of the group is still alive.
pub fn group_alive(pgid: i32) -> bool {
    killpg(Pid::from_raw(pgid), None).is_ok()
}

/// Waits until the group has no processes left; `false` when `deadline` came first.
pub fn wait_group(pgid: i32, deadline: Instant) -> bool {
    while group_alive(pgid) {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    true
}

/// `SIGTERM` to every group, `SIGKILL` to the groups that still have processes after `grace`.
pub fn terminate_groups(groups: &[i32], grace: Duration) {
    for &pgid in groups {
        let _ = killpg(Pid::from_raw(pgid), Signal::SIGTERM);
    }
    let deadline = Instant::now() + grace;
    let stubborn: Vec<i32> = groups.iter().copied().filter(|&pgid| !wait_group(pgid, deadline)).collect();
    for &pgid in &stubborn {
        let _ = killpg(Pid::from_raw(pgid), Signal::SIGKILL);
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    for &pgid in &stubborn {
        wait_group(pgid, deadline);
    }
}

/// Stops the processes of all running jobs (app quit, CLI interrupted).
pub fn stop_all() {
    let groups: Vec<i32> = JOB_GROUPS.lock().expect("job groups").iter().copied().collect();
    terminate_groups(&groups, Duration::from_secs(3));
}

/// Like [`run_streaming`] but fails on a non-zero exit code.
pub fn run_checked(script: &str, cwd: &Path, on_line: &mut dyn FnMut(String)) -> Result<()> {
    match run_streaming(script, cwd, on_line)? {
        0 => Ok(()),
        code => Err(Error::command(format!("`{script}` exited with code {code}"))),
    }
}

/// Runs a program directly and captures its output (for git and other quick calls).
pub fn capture(program: &str, args: &[&str], cwd: &Path) -> Result<(i32, String)> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(toolchain::child_env())
        .stdin(Stdio::null())
        .output()
        .map_err(|err| Error::command(format!("cannot start {program}: {err}")))?;
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    Ok((output.status.code().unwrap_or(-1), text.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_sees_env_values() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".env"), "APP_SLUG='my app'\nPATH=/nowhere\n").unwrap();
        let mut lines = Vec::new();
        run_cleanup("echo \"[$APP_SLUG]\"; command -v ls >/dev/null && echo path-ok", tmp.path(), &mut |l| {
            lines.push(l)
        })
        .unwrap();
        assert_eq!(lines, ["[my app]", "path-ok"]);
        // An unset variable fails a guarded script instead of expanding to "".
        assert!(run_cleanup("rm -rf \"$HOME/nowhere/${MISSING:?}\"", tmp.path(), &mut |_| {}).is_err());
    }

    #[test]
    fn tracks_job_processes_until_they_end() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let (tx, rx) = mpsc::channel();
        // The shell prints its pid (= its process group) and waits for a child that ignores SIGTERM.
        let job = std::thread::spawn(move || {
            run_streaming("echo $$; (trap '' TERM; sleep 30) & wait", &dir, &mut |line| {
                let _ = tx.send(line);
            })
        });
        let pgid: i32 = rx.recv_timeout(Duration::from_secs(10)).unwrap().parse().unwrap();
        assert!(JOB_GROUPS.lock().unwrap().contains(&pgid));
        let started = Instant::now();
        terminate_groups(&[pgid], Duration::from_millis(300));
        assert!(!group_alive(pgid), "the child that ignores SIGTERM is killed");
        assert_ne!(job.join().unwrap().unwrap(), 0);
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(!JOB_GROUPS.lock().unwrap().contains(&pgid));
    }
}
