//! Runs project commands in their own process groups and keeps their output (SPEC §7.2).

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde::Serialize;

use crate::error::{Error, Result};
use crate::events::{Event, OutputHub, OutputKey};
use crate::manifest::Command;
use crate::{shell, util};

const MAX_LINES: usize = 5000;
const STOP_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Running,
    Exited,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunInfo {
    pub id: String,
    pub project_id: String,
    pub command: String,
    pub label: String,
    pub run: String,
    pub long: bool,
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub url: Option<String>,
}

/// Output of a run: the last MAX_LINES lines and how many were ever written.
#[derive(Default)]
struct LineBuffer {
    lines: VecDeque<String>,
    total: u64,
}

impl LineBuffer {
    fn push(&mut self, line: String) -> u64 {
        if self.lines.len() >= MAX_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
        self.total += 1;
        self.total - 1
    }
}

pub struct Run {
    info: Mutex<RunInfo>,
    finished: Condvar,
    lines: Mutex<LineBuffer>,
    pgid: i32,
    stop_requested: AtomicBool,
    /// Whether processes of the group are left: children may outlive the shell that started them.
    group_alive: AtomicBool,
}

impl Run {
    pub fn info(&self) -> RunInfo {
        self.info.lock().expect("run").clone()
    }

    pub fn is_running(&self) -> bool {
        self.info.lock().expect("run").status == RunStatus::Running
    }

    /// Running, or exited while processes it started are still alive.
    fn is_active(&self) -> bool {
        self.is_running() || self.group_alive.load(Ordering::SeqCst)
    }

    /// Last `count` lines and the absolute index of the first one.
    pub fn tail_with_start(&self, count: usize) -> (u64, Vec<String>) {
        let buffer = self.lines.lock().expect("run lines");
        let tail: Vec<String> = buffer.lines.iter().skip(buffer.lines.len().saturating_sub(count)).cloned().collect();
        (buffer.total - tail.len() as u64, tail)
    }

    pub fn tail(&self, count: usize) -> Vec<String> {
        self.tail_with_start(count).1
    }

    /// Forgets the kept lines; indexes keep growing so clients stay in sync.
    pub fn clear(&self) {
        self.lines.lock().expect("run lines").lines.clear();
    }

    /// Waits until the process exits or the timeout passes.
    pub fn wait(&self, timeout: Duration) -> RunInfo {
        let deadline = Instant::now() + timeout;
        let mut info = self.info.lock().expect("run");
        while info.status == RunStatus::Running {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            info = self.finished.wait_timeout(info, deadline - now).expect("run").0;
        }
        info.clone()
    }
}

pub struct Runner {
    /// Latest run per `project_id:command`.
    runs: Mutex<IndexMap<String, Arc<Run>>>,
    hub: Arc<OutputHub>,
}

impl Runner {
    pub fn new(hub: Arc<OutputHub>) -> Self {
        Self { runs: Mutex::new(IndexMap::new()), hub }
    }

