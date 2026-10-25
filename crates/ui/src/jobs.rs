//! Background work. Anything that runs a command happens on a thread so the window never freezes;
//! the result comes back as a [`Done`] message the next frame picks up.

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use poddojo_core::{CheckResult, CommandOutput, ExamAttempt};
use poddojo_runner::cluster::Status;
use poddojo_runner::doctor::Tool;
use poddojo_runner::explore::{KubeObject, Layer, Row};
use poddojo_runner::targets::{EngineInfo, Target};

pub type Live = Arc<Mutex<String>>;

pub enum Done {
    Targets(Result<Vec<Target>, String>),
    Engine {
        target: String,
        info: Result<EngineInfo, String>,
    },
    Cluster(Status),
    Console {
        entry: usize,
        out: CommandOutput,
    },
    Prepared {
        lesson: String,
        ok: bool,
        log: String,
    },
    Reset {
        lesson: String,
        log: String,
    },
    Checked {
        lesson: String,
        step: usize,
        results: Vec<CheckResult>,
    },
    Docker {
        containers: Result<Vec<Row>, String>,
        images: Result<Vec<Row>, String>,
        volumes: Result<Vec<Row>, String>,
        networks: Result<Vec<Row>, String>,
    },
    Layers {
        image: String,
        layers: Result<Vec<Layer>, String>,
    },
    Namespaces(Result<Vec<String>, String>),
    Kube {
        kind: String,
        objects: Result<Vec<KubeObject>, String>,
    },
    Doctor(Vec<Tool>),
    ClusterChanged {
        ok: bool,
        log: String,
        created_on: Option<String>,
    },
    ExamStarted {
        exam: String,
        ok: bool,
        log: String,
    },
    ExamGraded(ExamAttempt),
}

pub struct Running {
    pub label: String,
    pub started: Instant,
    pub live: Live,
}

pub struct Jobs {
    tx: Sender<(u64, Done)>,
    rx: Receiver<(u64, Done)>,
    next: u64,
    pub running: BTreeMap<u64, Running>,
}

impl Default for Jobs {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self { tx, rx, next: 1, running: BTreeMap::new() }
    }
}

impl Jobs {
    /// Run `work` on a thread; `live` receives output as it streams.
    pub fn spawn(
        &mut self,
        ctx: &egui::Context,
        label: impl Into<String>,
        work: impl FnOnce(Live) -> Done + Send + 'static,
    ) -> u64 {
        let id = self.next;
        self.next += 1;
        let live: Live = Arc::new(Mutex::new(String::new()));
        self.running.insert(id, Running { label: label.into(), started: Instant::now(), live: live.clone() });
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let done = work(live);
            let _ = tx.send((id, done));
            ctx.request_repaint();
        });
        id
    }

    pub fn poll(&mut self) -> Vec<Done> {
        let mut out = Vec::new();
        while let Ok((id, done)) = self.rx.try_recv() {
            self.running.remove(&id);
            out.push(done);
        }
        out
    }

    pub fn busy(&self, label_prefix: &str) -> bool {
        self.running.values().any(|r| r.label.starts_with(label_prefix))
    }

    pub fn live(&self, id: u64) -> Option<String> {
        self.running.get(&id).and_then(|r| r.live.lock().ok().map(|l| l.clone()))
    }
}
