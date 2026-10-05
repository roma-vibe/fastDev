//! Long-running operations with steps, a log and a result (create project, verify, updates).

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::events::{Event, OutputHub, OutputKey};
use crate::util;

const MAX_LOG_LINES: usize = 5000;
const MAX_JOBS: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStep {
    pub label: String,
    pub status: StepStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSnapshot {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub status: JobStatus,
    pub steps: Vec<JobStep>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub result: Option<Value>,
    pub error: Option<Error>,
    pub warnings: Vec<String>,
    /// Translations of skeleton-provided step labels: language → English text → translation.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub translations: BTreeMap<String, BTreeMap<String, String>>,
}

struct JobState {
    snapshot: JobSnapshot,
    log: VecDeque<String>,
    /// Lines ever written (the log keeps only the last MAX_LOG_LINES).
    total: u64,
}

pub struct Job {
    state: Mutex<JobState>,
    done: Condvar,
}

impl Job {
    pub fn snapshot(&self) -> JobSnapshot {
        self.state.lock().expect("job").snapshot.clone()
    }

    /// Last `lines` log lines and the absolute index of the first one.
    pub fn log_tail(&self, lines: usize) -> (u64, Vec<String>) {
        let state = self.state.lock().expect("job");
        let skip = state.log.len().saturating_sub(lines);
        let tail: Vec<String> = state.log.iter().skip(skip).cloned().collect();
        (state.total - tail.len() as u64, tail)
    }

    /// Waits until the job finishes or the timeout passes, then returns its snapshot.
    pub fn wait(&self, timeout: Duration) -> JobSnapshot {
        let deadline = Instant::now() + timeout;
        let mut state = self.state.lock().expect("job");
        while state.snapshot.status == JobStatus::Running {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            state = self.done.wait_timeout(state, deadline - now).expect("job").0;
        }
        state.snapshot.clone()
    }
}

/// Handle given to the job body for reporting progress.
pub struct JobCtx {
    id: String,
    job: Arc<Job>,
    hub: Arc<OutputHub>,
}

impl JobCtx {
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Marks the running step done and starts `label` (appended when not declared).
    pub fn step(&self, label: &str) {
        let snapshot = {
            let mut state = self.job.state.lock().expect("job");
            for step in &mut state.snapshot.steps {
                if step.status == StepStatus::Running {
                    step.status = StepStatus::Done;
                }
            }
            match state.snapshot.steps.iter_mut().find(|s| s.label == label && s.status == StepStatus::Pending) {
                Some(step) => step.status = StepStatus::Running,
                None => state.snapshot.steps.push(JobStep { label: label.to_string(), status: StepStatus::Running }),
            }
            state.snapshot.clone()
        };
        self.log(format!("▸ {label}"));
        self.hub.sink().emit(Event::JobUpdated { job: snapshot });
    }

    pub fn skip(&self, label: &str) {
        let snapshot = {
            let mut state = self.job.state.lock().expect("job");
            if let Some(step) =
                state.snapshot.steps.iter_mut().find(|s| s.label == label && s.status == StepStatus::Pending)
            {
                step.status = StepStatus::Skipped;
            }
            state.snapshot.clone()
        };
        self.hub.sink().emit(Event::JobUpdated { job: snapshot });
    }

    pub fn warn(&self, message: impl Into<String>) {
        let message = message.into();
        self.log(format!("⚠ {message}"));
        let snapshot = {
            let mut state = self.job.state.lock().expect("job");
            state.snapshot.warnings.push(message);
            state.snapshot.clone()
        };
        self.hub.sink().emit(Event::JobUpdated { job: snapshot });
    }

    pub fn log(&self, line: impl Into<String>) {
        let line = line.into();
        let mut state = self.job.state.lock().expect("job");
        if state.log.len() >= MAX_LOG_LINES {
            state.log.pop_front();
        }
        state.log.push_back(line.clone());
        let index = state.total;
        state.total += 1;
        // Pushed under the lock so batches keep the order of indexes.
        self.hub.push(OutputKey::Job(self.id.clone()), index, line);
    }
}

pub struct JobManager {
    jobs: Mutex<IndexMap<String, Arc<Job>>>,
    hub: Arc<OutputHub>,
}

impl JobManager {
    pub fn new(hub: Arc<OutputHub>) -> Self {
        Self { jobs: Mutex::new(IndexMap::new()), hub }
    }

    /// Starts `body` on a background thread.
    pub fn start<F>(&self, kind: &str, title: &str, steps: &[&str], body: F) -> Arc<Job>
    where
        F: FnOnce(&JobCtx) -> Result<Value> + Send + 'static,
    {
        self.start_translated(kind, title, steps, BTreeMap::new(), body)
    }

    /// Like [`start`](Self::start), with translations of step labels that come from a skeleton.
    pub fn start_translated<F>(
        &self,
        kind: &str,
        title: &str,
        steps: &[&str],
        translations: BTreeMap<String, BTreeMap<String, String>>,
        body: F,
    ) -> Arc<Job>
    where
        F: FnOnce(&JobCtx) -> Result<Value> + Send + 'static,
    {
        let id = format!("job-{}", util::short_id());
        let snapshot = JobSnapshot {
            id: id.clone(),
            kind: kind.to_string(),
            title: title.to_string(),
            status: JobStatus::Running,
            steps: steps.iter().map(|s| JobStep { label: s.to_string(), status: StepStatus::Pending }).collect(),
            started_at: util::now_rfc3339(),
            finished_at: None,
            result: None,
            error: None,
            warnings: Vec::new(),
            translations,
        };
        let job = Arc::new(Job {
            state: Mutex::new(JobState { snapshot: snapshot.clone(), log: VecDeque::new(), total: 0 }),
            done: Condvar::new(),
        });
        {
            let mut jobs = self.jobs.lock().expect("jobs");
            if jobs.len() >= MAX_JOBS
                && let Some(key) =
                    jobs.iter().find(|(_, j)| j.snapshot().status != JobStatus::Running).map(|(k, _)| k.clone())
            {
                jobs.shift_remove(&key);
            }
            jobs.insert(id.clone(), job.clone());
        }
        self.hub.sink().emit(Event::JobUpdated { job: snapshot });

        let ctx = JobCtx { id: id.clone(), job: job.clone(), hub: self.hub.clone() };
        std::thread::Builder::new()
            .name(format!("fastdev-{kind}"))
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(&ctx)))
                    .unwrap_or_else(|_| Err(Error::internal("the job crashed")));
                if let Err(err) = &outcome {
                    ctx.log(format!("✗ {}", err.message));
                }
                ctx.hub.flush();
                let snapshot = {
                    let mut state = ctx.job.state.lock().expect("job");
                    let snap = &mut state.snapshot;
                    let failed = outcome.is_err();
                    for step in &mut snap.steps {
                        if step.status == StepStatus::Running {
                            step.status = if failed { StepStatus::Failed } else { StepStatus::Done };
                        }
                        if step.status == StepStatus::Pending && !failed {
                            step.status = StepStatus::Skipped;
                        }
                    }
                    snap.finished_at = Some(util::now_rfc3339());
                    match outcome {
                        Ok(value) => {
                            snap.status = JobStatus::Succeeded;
                            snap.result = Some(value);
                        }
                        Err(err) => {
                            snap.status = JobStatus::Failed;
                            snap.error = Some(err);
                        }
                    }
                    snap.clone()
                };
                ctx.job.done.notify_all();
                ctx.hub.sink().emit(Event::JobUpdated { job: snapshot });
            })
            .expect("spawn job thread");
        job
    }

    pub fn get(&self, id: &str) -> Option<Arc<Job>> {
        self.jobs.lock().expect("jobs").get(id).cloned()
    }

    pub fn list(&self) -> Vec<JobSnapshot> {
        self.jobs.lock().expect("jobs").values().rev().map(|j| j.snapshot()).collect()
    }
}
