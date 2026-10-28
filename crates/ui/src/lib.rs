//! PodDojo's interface. Thin by design: it reads `poddojo-core` state, asks `poddojo-runner` to do
//! things on background threads (`jobs`), and draws the result.

pub mod icons;
pub mod jobs;
mod logo;
pub mod mark;
pub mod markdown;
mod pages;
pub mod terminal;
mod theme;

pub use logo::icon_rgba;

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use egui::{Color32, RichText};
use poddojo_core::{Catalog, CheckResult, ExamAttempt, Progress, now};
use poddojo_runner::cluster::{self, ClusterRecord, Status};
use poddojo_runner::doctor::Tool;
use poddojo_runner::explore::{KubeObject, Layer, Row};
use poddojo_runner::profiles::{self, Profile, Profiles};
use poddojo_runner::targets::{self, EngineInfo, Target};
use poddojo_runner::{LabEnv, Paths, Settings};

use icons::Icon;
use jobs::{Done, Jobs};
use terminal::Terminal;
use theme::Tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Welcome,
    Home,
    Learn,
    Docker,
    Cluster,
    Exams,
    Review,
    Reference,
    Progress,
    Setup,
}

impl Page {
    const ALL: [(Page, Icon, &'static str); 9] = [
        (Page::Home, Icon::Home, "Home"),
        (Page::Learn, Icon::Book, "Learn"),
        (Page::Review, Icon::Repeat, "Review"),
        (Page::Exams, Icon::Stopwatch, "Exams"),
        (Page::Reference, Icon::Docs, "Reference"),
        (Page::Docker, Icon::Container, "Docker explorer"),
        (Page::Cluster, Icon::Cluster, "Lab cluster"),
        (Page::Progress, Icon::Chart, "Progress"),
        (Page::Setup, Icon::Gear, "Setup"),
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LessonTab {
    #[default]
    Concepts,
    Quiz,
    Lab,
}

#[derive(Default)]
pub struct LearnState {
    pub lesson: Option<String>,
    pub tab: LessonTab,
    pub answers: Vec<Option<usize>>,
    pub submitted: bool,
    pub results: BTreeMap<usize, Vec<CheckResult>>,
    pub hints: BTreeMap<usize, usize>,
    pub solutions: BTreeSet<usize>,
    pub opened_at: Option<Instant>,
    pub lab_log: String,
    pub scroll_to_top: bool,
    /// Lab steps the learner unfolded besides the current one.
    pub expanded: BTreeSet<usize>,
    /// The lab was prepared in this session.
    pub prepared: bool,
    /// Section of the concepts to scroll to (from the side rail's "On this page").
    pub scroll_to_section: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DockerTab {
    #[default]
    Containers,
    Images,
    Volumes,
    Networks,
}

#[derive(Default)]
pub struct DockerState {
    pub tab: DockerTab,
    pub containers: Vec<Row>,
    pub images: Vec<Row>,
    pub volumes: Vec<Row>,
    pub networks: Vec<Row>,
    pub error: Option<String>,
    pub loaded_for: Option<String>,
    pub filter: String,
    pub pull: String,
    pub image: Option<String>,
    pub layers: Vec<Layer>,
    pub layers_error: Option<String>,
    pub confirm_remove: Option<String>,
    /// Show everything on the machine, not only what PodDojo's labs created.
    pub show_all: bool,
    /// `docker stop` on a container PodDojo did not create, waiting for confirmation.
    pub confirm_stop: Option<String>,
}

#[derive(Default)]
pub struct KubeState {
    pub namespaces: Vec<String>,
    /// `None`: all namespaces.
    pub namespace: Option<String>,
    pub kind: String,
    pub objects: Vec<KubeObject>,
    pub error: Option<String>,
    pub selected: Option<(String, String)>,
    pub confirm_delete: bool,
    pub log: String,
}

#[derive(Default)]
pub struct WelcomeState {
    /// Each profile's progress, read when the welcome screen opens.
    pub loaded: Option<Vec<(Profile, Progress)>>,
    pub new_name: String,
    pub creating: bool,
    pub confirm_remove: Option<String>,
}

#[derive(Default)]
pub struct ExamState {
    pub task: usize,
    /// Time ran out and grading was started automatically.
    pub auto_graded: bool,
    pub result: Option<ExamAttempt>,
    pub reveal: BTreeSet<String>,
    pub confirm_finish: bool,
    /// The task list drawer is open (narrow windows fold the list away).
    pub drawer: bool,
    pub log: String,
}

pub struct App {
    pub profiles: Profiles,
    /// Who is studying; `None` while the welcome screen is shown.
    pub profile: Option<Profile>,
    pub welcome: WelcomeState,
    /// Lesson list filter on the Learn page.
    pub tree_filter: String,
    /// The lesson list is folded away to give the lesson more room.
    pub tree_hidden: bool,
    pub paths: Paths,
    pub settings: Settings,
    pub catalog: Catalog,
    pub content_errors: Vec<String>,
    pub progress: Progress,
    pub page: Page,
    pub t: Tokens,
    pub jobs: Jobs,
    pub term: Terminal,
    pub targets: Vec<Target>,
    pub targets_error: Option<String>,
    pub engine: BTreeMap<String, Result<EngineInfo, String>>,
    pub cluster: Option<Status>,
    pub cluster_record: Option<ClusterRecord>,
    pub cluster_log: String,
    pub doctor: Option<Vec<Tool>>,
    pub learn: LearnState,
    pub docker: DockerState,
    pub kube: KubeState,
    pub exam: ExamState,
    pub review: pages::ReviewState,
    pub reference: pages::ReferenceState,
    pub toasts: Vec<(String, bool, Instant)>,
    pub logo: Option<egui::TextureHandle>,
    last_minute: Instant,
    started: bool,
    /// Modification time of progress.json when we last read or wrote it: the `dojo` CLI may change
    /// it while the app is open, and the app must build on that instead of overwriting it.
    progress_stamp: Option<std::time::SystemTime>,
    demo_ran: bool,
    /// A short celebration overlay: (started, headline, detail).
    pub celebrate: Option<(Instant, String, String)>,
    /// Machines were tested since the Setup page was opened.
    pub setup_probed: bool,
    /// `PODDOJO_SCREENSHOT=<png>`: capture this window after the first jobs settle, then quit
    /// (with `PODDOJO_PAGE=home|learn:<lesson>[:quiz|:lab]|review[:session]|reference[:<id>]|docker|cluster|exams|progress|setup`).
    shot: Option<(std::path::PathBuf, Instant, bool)>,
}

impl App {
    pub fn new(ctx: &egui::Context) -> Self {
        let paths = Paths::discover();
        let settings = Settings::load(&paths.settings());
        let (catalog, content_errors) = match Catalog::load() {
            Ok(c) => (c, Vec::new()),
            Err(errs) => (Catalog::default(), errs.iter().map(|e| e.to_string()).collect()),
        };
        let profiles = Profiles::load(&paths);
        let t = Tokens::get(settings.dark);
        theme::apply(ctx, &t, settings.zoom, settings.motion);
        let cluster_record = ClusterRecord::load(&paths);
        let mut app = Self {
            tree_filter: String::new(),
            tree_hidden: std::env::var_os("PODDOJO_TREE_HIDDEN").is_some(),
            paths,
            settings,
            catalog,
            content_errors,
            progress: Progress::default(),
            profiles,
            profile: None,
            welcome: WelcomeState::default(),
            page: Page::Welcome,
            t,
            jobs: Jobs::default(),
            term: Terminal::default(),
            targets: Vec::new(),
            targets_error: None,
            engine: BTreeMap::new(),
            cluster: None,
            cluster_record,
            cluster_log: String::new(),
            doctor: None,
            learn: LearnState::default(),
            docker: DockerState::default(),
            kube: KubeState { kind: "pods".into(), ..Default::default() },
            exam: ExamState::default(),
            review: pages::ReviewState::default(),
            reference: pages::ReferenceState::default(),
            toasts: Vec::new(),
            logo: logo::texture(ctx),
            last_minute: Instant::now(),
            started: false,
            progress_stamp: None,
            demo_ran: false,
            celebrate: None,
            setup_probed: false,
            shot: std::env::var_os("PODDOJO_SCREENSHOT").map(|p| (std::path::PathBuf::from(p), Instant::now(), false)),
        };
        // Development aid: `PODDOJO_PROFILE=<id|name|guest>` skips the welcome screen.
        if let Ok(who) = std::env::var("PODDOJO_PROFILE") {
            let profile =
                if who == profiles::GUEST { Some(Profile::guest()) } else { app.profiles.find(&who).cloned() };
            if let Some(p) = profile {
                app.enter(p);
            }
        }
        if let Ok(page) = std::env::var("PODDOJO_PAGE") {
            app.goto(&page);
        }
        app
    }

    /// Start studying as `profile`: load its progress and open Home (or the exam in progress).
    pub fn enter(&mut self, profile: Profile) {
        if self.profile.is_some() {
            self.save_progress();
        }
        self.paths = if profile.is_guest() { self.profiles.start_guest() } else { self.profiles.paths_for(&profile) };
        let _ = self.profiles.set_last(&profile.id);
        self.progress = match Progress::load(&self.paths.progress()) {
            Ok(p) => p,
            Err(e) => {
                // Keep the unreadable file instead of overwriting it with an empty history.
                let backup = self.paths.profile_dir().join(format!("progress.json.unreadable-{}", now()));
                let kept = std::fs::copy(self.paths.progress(), &backup).is_ok();
                self.toast(
                    format!(
                        "Your progress file could not be read ({e}). {}",
                        if kept {
                            format!("A copy was kept at {}.", backup.display())
                        } else {
                            "It was left untouched.".into()
                        }
                    ),
                    false,
                );
                Progress::default()
            }
        };
        self.progress_stamp = progress_mtime(&self.paths);
        self.learn = LearnState::default();
        self.exam = ExamState::default();
        self.exam.task = self.progress.active_exam.as_ref().map_or(0, |a| a.current_task);
        self.jobs.owner = profile.id.clone();
        self.page = if self.progress.active_exam.is_some() { Page::Exams } else { Page::Home };
        if profile.is_guest() {
            self.toast("Guest mode: practise freely, nothing is kept for next time", true);
        }
        self.profile = Some(profile);
    }

    /// Back to the welcome screen to pick someone else.
    pub fn switch_profile(&mut self) {
        self.save_progress();
        self.profile = None;
        self.welcome = WelcomeState::default();
        self.page = Page::Welcome;
    }

    // ---------------------------------------------------------------------------------------
    // State helpers

    pub fn target(&self) -> Option<Target> {
        targets::choose(&self.targets, &self.settings.target)
    }

    pub fn target_name(&self) -> String {
        self.target().map(|t| t.name).unwrap_or_else(|| "no machine".into())
    }

    /// The environment for work in `id`'s lab folder on the chosen machine.
    pub fn env_for(&self, id: &str) -> Option<LabEnv> {
        let workdir = self.paths.workdir(id);
        self.paths.ensure(&workdir).ok()?;
        self.target().map(|t| t.env(&self.paths, workdir))
    }

    /// Where the terminal runs: the open exam, else the open lesson, else a scratch folder.
    pub fn terminal_env(&self) -> Option<LabEnv> {
        let id = if let Some(e) = &self.progress.active_exam {
            format!("exam-{}", e.exam_id)
        } else if let Some(d) = self.review.drill.as_ref().filter(|_| self.page == Page::Review) {
            format!("exam-{}", d.exam)
        } else if let Some(l) = &self.learn.lesson {
            l.clone()
        } else {
            "playground".into()
        };
        self.env_for(&id)
    }

    pub fn save_progress(&mut self) {
        if self.profile.is_none() {
            return;
        }
        if let Err(e) = self.progress.save(&self.paths.progress()) {
            self.toast(format!("Could not save progress: {e}"), false);
        }
        self.progress_stamp = progress_mtime(&self.paths);
    }

    /// Pick up progress written by the CLI (or another window) since we last looked.
    fn sync_progress(&mut self) {
        if self.profile.is_none() {
            return;
        }
        let stamp = progress_mtime(&self.paths);
        if stamp.is_some() && stamp != self.progress_stamp {
            if let Ok(p) = Progress::load(&self.paths.progress()) {
                self.progress = p;
            }
            self.progress_stamp = stamp;
        }
    }

    pub fn save_settings(&mut self) {
        if let Err(e) = self.settings.save(&self.paths.settings()) {
            self.toast(format!("Could not save settings: {e}"), false);
        }
    }

    pub fn toast(&mut self, text: impl Into<String>, ok: bool) {
        self.toasts.push((text.into(), ok, Instant::now()));
    }

    pub fn cluster_running(&self) -> bool {
        self.cluster.as_ref().is_some_and(Status::is_running)
    }

    // ---------------------------------------------------------------------------------------
    // Background work

    pub fn refresh_targets(&mut self, ctx: &egui::Context) {
        if self.jobs.busy("targets") {
            return;
        }
        let paths = self.paths.clone();
        self.jobs.spawn(ctx, "targets", move |_| Done::Targets(targets::list(&paths)));
    }

    pub fn probe_target(&mut self, ctx: &egui::Context, target: &Target) {
        let label = format!("probe {}", target.name);
        if self.jobs.busy(&label) {
            return;
        }
        let env = target.env(&self.paths, self.paths.root.clone());
        let name = target.name.clone();
        self.jobs.spawn(ctx, label, move |_| Done::Engine { target: name, info: targets::probe(&env) });
    }

    pub fn refresh_cluster(&mut self, ctx: &egui::Context) {
        if self.jobs.busy("cluster status") {
            return;
        }
        let Some(env) = self.env_for("cluster") else { return };
        let paths = self.paths.clone();
        let target = self.target_name();
        self.jobs.spawn(ctx, "cluster status", move |_| {
            let status = cluster::status(&env);
            if status == Status::Missing && cluster::adopt_existing(&env, &paths, &target) {
                return Done::Cluster(cluster::status(&env));
            }
            Done::Cluster(status)
        });
    }

    pub fn select_target(&mut self, ctx: &egui::Context, name: &str) {
        self.settings.target = name.to_string();
        self.save_settings();
        self.docker = DockerState::default();
        if let Some(t) = self.target() {
            self.probe_target(ctx, &t);
        }
        self.refresh_cluster(ctx);
        self.toast(format!("Labs now run on {name}"), true);
    }

    /// Type `cmd` into the terminal and run it, as if the learner did.
    pub fn run_in_terminal(&mut self, cmd: &str) {
        if self.terminal_env().is_none() {
            self.toast("No machine with Docker found: see Setup", false);
            return;
        }
        if !self.in_workspace() {
            self.term.open = true;
        }
        self.term.focus = true;
        if self.term.busy() {
            self.toast("The terminal is running a full-screen program: quit it first", false);
            return;
        }
        self.term.run(cmd);
    }

    fn handle(&mut self, ctx: &egui::Context, done: Done) {
        match done {
            Done::Targets(Ok(list)) => {
                self.targets = list;
                self.targets_error = None;
                if let Some(t) = self.target() {
                    self.probe_target(ctx, &t);
                }
                self.refresh_cluster(ctx);
            }
            Done::Targets(Err(e)) => self.targets_error = Some(e),
            Done::Engine { target, info } => {
                self.engine.insert(target, info);
            }
            Done::Cluster(s) => {
                if s.is_running() && self.cluster_record.is_none() {
                    self.cluster_record = ClusterRecord::load(&self.paths);
                }
                self.cluster = Some(s);
            }
            Done::Prepared { lesson, ok, log } => {
                if self.learn.lesson.as_deref() == Some(lesson.as_str()) {
                    self.learn.lab_log = log;
                    self.learn.prepared = ok;
                }
                if ok && let Some(l) = self.catalog.lesson(&lesson).cloned() {
                    self.progress.record_prepared(&l, now());
                    self.save_progress();
                }
                self.toast(if ok { "Lab ready: work in the terminal" } else { "Lab setup failed: see the log" }, ok);
            }
            Done::Reset { lesson, ok, log } => {
                if ok && let Some(l) = self.catalog.lesson(&lesson).cloned() {
                    self.progress.reset_lab(&l);
                    self.save_progress();
                }
                if self.learn.lesson.as_deref() == Some(lesson.as_str()) {
                    if ok {
                        self.learn.results.clear();
                        self.learn.hints.clear();
                        self.learn.prepared = false;
                    }
                    self.learn.lab_log = log;
                }
                self.toast(if ok { "Lab reset" } else { "Reset refused: your progress was kept (see the log)" }, ok);
            }
            Done::Checked { lesson, step, results } => {
                let passed = results.iter().all(|r| r.passed);
                if let Some(l) = self.catalog.lesson(&lesson).cloned() {
                    let was_done = self.progress.is_done(&l);
                    self.progress.record_step(&l, step, passed, now());
                    self.progress.record_results(&l, step, results.clone());
                    self.save_progress();
                    if !was_done && self.progress.is_done(&l) {
                        self.toast(format!("Lesson complete: {}", l.title), true);
                        self.celebrate = Some((Instant::now(), "Lesson complete".into(), l.title.clone()));
                    }
                }
                if self.learn.lesson.as_deref() == Some(lesson.as_str()) {
                    self.learn.results.insert(step, results);
                }
            }
            // Results from a machine that is no longer selected are dropped: their names must never
            // end up next to buttons that act on the new machine.
            Done::Docker { target, .. } if target != self.target_name() => {}
            Done::Layers { target, .. } if target != self.target_name() => {}
            Done::Docker { containers, images, volumes, networks, .. } => {
                let mut err = None;
                match containers {
                    Ok(v) => self.docker.containers = v,
                    Err(e) => err = Some(e),
                }
                if let Ok(v) = images {
                    self.docker.images = v;
                }
                if let Ok(v) = volumes {
                    self.docker.volumes = v;
                }
                if let Ok(v) = networks {
                    self.docker.networks = v;
                }
                self.docker.error = err;
            }
            Done::Layers { image, layers, .. } => {
                if self.docker.image.as_deref() == Some(image.as_str()) {
                    match layers {
                        Ok(l) => {
                            self.docker.layers = l;
                            self.docker.layers_error = None;
                        }
                        Err(e) => self.docker.layers_error = Some(e),
                    }
                }
            }
            Done::Namespaces(r) => match r {
                Ok(ns) => self.kube.namespaces = ns,
                Err(e) => self.kube.error = Some(e),
            },
            Done::Kube { kind, objects } => {
                if kind == self.kube.kind {
                    match objects {
                        Ok(o) => {
                            self.kube.objects = o;
                            self.kube.error = None;
                        }
                        Err(e) => self.kube.error = Some(e),
                    }
                }
            }
            Done::Doctor(tools) => self.doctor = Some(tools),
            Done::ClusterChanged { ok, log, created_on } => {
                self.cluster_log = log;
                if ok {
                    match created_on {
                        Some(target) => {
                            let rec = ClusterRecord { target, created_at: now() };
                            let _ = rec.save(&self.paths);
                            self.cluster_record = Some(rec);
                            self.toast("Lab cluster ready", true);
                        }
                        None => {
                            ClusterRecord::clear(&self.paths);
                            let _ = std::fs::remove_file(self.paths.kubeconfig());
                            self.cluster_record = None;
                            self.toast("Lab cluster deleted", true);
                        }
                    }
                } else {
                    self.toast("Cluster operation failed: see the log", false);
                }
                self.refresh_cluster(ctx);
            }
            Done::ExamStarted { exam, ok, log } => {
                self.exam.log = log;
                if ok {
                    self.progress.active_exam =
                        Some(poddojo_core::ActiveExam { exam_id: exam, started_at: now(), ..Default::default() });
                    self.exam.result = None;
                    self.exam.task = 0;
                    self.exam.auto_graded = false;
                    self.save_progress();
                    self.toast("The clock is running. Good luck!", true);
                } else {
                    self.toast("Exam setup failed: see the log", false);
                }
            }
            Done::DrillReady { key, ok, log } => self.drill_ready(key, ok, log),
            Done::DrillChecked { key, results } => self.drill_checked(key, results),
            Done::ExamGraded(attempt) => {
                let passed = attempt.passed;
                let percent = attempt.percent;
                self.exam.result = Some(attempt.clone());
                self.progress.record_exam(attempt, &self.catalog);
                self.save_progress();
                self.toast(format!("Score {percent:.0}%: {}", if passed { "PASS" } else { "keep training" }), passed);
                if passed {
                    self.celebrate = Some((Instant::now(), format!("{percent:.0}% · PASS"), "Mock exam passed".into()));
                }
            }
        }
    }

    /// Navigate by name (used by `PODDOJO_PAGE`).
    pub fn goto(&mut self, page: &str) {
        let mut parts = page.split(':');
        match parts.next().unwrap_or_default() {
            "learn" => {
                self.page = Page::Learn;
                if let Some(id) = parts.next() {
                    self.open_lesson(id);
                    match parts.next() {
                        Some("quiz") => self.learn.tab = LessonTab::Quiz,
                        Some("lab") => self.learn.tab = LessonTab::Lab,
                        Some("concepts") => self.learn.tab = LessonTab::Concepts,
                        _ => {}
                    }
                }
            }
            "docker" => self.page = Page::Docker,
            "cluster" => self.page = Page::Cluster,
            "exams" => self.page = Page::Exams,
            "review" => {
                self.page = Page::Review;
                if parts.next() == Some("session") {
                    self.start_session(false);
                }
            }
            "reference" => {
                self.page = Page::Reference;
                self.reference.page = parts.next().map(str::to_string);
            }
            "progress" => self.page = Page::Progress,
            "setup" => self.page = Page::Setup,
            _ => self.page = Page::Home,
        }
    }

    fn screenshot(&mut self, ctx: &egui::Context) {
        let Some((path, since, requested)) = self.shot.clone() else { return };
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let [w, h] = image.size;
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, bytes)
                && let Err(e) = buf.save(&path)
            {
                eprintln!("screenshot: {e}");
            }
            self.shot = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        // Development aid: `PODDOJO_CELEBRATE=1` shows the celebration just before the capture.
        if std::env::var_os("PODDOJO_CELEBRATE").is_some()
            && self.celebrate.is_none()
            && since.elapsed() > Duration::from_secs(3)
        {
            self.celebrate = Some((
                Instant::now() - Duration::from_millis(600),
                "Lesson complete".into(),
                "Build and deliver images to Kubernetes".into(),
            ));
        }
        let settled = since.elapsed() > Duration::from_secs(4)
            && self.jobs.running.is_empty()
            && self.term.settled(Duration::from_millis(2500));
        if !requested && (settled || since.elapsed() > Duration::from_secs(25)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.shot = Some((path, since, true));
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    /// ⌘1–⌘7 switch pages, ⌘J toggles the terminal, ⌘T opens an external one.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.profile.is_none() {
            return;
        }
        let (page, toggle, external) = ctx.input_mut(|i| {
            let mut page = None;
            for (n, key) in [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
                egui::Key::Num6,
                egui::Key::Num7,
                egui::Key::Num8,
                egui::Key::Num9,
            ]
            .into_iter()
            .enumerate()
            {
                if i.consume_key(egui::Modifiers::COMMAND, key) {
                    page = Page::ALL.get(n).map(|p| p.0);
                }
            }
            (
                page,
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::J),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::T),
            )
        });
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K)) {
            self.reference.overlay = !self.reference.overlay;
        }
        if let Some(p) = page {
            self.page = p;
        }
        if toggle {
            if self.in_workspace() {
                self.term.focus = true;
            } else {
                self.term.open = !self.term.open;
                self.term.focus = self.term.open;
            }
        }
        if external {
            self.open_terminal();
        }
    }

    fn tick_study_time(&mut self) {
        if self.last_minute.elapsed() >= Duration::from_secs(60) {
            self.last_minute = Instant::now();
            if self.page == Page::Learn
                && let Some(id) = self.learn.lesson.clone()
            {
                self.progress.add_minutes(&id, 1);
                self.save_progress();
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Frame

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        egui::Panel::top("top")
            .exact_size(54.0)
            .frame(egui::Frame::new().fill(t.panel).inner_margin(egui::Margin::symmetric(14, 8)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    // The living mark: still when idle, alive while commands run (or on hover).
                    let energy = if self.jobs.running.is_empty() { 0.0 } else { 1.0 };
                    mark::show(ui, 34.0, energy, true, self.settings.motion)
                        .on_hover_text(if self.jobs.running.is_empty() { "PodDojo" } else { "Working…" });
                    ui.label(RichText::new("PodDojo").size(19.0).strong().color(if t.dark {
                        Color32::WHITE
                    } else {
                        Color32::BLACK
                    }));
                    ui.label(RichText::new("Docker + CKAD").size(12.0).color(t.muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ctx = ui.ctx().clone();
                        if self.page != Page::Reference
                            && icons::button(ui, Icon::Docs, "Docs")
                                .on_hover_text("The reference in a floating window, beside your work (⌘K)")
                                .clicked()
                        {
                            self.reference.overlay = !self.reference.overlay;
                        }
                        if !self.in_workspace()
                            && icons::button(
                                ui,
                                Icon::Console,
                                if self.term.open { "Hide terminal" } else { "Terminal" },
                            )
                            .on_hover_text("The built-in terminal, with the lab environment (⌘J)")
                            .clicked()
                        {
                            self.term.open = !self.term.open;
                            self.term.focus = self.term.open;
                        }
                        ui.separator();
                        self.cluster_badge(ui);
                        ui.separator();
                        self.machine_picker(ui, &ctx);
                    });
                });
            });
    }

    /// The Lab tab of a lesson and a running exam are full workspaces with their own terminal pane.
    pub fn in_workspace(&self) -> bool {
        (self.page == Page::Learn && self.learn.tab == LessonTab::Lab && self.learn.lesson.is_some())
            || (self.page == Page::Exams && self.progress.active_exam.is_some())
            || (self.page == Page::Review && self.review.drill.as_ref().is_some_and(|d| d.ready()))
    }

    /// One-click commands that fit what the learner is doing.
    pub fn terminal_chips(&self) -> Vec<String> {
        let drill =
            self.review.drill.as_ref().filter(|_| self.page == Page::Review && self.progress.active_exam.is_none());
        if let Some(d) = drill {
            let ns = self
                .catalog
                .exam(&d.exam)
                .and_then(|e| e.tasks.iter().find(|t| t.id == d.task))
                .and_then(|t| t.namespace.clone());
            return match ns {
                Some(ns) => vec![
                    format!("kubectl -n {ns} get all"),
                    format!("kubectl -n {ns} get events --sort-by=.lastTimestamp"),
                ],
                None => vec!["kubectl get ns".into()],
            };
        }
        if let Some(a) = &self.progress.active_exam {
            let ns = self
                .catalog
                .exam(&a.exam_id)
                .and_then(|e| e.tasks.get(self.exam.task))
                .and_then(|t| t.namespace.clone());
            return match ns {
                Some(ns) => vec![
                    format!("kubectl -n {ns} get all"),
                    format!("kubectl -n {ns} get events --sort-by=.lastTimestamp"),
                    "kubectl get ns".into(),
                ],
                None => vec!["kubectl get ns".into(), "kubectl get all -A".into()],
            };
        }
        let lesson = self.learn.lesson.as_ref().and_then(|id| self.catalog.lesson(id));
        match lesson.map(|l| l.needs) {
            Some(poddojo_core::Needs::Cluster) => vec![
                "kubectl get pods -A".into(),
                "kubectl get events -A --sort-by=.lastTimestamp".into(),
                "kubectl api-resources --namespaced=true".into(),
            ],
            _ => vec!["docker ps -a".into(), "docker images".into(), "docker network ls".into()],
        }
    }

    /// Draw the terminal in `ui` and act on what the learner does with it.
    pub fn terminal(&mut self, ui: &mut egui::Ui, chips: &[String]) {
        let env = self.terminal_env();
        let machine = self.target_name();
        let home = self.paths.profile_dir();
        let chips: Vec<&str> = chips.iter().map(String::as_str).collect();
        if let Some(terminal::Action::OpenExternal) = self.term.ui(ui, env.as_ref(), &machine, &home, &chips) {
            self.open_terminal();
        }
    }

    pub fn open_terminal(&mut self) {
        match (self.terminal_env(), self.target()) {
            (Some(env), Some(t)) => {
                if let Err(e) = poddojo_runner::terminal::open(&env, &t.name, &self.paths.root) {
                    self.toast(e, false);
                }
            }
            _ => self.toast("No machine with Docker found: see Setup", false),
        }
    }

    fn machine_picker(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let t = self.t;
        let current = self.target();
        let (dot, tip) = match current.as_ref().and_then(|c| self.engine.get(&c.name)) {
            Some(Ok(i)) => (t.good, format!("Docker {} · {} · {} CPUs", i.server_version, i.os, i.cpus)),
            Some(Err(e)) => (t.bad, e.clone()),
            None => (t.muted, "checking…".into()),
        };
        let mut pick = None;
        egui::ComboBox::from_id_salt("machine")
            .selected_text(RichText::new(current.as_ref().map_or("choose a machine", |c| c.name.as_str())).strong())
            .width(170.0)
            .show_ui(ui, |ui| {
                for target in &self.targets {
                    let label = format!(
                        "{}  ·  {}",
                        target.name,
                        if target.is_remote() { target.endpoint.trim_start_matches("ssh://") } else { "this computer" }
                    );
                    if ui.selectable_label(current.as_ref().is_some_and(|c| c.name == target.name), label).clicked() {
                        pick = Some(target.name.clone());
                    }
                }
                ui.separator();
                ui.label(RichText::new("Add machines with `docker context create`").small().color(t.muted));
            })
            .response
            .on_hover_text(tip);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.5, dot);
        icons::show(ui, Icon::Server, 16.0, t.muted);
        if let Some(name) = pick {
            self.select_target(ctx, &name);
        }
    }

    fn cluster_badge(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        let (color, text) = match &self.cluster {
            Some(Status::Running { nodes }) => {
                (t.good, format!("kind-poddojo {}", nodes.first().map_or("", |n| n.version.as_str())))
            }
            Some(Status::Missing) => (t.muted, "no lab cluster".into()),
            Some(Status::Unreachable(_)) => (t.bad, "cluster unreachable".into()),
            None => (t.muted, "cluster…".into()),
        };
        if ui
            .add(egui::Button::new(RichText::new(text).color(color)).frame(false))
            .on_hover_text("Open the Cluster page")
            .clicked()
        {
            self.page = Page::Cluster;
        }
        icons::show(ui, Icon::Cluster, 16.0, color);
    }

    fn nav(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        egui::Panel::left("nav")
            .exact_size(168.0)
            .frame(egui::Frame::new().fill(t.panel).inner_margin(egui::Margin::symmetric(10, 14)))
            .show(ui, |ui| {
                ui.add_space(4.0);
                // The selection highlight slides between items (positions from the last frame).
                let rects_id = egui::Id::new("nav-rects");
                let rects: Vec<egui::Rect> = ui.ctx().data(|d| d.get_temp(rects_id)).unwrap_or_default();
                let sel = Page::ALL.iter().position(|p| p.0 == self.page);
                let sliding = sel.and_then(|i| rects.get(i)).is_some();
                if let Some(r) = sel.and_then(|i| rects.get(i)) {
                    let y = ui.ctx().animate_value_with_time(
                        egui::Id::new("nav-sel-y"),
                        r.min.y,
                        theme::motion(ui.ctx(), 0.18),
                    );
                    let hr = egui::Rect::from_min_size(egui::pos2(r.min.x, y), r.size());
                    ui.painter().rect_filled(hr, 9.0, t.accent_soft);
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(hr.min + egui::vec2(0.0, 9.0), egui::vec2(3.0, 20.0)),
                        2.0,
                        t.accent,
                    );
                }
                let mut new_rects = Vec::new();
                for (i, (page, icon, label)) in Page::ALL.into_iter().enumerate() {
                    let resp = nav_item(ui, &t, icon, label, self.page == page, !sliding, &format!("⌘{}", i + 1));
                    new_rects.push(resp.rect);
                    if resp.clicked() {
                        self.page = page;
                    }
                    if page == Page::Exams && self.progress.active_exam.is_some() {
                        ui.horizontal(|ui| {
                            ui.add_space(40.0);
                            let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                            ui.painter().circle_filled(r.center(), 3.5, t.amber);
                            ui.label(RichText::new("exam running").size(11.0).color(t.amber));
                        });
                    }
                }
                ui.ctx().data_mut(|d| d.insert_temp(rects_id, new_rects));
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    if let Some(p) = self.profile.clone()
                        && profile_chip(ui, &t, &p).on_hover_text("Switch profile").clicked()
                    {
                        self.switch_profile();
                    }
                    ui.add_space(8.0);
                    let (done, total) = self.progress.done_count(&self.catalog);
                    ui.label(RichText::new(format!("{done}/{total} lessons")).size(12.0).color(t.muted));
                    theme::bar(ui, &t, if total == 0 { 0.0 } else { done as f32 / total as f32 }, 148.0, t.accent);
                    let streak = self.progress.streak(chrono::Local::now().date_naive());
                    if streak > 0 {
                        ui.label(RichText::new(format!("{streak}-day streak")).size(12.0).color(t.amber));
                    }
                });
            });
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        self.toasts.retain(|(_, _, at)| at.elapsed() < Duration::from_secs(5));
        if self.toasts.is_empty() {
            return;
        }
        let t = self.t;
        egui::Area::new(egui::Id::new("toasts")).anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0)).show(
            ctx,
            |ui| {
                for (text, ok, at) in &self.toasts {
                    let age = at.elapsed().as_secs_f32();
                    let enter = if self.settings.motion { (age / 0.25).clamp(0.0, 1.0) } else { 1.0 };
                    let leave = if self.settings.motion { ((5.0 - age) / 0.4).clamp(0.0, 1.0) } else { 1.0 };
                    ui.add_space((1.0 - enter) * 24.0);
                    ui.set_opacity(enter.min(leave));
                    egui::Frame::new()
                        .fill(t.card)
                        .stroke(egui::Stroke::new(1.0, if *ok { t.good } else { t.bad }))
                        .corner_radius(10)
                        .inner_margin(egui::Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.label(RichText::new(text).color(t.text));
                        });
                    ui.add_space(6.0);
                }
            },
        );
        ctx.request_repaint_after(Duration::from_millis(if self.settings.motion { 16 } else { 500 }));
    }

    pub fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.started = true;
            self.refresh_targets(&ctx);
        }
        // Development aid for screenshots and demos: `PODDOJO_RUN="cmd1;;cmd2"` runs commands in the
        // terminal once the machine is known.
        if !self.demo_ran && self.target().is_some() {
            self.demo_ran = true;
            if let Ok(cmds) = std::env::var("PODDOJO_RUN") {
                for cmd in cmds.split(";;").map(str::trim).filter(|c| !c.is_empty()) {
                    self.run_in_terminal(cmd);
                }
            }
        }
        self.sync_progress();
        self.shortcuts(&ctx);
        let me = self.profile.as_ref().map(|p| p.id.clone()).unwrap_or_default();
        for (owner, done) in self.jobs.poll() {
            // Lab and exam results belong to the profile that started them: if the learner switched
            // profiles meanwhile, drop them instead of recording them for the wrong person.
            let personal = matches!(
                done,
                Done::Checked { .. }
                    | Done::Prepared { .. }
                    | Done::Reset { .. }
                    | Done::ExamStarted { .. }
                    | Done::ExamGraded(_)
                    | Done::DrillReady { .. }
                    | Done::DrillChecked { .. }
            );
            if personal && owner != me {
                continue;
            }
            self.handle(&ctx, done);
        }
        self.tick_study_time();
        if self.profile.is_none() {
            let t = self.t;
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin::same(24)))
                .show(ui, |ui| {
                    self.welcome_page(ui);
                });
            self.toasts(&ctx);
            self.screenshot(&ctx);
            if !self.jobs.running.is_empty() {
                ctx.request_repaint_after(Duration::from_millis(150));
            }
            return;
        }
        self.top_bar(ui);
        self.nav(ui);
        if self.term.open && !self.in_workspace() {
            let t = self.t;
            egui::Panel::bottom("terminal")
                .resizable(true)
                .default_size(300.0)
                .min_size(160.0)
                .frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin {
                    left: 14,
                    right: 14,
                    top: 8,
                    bottom: 12,
                }))
                .show(ui, |ui| {
                    let chips = self.terminal_chips();
                    self.terminal(ui, &chips);
                });
        }
        let t = self.t;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin::symmetric(24, 18)))
            .show(ui, |ui| {
                if !self.content_errors.is_empty() {
                    theme::card(&t).show(ui, |ui| {
                        ui.label(RichText::new("The curriculum has errors").color(t.bad).strong());
                        for e in self.content_errors.iter().take(8) {
                            ui.label(RichText::new(e).small());
                        }
                    });
                }
                match self.page {
                    Page::Welcome | Page::Home => self.home(ui),
                    Page::Learn => self.learn_page(ui),
                    Page::Docker => self.docker_page(ui),
                    Page::Cluster => self.cluster_page(ui),
                    Page::Exams => self.exams_page(ui),
                    Page::Review => self.review_page(ui),
                    Page::Reference => self.reference_page(ui),
                    Page::Progress => self.progress_page(ui),
                    Page::Setup => self.setup_page(ui),
                }
            });
        self.toasts(&ctx);
        self.reference_overlay(&ctx);
        self.celebration(&ctx);
        self.screenshot(&ctx);
        if !self.jobs.running.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
    }

    /// The living mark bursting with rings for a moment after a lesson is completed or an exam
    /// passed.
    fn celebration(&mut self, ctx: &egui::Context) {
        let Some((at, headline, detail)) = self.celebrate.clone() else { return };
        let age = at.elapsed().as_secs_f32();
        // Short, and never during an exam: motion must not get in the way of work.
        if age > 1.4 || !self.settings.motion || self.progress.active_exam.is_some() {
            self.celebrate = None;
            return;
        }
        let t = self.t;
        let fade = (age / 0.15).min(1.0).min(((1.4 - age) / 0.35).clamp(0.0, 1.0));
        let screen = ctx.content_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("celebrate")));
        let c = screen.center();
        for i in 0..3 {
            let r = (age - i as f32 * 0.18).max(0.0) * 260.0;
            let a = (1.0 - r / 520.0).clamp(0.0, 1.0) * fade;
            painter.circle_stroke(c, r, egui::Stroke::new(3.0, t.accent.gamma_multiply(a * 0.8)));
        }
        let card = egui::Rect::from_center_size(c, egui::vec2(320.0, 220.0));
        painter.rect_filled(card, 20.0, t.card.gamma_multiply(fade));
        painter.rect_stroke(
            card,
            20.0,
            egui::Stroke::new(1.5, t.accent.gamma_multiply(fade)),
            egui::StrokeKind::Inside,
        );
        let scale = 0.7 + 0.3 * (age / 0.35).min(1.0);
        mark::paint(
            &painter,
            egui::Rect::from_center_size(c - egui::vec2(0.0, 30.0), egui::vec2(110.0 * scale, 110.0 * scale)),
            ctx.input(|i| i.time),
            1.0,
            false,
        );
        painter.text(
            c + egui::vec2(0.0, 50.0),
            egui::Align2::CENTER_CENTER,
            headline,
            egui::FontId::proportional(22.0),
            t.text.gamma_multiply(fade),
        );
        painter.text(
            c + egui::vec2(0.0, 78.0),
            egui::Align2::CENTER_CENTER,
            detail,
            egui::FontId::proportional(13.0),
            t.muted.gamma_multiply(fade),
        );
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn progress_mtime(paths: &Paths) -> Option<std::time::SystemTime> {
    std::fs::metadata(paths.progress()).and_then(|m| m.modified()).ok()
}

