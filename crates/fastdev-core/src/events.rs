//! Events pushed to the UI. The Tauri app implements [`EventSink`] by emitting to the webview.

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use indexmap::IndexMap;
use serde::Serialize;

use crate::jobs::JobSnapshot;
use crate::runner::RunInfo;
use crate::settings::Settings;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    JobUpdated {
        job: JobSnapshot,
    },
    /// `start` is the absolute index of the first line, so clients can merge batches
    /// with a full log fetched in parallel without duplicates.
    JobOutput {
        job_id: String,
        start: u64,
        lines: Vec<String>,
    },
    RunUpdated {
        run: RunInfo,
    },
    RunOutput {
        run_id: String,
        start: u64,
        lines: Vec<String>,
    },
    ProjectsChanged,
    LibraryChanged,
    SettingsChanged {
        settings: Box<Settings>,
    },
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: Event);
}

/// Sink that drops everything (CLI, tests).
pub struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _event: Event) {}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OutputKey {
    Job(String),
    Run(String),
}

/// Collects output lines and emits them in batches, so noisy commands
/// (npm install, dev servers) do not flood the webview with one event per line.
pub struct OutputHub {
    pending: Mutex<IndexMap<OutputKey, (u64, Vec<String>)>>,
    sink: Arc<dyn EventSink>,
}

impl OutputHub {
    pub fn new(sink: Arc<dyn EventSink>) -> Arc<Self> {
        let hub = Arc::new(Self { pending: Mutex::new(IndexMap::new()), sink });
        let weak: Weak<Self> = Arc::downgrade(&hub);
        std::thread::Builder::new()
            .name("fastdev-output".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_millis(80));
                    match weak.upgrade() {
                        Some(hub) => hub.flush(),
                        None => break,
                    }
                }
            })
            .expect("spawn output thread");
        hub
    }

    /// `index` is the absolute position of `line` in its log.
    pub fn push(&self, key: OutputKey, index: u64, line: String) {
        self.pending.lock().expect("output hub").entry(key).or_insert_with(|| (index, Vec::new())).1.push(line);
    }

    pub fn flush(&self) {
        let batch = std::mem::take(&mut *self.pending.lock().expect("output hub"));
        for (key, (start, lines)) in batch {
            let event = match key {
                OutputKey::Job(job_id) => Event::JobOutput { job_id, start, lines },
                OutputKey::Run(run_id) => Event::RunOutput { run_id, start, lines },
            };
            self.sink.emit(event);
        }
    }

    pub fn sink(&self) -> &Arc<dyn EventSink> {
        &self.sink
    }
}