    pub fn start(
        &self,
        project_id: &str,
        cwd: &Path,
        key: &str,
        command: &Command,
        url: Option<String>,
        inputs: &BTreeMap<String, String>,
    ) -> Result<RunInfo> {
        let slot = format!("{project_id}:{key}");
        // What the previous run left behind (e.g. a server whose shell exited) would hold its ports.
        let previous = self.runs.lock().expect("runs").get(&slot).cloned();
        if let Some(previous) = previous.filter(|r| !r.is_running() && r.is_active()) {
            terminate(&[previous], STOP_GRACE);
        }
        // Held until the new run is in the map, so concurrent starts of one command cannot both pass.
        let mut runs = self.runs.lock().expect("runs");
        if let Some(existing) = runs.get(&slot)
            && existing.is_running()
        {
            return Err(Error::conflict(format!("\"{key}\" is already running")));
        }
        if !cwd.is_dir() {
            return Err(Error::not_found(format!("project folder {} does not exist", cwd.display())));
        }
        let mut child = shell::command(&command.run, cwd)
            // Command inputs arrive as environment variables, never spliced into the script.
            .envs(inputs)
            .spawn()
            .map_err(|err| Error::command(format!("cannot start `{}`: {err}", command.run)))?;
        let pgid = i32::try_from(child.id()).map_err(|_| Error::internal("invalid pid"))?;
        let info = RunInfo {
            id: format!("run-{}", util::short_id()),
            project_id: project_id.to_string(),
            command: key.to_string(),
            label: if command.label.is_empty() { key.to_string() } else { command.label.clone() },
            run: command.run.clone(),
            long: command.long,
            status: RunStatus::Running,
            exit_code: None,
            started_at: util::now_rfc3339(),
            finished_at: None,
            url,
        };
        let run = Arc::new(Run {
            info: Mutex::new(info.clone()),
            finished: Condvar::new(),
            lines: Mutex::new(LineBuffer::default()),
            pgid,
            stop_requested: AtomicBool::new(false),
            group_alive: AtomicBool::new(true),
        });
        runs.insert(slot, run.clone());
        drop(runs);
        let header = format!("$ {}", command.run);
        {
            let mut buffer = run.lines.lock().expect("run lines");
            let index = buffer.push(header.clone());
            self.hub.push(OutputKey::Run(info.id.clone()), index, header);
        }

        let output = shell::stream_output(&mut child);
        let (reader_done_tx, reader_done_rx) = mpsc::channel::<()>();
        {
            let run = run.clone();
            let hub = self.hub.clone();
            let run_id = info.id.clone();
            std::thread::spawn(move || {
                for line in output {
                    let mut buffer = run.lines.lock().expect("run lines");
                    let index = buffer.push(line.clone());
                    hub.push(OutputKey::Run(run_id.clone()), index, line);
                }
                let _ = reader_done_tx.send(());
            });
        }
        {
            let run = run.clone();
            let hub = self.hub.clone();
            std::thread::spawn(move || {
                let status = child.wait();
                // Give the reader a moment to collect the last lines.
                let _ = reader_done_rx.recv_timeout(Duration::from_millis(500));
                hub.flush();
                let snapshot = {
                    let mut info = run.info.lock().expect("run");
                    info.finished_at = Some(util::now_rfc3339());
                    match status {
                        Ok(status) => {
                            info.exit_code = status.code();
                            info.status = if run.stop_requested.load(Ordering::SeqCst) {
                                RunStatus::Stopped
                            } else {
                                RunStatus::Exited
                            };
                        }
                        Err(_) => info.status = RunStatus::Failed,
                    }
                    info.clone()
                };
                run.finished.notify_all();
                hub.sink().emit(Event::RunUpdated { run: snapshot });
                // Watch what the shell left behind, so `stop` can still reach it.
                while shell::group_alive(run.pgid) {
                    std::thread::sleep(Duration::from_millis(250));
                }
                run.group_alive.store(false, Ordering::SeqCst);
            });
        }

        self.hub.sink().emit(Event::RunUpdated { run: info.clone() });
        Ok(info)
    }

    /// SIGTERM to the process group, SIGKILL after a grace period. Returns once every process of
    /// the group is gone: the shell often exits first, while its children are still shutting down.
    pub fn stop(&self, run_id: &str) -> Result<RunInfo> {
        let run = self.get(run_id).ok_or_else(|| Error::not_found(format!("run {run_id} not found")))?;
        terminate(std::slice::from_ref(&run), STOP_GRACE);
        Ok(run.info())
    }

    /// Stops every running process (app quit).
    pub fn stop_all(&self) {
        let runs: Vec<Arc<Run>> = self.runs.lock().expect("runs").values().cloned().collect();
        terminate(&runs, Duration::from_secs(3));
    }

    pub fn get(&self, run_id: &str) -> Option<Arc<Run>> {
        self.runs.lock().expect("runs").values().find(|r| r.info.lock().expect("run").id == run_id).cloned()
    }

    pub fn latest(&self, project_id: &str, command: &str) -> Option<Arc<Run>> {
        self.runs.lock().expect("runs").get(&format!("{project_id}:{command}")).cloned()
    }

    pub fn for_project(&self, project_id: &str) -> Vec<Arc<Run>> {
        self.runs
            .lock()
            .expect("runs")
            .values()
            .filter(|r| r.info.lock().expect("run").project_id == project_id)
            .cloned()
            .collect()
    }

    pub fn list(&self) -> Vec<RunInfo> {
        self.runs.lock().expect("runs").values().map(|r| r.info()).collect()
    }

    pub fn running(&self) -> Vec<RunInfo> {
        self.list().into_iter().filter(|r| r.status == RunStatus::Running).collect()
    }

    /// Forgets finished runs of a project (after removing it from the list).
    pub fn forget_project(&self, project_id: &str) {
        self.runs
            .lock()
            .expect("runs")
            .retain(|_, r| r.is_active() || r.info.lock().expect("run").project_id != project_id);
    }
}