/// A navigation row: icon + label, accent bar and soft fill when selected.
fn nav_item(
    ui: &mut egui::Ui,
    t: &Tokens,
    icon: Icon,
    label: &str,
    selected: bool,
    paint_selection: bool,
    shortcut: &str,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label));
    theme::focus_ring(ui, &resp, rect, 9.0);
    let hovered = resp.hovered();
    let hover =
        ui.ctx().animate_bool_with_time(resp.id.with("hover"), hovered && !selected, theme::motion(ui.ctx(), 0.12));
    let p = ui.painter();
    if selected && paint_selection {
        p.rect_filled(rect, 9.0, t.accent_soft);
        p.rect_filled(egui::Rect::from_min_size(rect.min + egui::vec2(0.0, 9.0), egui::vec2(3.0, 20.0)), 2.0, t.accent);
    } else if hover > 0.0 {
        p.rect_filled(rect, 9.0, t.card.gamma_multiply(hover));
    }
    let color = if selected {
        t.accent
    } else if hovered {
        t.text
    } else {
        t.muted
    };
    icons::paint(
        p,
        egui::Rect::from_center_size(egui::pos2(rect.min.x + 24.0, rect.center().y), egui::vec2(18.0, 18.0)),
        icon,
        color,
    );
    p.text(
        egui::pos2(rect.min.x + 44.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(15.0),
        if selected || hovered { t.text } else { t.muted },
    );
    if hovered {
        p.text(
            egui::pos2(rect.max.x - 10.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            shortcut,
            egui::FontId::proportional(11.0),
            t.muted,
        );
    }
    ui.add_space(2.0);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Avatar colours for profiles.
pub const AVATARS: [Color32; 8] = [
    Color32::from_rgb(45, 212, 191),
    Color32::from_rgb(129, 140, 248),
    Color32::from_rgb(251, 146, 60),
    Color32::from_rgb(244, 114, 182),
    Color32::from_rgb(56, 189, 248),
    Color32::from_rgb(163, 230, 53),
    Color32::from_rgb(250, 204, 21),
    Color32::from_rgb(148, 163, 184),
];

/// A round avatar with the profile's initials.
pub fn avatar(ui: &mut egui::Ui, p: &Profile, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let color = AVATARS.get(p.color as usize % AVATARS.len()).copied().unwrap_or(Color32::GRAY);
    ui.painter().circle_filled(rect.center(), size / 2.0, color);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        p.initials(),
        egui::FontId::proportional(size * 0.38),
        Color32::from_rgb(10, 14, 30),
    );
    resp
}

/// The current learner at the bottom of the navigation; click to switch.
fn profile_chip(ui: &mut egui::Ui, t: &Tokens, p: &Profile) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), egui::Sense::click());
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("{}: switch profile", p.name))
    });
    theme::focus_ring(ui, &resp, rect, 10.0);
    if resp.hovered() {
        ui.painter().rect_filled(rect, 10.0, t.card);
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(8.0, 6.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    avatar(&mut child, p, 30.0);
    child.vertical(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        ui.label(RichText::new(&p.name).size(13.5).strong().color(t.text));
        ui.label(
            RichText::new(if p.is_guest() { "Guest · not saved" } else { "Switch profile" }).size(11.0).color(t.muted),
        );
    });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
