//! PodDojo's interface. Thin by design: it reads `poddojo-core` state, asks `poddojo-runner` to do
//! things on background threads (`jobs`), and draws the result.

mod console;
pub mod jobs;
mod logo;
pub mod markdown;
mod pages;
pub mod theme;

pub use logo::icon_rgba;

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use egui::{Color32, RichText};
use poddojo_core::{Catalog, CheckResult, ExamAttempt, Progress, now};
use poddojo_runner::cluster::{self, ClusterRecord, Status};
use poddojo_runner::doctor::Tool;
use poddojo_runner::explore::{KubeObject, Layer, Row};
use poddojo_runner::targets::{self, EngineInfo, Target};
use poddojo_runner::{LabEnv, Paths, Settings};

use console::Console;
use jobs::{Done, Jobs};
use theme::Tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Learn,
    Docker,
    Cluster,
    Exams,
    Progress,
    Setup,
}

impl Page {
    const ALL: [(Page, &'static str, &'static str); 7] = [
        (Page::Home, "🏠", "Home"),
        (Page::Learn, "📖", "Learn"),
        (Page::Docker, "🐳", "Docker"),
        (Page::Cluster, "☸", "Cluster"),
        (Page::Exams, "⏱", "Exams"),
        (Page::Progress, "📈", "Progress"),
        (Page::Setup, "⚙", "Setup"),
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
pub struct ExamState {
    pub task: usize,
    pub result: Option<ExamAttempt>,
    pub reveal: BTreeSet<String>,
    pub confirm_finish: bool,
    pub log: String,
    pub drill: Option<Drill>,
}

/// A knowledge drill: random quiz questions from lessons already opened.
pub struct Drill {
    pub questions: Vec<(String, usize)>,
    pub answers: Vec<Option<usize>>,
    pub submitted: bool,
}

pub struct App {
    pub paths: Paths,
    pub settings: Settings,
    pub catalog: Catalog,
    pub content_errors: Vec<String>,
    pub progress: Progress,
    pub page: Page,
    pub t: Tokens,
    pub jobs: Jobs,
    pub console: Console,
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
    pub toasts: Vec<(String, bool, Instant)>,
    pub logo: Option<egui::TextureHandle>,
    last_minute: Instant,
    started: bool,
    /// Modification time of progress.json when we last read or wrote it: the `dojo` CLI may change
    /// it while the app is open, and the app must build on that instead of overwriting it.
    progress_stamp: Option<std::time::SystemTime>,
    /// `PODDOJO_SCREENSHOT=<png>`: capture this window after the first jobs settle, then quit
    /// (with `PODDOJO_PAGE=home|learn:<lesson>[:quiz|:lab]|docker|cluster|exams|progress|setup`).
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
        let mut startup_toast = None;
        let progress = match Progress::load(&paths.progress()) {
            Ok(p) => p,
            Err(e) => {
                // Keep the unreadable file instead of overwriting it with an empty history.
                let backup = paths.root.join(format!("progress.json.unreadable-{}", now()));
                let kept = std::fs::copy(paths.progress(), &backup).is_ok();
                startup_toast = Some(format!(
                    "Your progress file could not be read ({e}). {}",
                    if kept {
                        format!("A copy was kept at {}.", backup.display())
                    } else {
                        "It was left untouched.".into()
                    }
                ));
                Progress::default()
            }
        };
        let t = Tokens::get(settings.dark);
        theme::apply(ctx, &t, settings.zoom);
        let cluster_record = ClusterRecord::load(&paths);
        let mut app = Self {
            paths,
            settings,
            catalog,
            content_errors,
            progress,
            page: Page::Home,
            t,
            jobs: Jobs::default(),
            console: Console::default(),
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
            toasts: Vec::new(),
            logo: logo::texture(ctx),
            last_minute: Instant::now(),
            started: false,
            progress_stamp: None,
            shot: std::env::var_os("PODDOJO_SCREENSHOT").map(|p| (std::path::PathBuf::from(p), Instant::now(), false)),
        };
        if let Ok(page) = std::env::var("PODDOJO_PAGE") {
            app.goto(&page);
        }
        app.progress_stamp = progress_mtime(&app.paths);
        if let Some(t) = startup_toast {
            app.toast(t, false);
        }
        if app.progress.active_exam.is_some() {
            app.page = Page::Exams;
        }
        app
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

    /// Where console commands run: the open exam, else the open lesson, else a scratch folder.
    pub fn console_env(&self) -> Option<LabEnv> {
        let id = if let Some(e) = &self.progress.active_exam {
            format!("exam-{}", e.exam_id)
        } else if let Some(l) = &self.learn.lesson {
            l.clone()
        } else {
            "playground".into()
        };
        self.env_for(&id)
    }

    pub fn save_progress(&mut self) {
        if let Err(e) = self.progress.save(&self.paths.progress()) {
            self.toast(format!("Could not save progress: {e}"), false);
        }
        self.progress_stamp = progress_mtime(&self.paths);
    }

    /// Pick up progress written by the CLI (or another window) since we last looked.
    fn sync_progress(&mut self) {
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
        self.jobs.spawn(ctx, "cluster status", move |_| Done::Cluster(cluster::status(&env)));
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

    /// Run `cmd` in the console (shown with its live output).
    pub fn run_in_console(&mut self, ctx: &egui::Context, cmd: &str) {
        let Some(env) = self.console_env() else {
            self.toast("No machine with Docker found: see Setup", false);
            return;
        };
        self.console.open = true;
        let entry = self.console.push(cmd);
        let cmd = cmd.to_string();
        let id = self.jobs.spawn(ctx, format!("console {entry}"), move |live| Done::Console {
            entry,
            out: poddojo_runner::exec::run_live(&cmd, &env, Duration::from_secs(900), Some(live)),
        });
        self.console.set_job(entry, id);
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
            Done::Cluster(s) => self.cluster = Some(s),
            Done::Console { entry, out } => self.console.finish(entry, out),
            Done::Prepared { lesson, ok, log } => {
                if self.learn.lesson.as_deref() == Some(lesson.as_str()) {
                    self.learn.lab_log = log;
                }
                self.toast(
                    if ok {
                        "Lab ready: work in the console or Open terminal"
                    } else {
                        "Lab setup failed: see the log"
                    },
                    ok,
                );
            }
            Done::Reset { lesson, log } => {
                if let Some(l) = self.catalog.lesson(&lesson).cloned() {
                    self.progress.reset_lab(&l);
                    self.save_progress();
                }
                if self.learn.lesson.as_deref() == Some(lesson.as_str()) {
                    self.learn.results.clear();
                    self.learn.lab_log = log;
                }
                self.toast("Lab reset", true);
            }
            Done::Checked { lesson, step, results } => {
                let passed = results.iter().all(|r| r.passed);
                if let Some(l) = self.catalog.lesson(&lesson).cloned() {
                    let was_done = self.progress.is_done(&l);
                    self.progress.record_step(&l, step, passed, now());
                    self.save_progress();
                    if !was_done && self.progress.is_done(&l) {
                        self.toast(format!("🥋 Lesson complete: {}", l.title), true);
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
                    self.save_progress();
                    self.toast("The clock is running. Good luck!", true);
                } else {
                    self.toast("Exam setup failed: see the log", false);
                }
            }
            Done::ExamGraded(attempt) => {
                let passed = attempt.passed;
                let percent = attempt.percent;
                self.exam.result = Some(attempt.clone());
                self.progress.record_exam(attempt);
                self.save_progress();
                self.toast(format!("Score {percent:.0}%: {}", if passed { "PASS" } else { "keep training" }), passed);
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
        let settled = since.elapsed() > Duration::from_secs(4) && self.jobs.running.is_empty();
        if !requested && (settled || since.elapsed() > Duration::from_secs(25)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.shot = Some((path, since, true));
        }
        ctx.request_repaint_after(Duration::from_millis(100));
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
                    if let Some(tex) = &self.logo {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(32.0, 32.0)).corner_radius(7.0));
                    }
                    ui.label(RichText::new("PodDojo").size(19.0).strong().color(if t.dark {
                        Color32::WHITE
                    } else {
                        Color32::BLACK
                    }));
                    ui.label(RichText::new("Docker + CKAD").size(12.0).color(t.muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ctx = ui.ctx().clone();
                        if ui
                            .button("🖥 Open terminal")
                            .on_hover_text("A real shell with DOCKER_CONTEXT and KUBECONFIG set for the lab")
                            .clicked()
                        {
                            self.open_terminal();
                        }
                        let console_label = if self.console.open { "⌨ Hide console" } else { "⌨ Console" };
                        if ui.button(console_label).clicked() {
                            self.console.open = !self.console.open;
                        }
                        ui.separator();
                        self.cluster_badge(ui);
                        ui.separator();
                        self.machine_picker(ui, &ctx);
                    });
                });
            });
    }

    pub fn open_terminal(&mut self) {
        match (self.console_env(), self.target()) {
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
            .selected_text(
                RichText::new(format!("🖧 {}", current.as_ref().map_or("choose a machine", |c| c.name.as_str())))
                    .strong(),
            )
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
        ui.label(RichText::new("Machine").size(12.0).color(t.muted));
        if let Some(name) = pick {
            self.select_target(ctx, &name);
        }
    }

    fn cluster_badge(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        let (color, text) = match &self.cluster {
            Some(Status::Running { nodes }) => {
                (t.good, format!("☸ kind-poddojo {}", nodes.first().map_or("", |n| n.version.as_str())))
            }
            Some(Status::Missing) => (t.muted, "☸ no lab cluster".into()),
            Some(Status::Unreachable(_)) => (t.bad, "☸ cluster unreachable".into()),
            None => (t.muted, "☸ …".into()),
        };
        if ui
            .add(egui::Button::new(RichText::new(text).color(color)).frame(false))
            .on_hover_text("Open the Cluster page")
            .clicked()
        {
            self.page = Page::Cluster;
        }
    }

    fn nav(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        egui::Panel::left("nav")
            .exact_size(168.0)
            .frame(egui::Frame::new().fill(t.panel).inner_margin(egui::Margin::symmetric(10, 14)))
            .show(ui, |ui| {
                for (page, icon, label) in Page::ALL {
                    let selected = self.page == page;
                    let text = RichText::new(format!("{icon}  {label}")).size(15.0).color(if selected {
                        t.accent
                    } else {
                        t.text
                    });
                    let resp = ui.add_sized(
                        [148.0, 34.0],
                        egui::Button::new(text)
                            .fill(if selected { t.accent_soft } else { Color32::TRANSPARENT })
                            .stroke(egui::Stroke::NONE),
                    );
                    if resp.clicked() {
                        self.page = page;
                    }
                    if page == Page::Exams && self.progress.active_exam.is_some() {
                        ui.label(RichText::new("   ● exam running").size(11.0).color(t.amber));
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    let (done, total) = self.progress.done_count(&self.catalog);
                    ui.label(RichText::new(format!("{done}/{total} lessons")).size(12.0).color(t.muted));
                    theme::bar(ui, &t, if total == 0 { 0.0 } else { done as f32 / total as f32 }, 148.0, t.accent);
                    let streak = self.progress.streak(chrono::Local::now().date_naive());
                    if streak > 0 {
                        ui.label(RichText::new(format!("🔥 {streak}-day streak")).size(12.0).color(t.amber));
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
                for (text, ok, _) in &self.toasts {
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
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    pub fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.started = true;
            self.refresh_targets(&ctx);
        }
        self.sync_progress();
        for done in self.jobs.poll() {
            self.handle(&ctx, done);
        }
        self.tick_study_time();
        self.top_bar(ui);
        self.nav(ui);
        if self.console.open {
            let t = self.t;
            let mut run = None;
            egui::Panel::bottom("console")
                .resizable(true)
                .default_size(230.0)
                .min_size(120.0)
                .frame(egui::Frame::new().fill(t.code_bg).inner_margin(egui::Margin::same(10)))
                .show(ui, |ui| {
                    let env = self.console_env();
                    run = self.console.ui(ui, &t, &self.jobs, env.as_ref(), &self.target_name());
                });
            if let Some(cmd) = run {
                self.run_in_console(&ctx, &cmd);
            }
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
                    Page::Home => self.home(ui),
                    Page::Learn => self.learn_page(ui),
                    Page::Docker => self.docker_page(ui),
                    Page::Cluster => self.cluster_page(ui),
                    Page::Exams => self.exams_page(ui),
                    Page::Progress => self.progress_page(ui),
                    Page::Setup => self.setup_page(ui),
                }
            });
        self.toasts(&ctx);
        self.screenshot(&ctx);
        if !self.jobs.running.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
    }
}

fn progress_mtime(paths: &Paths) -> Option<std::time::SystemTime> {
    std::fs::metadata(paths.progress()).and_then(|m| m.modified()).ok()
}