/// Terminates the process groups of the active runs: SIGTERM, then SIGKILL after `grace`.
fn terminate(runs: &[Arc<Run>], grace: Duration) {
    let active: Vec<&Arc<Run>> = runs.iter().filter(|r| r.is_active()).collect();
    for run in &active {
        run.stop_requested.store(true, Ordering::SeqCst);
    }
    let groups: Vec<i32> = active.iter().map(|r| r.pgid).collect();
    shell::terminate_groups(&groups, grace);
    // The waiter threads record the final status right after the shell is reaped.
    let deadline = Instant::now() + Duration::from_secs(2);
    for run in &active {
        run.wait(deadline.saturating_duration_since(Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::NullSink;

    fn cmd(run: &str, long: bool) -> Command {
        Command {
            label: String::new(),
            run: run.into(),
            description: String::new(),
            long,
            url: None,
            primary: false,
            feature: None,
            inputs: Vec::new(),
        }
    }

    #[test]
    fn passes_inputs_as_environment() {
        let runner = Runner::new(OutputHub::new(Arc::new(NullSink)));
        let dir = tempfile::tempdir().unwrap();
        let inputs = BTreeMap::from([("NAME".to_string(), "Tab \"Organizer\"; rm -rf x".to_string())]);
        let info = runner.start("p1", dir.path(), "new", &cmd("printf '%s' \"$NAME\"", false), None, &inputs).unwrap();
        let run = runner.get(&info.id).unwrap();
        run.wait(Duration::from_secs(10));
        assert_eq!(run.tail(5).last().map(String::as_str), Some("Tab \"Organizer\"; rm -rf x"));
    }

    #[test]
    fn runs_and_stops_process_groups() {
        let runner = Runner::new(OutputHub::new(Arc::new(NullSink)));
        let dir = tempfile::tempdir().unwrap();

        let info =
            runner.start("p1", dir.path(), "hello", &cmd("echo hello; exit 3", false), None, &BTreeMap::new()).unwrap();
        let done = runner.get(&info.id).unwrap().wait(Duration::from_secs(5));
        assert_eq!(done.status, RunStatus::Exited);
        assert_eq!(done.exit_code, Some(3));
        assert!(runner.get(&info.id).unwrap().tail(10).contains(&"hello".to_string()));

        // A child that ignores the parent's exit must die with the group.
        let info =
            runner.start("p1", dir.path(), "dev", &cmd("sleep 30 & sleep 30", true), None, &BTreeMap::new()).unwrap();
        assert!(runner.start("p1", dir.path(), "dev", &cmd("true", true), None, &BTreeMap::new()).is_err());
        let stopped = runner.stop(&info.id).unwrap();
        assert_eq!(stopped.status, RunStatus::Stopped);
        assert!(runner.running().is_empty());
    }

    #[test]
    fn stop_waits_for_children_that_outlive_the_shell() {
        let runner = Runner::new(OutputHub::new(Arc::new(NullSink)));
        let dir = tempfile::tempdir().unwrap();
        // The shell dies on SIGTERM; its child ignores it and must be killed with the group.
        let script = "(trap '' TERM; sleep 30) & wait";
        let info = runner.start("p1", dir.path(), "dev", &cmd(script, true), None, &BTreeMap::new()).unwrap();
        let pgid = runner.get(&info.id).unwrap().pgid;
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(runner.stop(&info.id).unwrap().status, RunStatus::Stopped);
        assert!(!shell::group_alive(pgid));
    }

    #[test]
    fn starts_one_run_per_command_under_concurrent_calls() {
        let runner = Arc::new(Runner::new(OutputHub::new(Arc::new(NullSink))));
        let dir = tempfile::tempdir().unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let (runner, barrier, dir) = (runner.clone(), barrier.clone(), dir.path().to_path_buf());
                std::thread::spawn(move || {
                    barrier.wait();
                    runner.start("p1", &dir, "dev", &cmd("sleep 30", true), None, &BTreeMap::new()).is_ok()
                })
            })
            .collect();
        let started = threads.into_iter().map(|t| t.join().unwrap()).filter(|ok| *ok).count();
        assert_eq!(started, 1);
        let pgids: Vec<i32> = runner.runs.lock().unwrap().values().map(|r| r.pgid).collect();
        runner.stop_all();
        assert!(pgids.iter().all(|pgid| !shell::group_alive(*pgid)));
    }
}
