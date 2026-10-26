//! The built-in terminal: type `docker`/`kubectl` commands and see their output, run on the
//! chosen machine with the lab environment. Non-interactive (no `-it`, no editors): for that,
//! "Open terminal" starts a real shell with the same environment.

use std::time::{Duration, Instant};

use egui::{Color32, CornerRadius, FontId, Margin, RichText, Stroke};
use poddojo_core::CommandOutput;
use poddojo_runner::LabEnv;

use crate::icons::{self, Icon};
use crate::jobs::Jobs;
use crate::theme::Tokens;

/// The terminal keeps its own dark palette in both themes, like a real terminal.
const BG: Color32 = Color32::from_rgb(7, 10, 22);
const BAR: Color32 = Color32::from_rgb(14, 19, 40);
const BORDER: Color32 = Color32::from_rgb(36, 44, 82);
const TEXT: Color32 = Color32::from_rgb(203, 213, 235);
const DIM: Color32 = Color32::from_rgb(110, 122, 160);
const PROMPT: Color32 = Color32::from_rgb(45, 212, 191);
const PATH: Color32 = Color32::from_rgb(129, 140, 248);
const OK: Color32 = Color32::from_rgb(52, 211, 153);
const ERR: Color32 = Color32::from_rgb(248, 113, 113);
const RUN: Color32 = Color32::from_rgb(251, 191, 36);

pub struct Entry {
    /// Stable id: completions find their entry by id, never by position (Clear and the 200-entry
    /// cap shift positions while commands are still running).
    pub id: u64,
    pub cmd: String,
    pub output: String,
    pub status: Option<i32>,
    pub running: bool,
    pub job: Option<u64>,
    pub started: Instant,
    pub took: Option<Duration>,
    /// Where it ran: `machine:folder`.
    pub place: String,
}

pub enum Action {
    Run(String),
    OpenTerminal,
}

#[derive(Default)]
pub struct Console {
    pub open: bool,
    pub entries: Vec<Entry>,
    pub input: String,
    history_pos: Option<usize>,
    stick_to_bottom: bool,
    next_id: u64,
    pub focus: bool,
}

fn place(env: Option<&LabEnv>, machine: &str) -> (String, String) {
    let folder = env
        .and_then(|e| e.workdir.file_name().map(|f| f.to_string_lossy().to_string()))
        .unwrap_or_else(|| "lab".into());
    // Lesson folders are long ids (`ckad-images-for-kubernetes`); keep the prompt short.
    let short: String = folder
        .strip_prefix("ckad-")
        .or_else(|| folder.strip_prefix("docker-"))
        .unwrap_or(&folder)
        .chars()
        .take(22)
        .collect();
    (machine.to_string(), format!("~/{short}"))
}

impl Console {
    pub fn push(&mut self, cmd: &str, place: String) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.entries.push(Entry {
            id,
            cmd: cmd.to_string(),
            output: String::new(),
            status: None,
            running: true,
            job: None,
            started: Instant::now(),
            took: None,
            place,
        });
        if self.entries.len() > 200
            && let Some(pos) = self.entries.iter().position(|e| !e.running)
        {
            self.entries.remove(pos);
        }
        self.stick_to_bottom = true;
        id
    }

    pub fn set_job(&mut self, entry: u64, job: u64) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == entry) {
            e.job = Some(job);
        }
    }

    pub fn finish(&mut self, entry: u64, out: CommandOutput) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == entry) {
            e.running = false;
            e.status = out.status;
            e.took = Some(e.started.elapsed());
            e.output = out.combined();
            if out.timed_out {
                e.output.push_str("\n(timed out)");
            }
        }
        self.stick_to_bottom = true;
    }

    pub fn place_label(env: Option<&LabEnv>, machine: &str) -> String {
        let (m, f) = place(env, machine);
        format!("{m} {f}")
    }

    /// Draw the terminal filling the available space. `chips` are one-click commands.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        t: &Tokens,
        jobs: &Jobs,
        env: Option<&LabEnv>,
        machine: &str,
        chips: &[&str],
    ) -> Option<Action> {
        let mut action = None;
        let (m, folder) = place(env, machine);
        egui::Frame::new().fill(BG).stroke(Stroke::new(1.0, BORDER)).corner_radius(CornerRadius::same(12)).show(
            ui,
            |ui| {
                ui.set_min_size(ui.available_size());
                // Title bar.
                egui::Frame::new()
                    .fill(BAR)
                    .corner_radius(CornerRadius { nw: 12, ne: 12, sw: 0, se: 0 })
                    .inner_margin(Margin::symmetric(12, 7))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            for c in [Color32::from_rgb(248, 113, 113), Color32::from_rgb(251, 191, 36), OK] {
                                let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                                ui.painter().circle_filled(r.center(), 4.5, c.gamma_multiply(0.85));
                            }
                            ui.add_space(6.0);
                            icons::show(ui, Icon::Terminal, 14.0, DIM);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                if tiny(ui, "Open real terminal")
                                    .on_hover_text("A full shell (vim, -it, watch) with the same environment")
                                    .clicked()
                                {
                                    action = Some(Action::OpenTerminal);
                                }
                                if tiny(ui, "Clear").clicked() {
                                    self.entries.retain(|e| e.running);
                                }
                                if let Some(e) = env
                                    && tiny(ui, "Copy env")
                                        .on_hover_text("Shell exports to paste in your own terminal")
                                        .clicked()
                                {
                                    ui.ctx().copy_text(e.export_script());
                                }
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                    ui.label(RichText::new(format!("{m} · kind-poddojo")).size(12.5).color(DIM));
                                });
                            });
                        });
                    });
                let input_h = 40.0;
                let chips_h = if chips.is_empty() { 0.0 } else { 46.0 };
                egui::Frame::new().inner_margin(Margin::symmetric(14, 8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::both()
                        .id_salt("terminal-scroll")
                        .max_height((ui.available_height() - input_h - chips_h - 16.0).max(60.0))
                        .min_scrolled_height((ui.available_height() - input_h - chips_h - 16.0).max(60.0))
                        .auto_shrink([false, false])
                        .stick_to_bottom(self.stick_to_bottom)
                        .show(ui, |ui| {
                            if self.entries.is_empty() {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(
                                        "Commands run on your chosen machine; kubectl talks only to the lab cluster.",
                                    )
                                    .monospace()
                                    .size(12.5)
                                    .color(DIM),
                                );
                                ui.label(
                                    RichText::new("Type below and press Enter. ↑ recalls history.")
                                        .monospace()
                                        .size(12.5)
                                        .color(DIM),
                                );
                            }
                            for e in &self.entries {
                                self.entry_ui(ui, e, jobs);
                            }
                        });
                });
                // Quick commands: one scrollable row, so the prompt below always stays in view.
                if !chips.is_empty() {
                    egui::Frame::new().inner_margin(Margin { left: 14, right: 14, top: 0, bottom: 6 }).show(ui, |ui| {
                        egui::ScrollArea::horizontal().id_salt("terminal-chips").show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                                for c in chips {
                                    let resp = ui.add(
                                        egui::Button::new(RichText::new(*c).monospace().size(11.5).color(TEXT))
                                            .fill(BAR)
                                            .stroke(Stroke::new(1.0, BORDER))
                                            .corner_radius(CornerRadius::same(8)),
                                    );
                                    if resp.on_hover_text("Run").clicked() {
                                        action = Some(Action::Run((*c).to_string()));
                                    }
                                }
                            });
                        });
                    });
                }
                // Prompt.
                egui::Frame::new()
                    .fill(BAR)
                    .corner_radius(CornerRadius { nw: 0, ne: 0, sw: 12, se: 12 })
                    .inner_margin(Margin::symmetric(14, 9))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&m).monospace().color(PROMPT));
                            ui.label(RichText::new(&folder).monospace().color(PATH));
                            ui.label(RichText::new("$").monospace().color(TEXT));
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut self.input)
                                    .font(egui::TextStyle::Monospace)
                                    .text_color(Color32::WHITE)
                                    .frame(egui::Frame::NONE)
                                    .hint_text(RichText::new("docker ps · kubectl get pods -A").color(DIM))
                                    .desired_width(f32::INFINITY),
                            );
                            if std::mem::take(&mut self.focus) {
                                resp.request_focus();
                            }
                            if resp.has_focus() {
                                self.history(ui);
                            }
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let cmd = self.input.trim().to_string();
                                if !cmd.is_empty() {
                                    action = Some(Action::Run(cmd));
                                    self.input.clear();
                                    self.history_pos = None;
                                }
                                resp.request_focus();
                            }
                        });
                    });
            },
        );
        let _ = t;
        if !self.entries.iter().any(|e| e.running) {
            self.stick_to_bottom = false;
        }
        action
    }

    fn entry_ui(&self, ui: &mut egui::Ui, e: &Entry, jobs: &Jobs) {
        ui.add_space(6.0);
        let (pill, color) = match (e.running, e.status) {
            (true, _) => (format!("{:.0}s", e.started.elapsed().as_secs_f32()), RUN),
            (false, Some(0)) => (took(e.took), OK),
            (false, Some(code)) => (format!("exit {code}"), ERR),
            (false, None) => ("killed".into(), ERR),
        };
        let (m, f) = e.place.split_once(' ').unwrap_or((e.place.as_str(), ""));
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = f32::INFINITY;
        let fmt = |c: Color32| egui::TextFormat { font_id: FontId::monospace(13.0), color: c, ..Default::default() };
        job.append(m, 0.0, fmt(PROMPT));
        job.append(f, 6.0, fmt(PATH));
        job.append("$", 6.0, fmt(TEXT));
        job.append(e.cmd.trim(), 6.0, fmt(Color32::WHITE));
        ui.horizontal(|ui| {
            egui::Frame::new()
                .fill(color.gamma_multiply(0.16))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(6, 0))
                .show(ui, |ui| {
                    ui.label(RichText::new(format!("{pill:^7}")).monospace().size(11.0).color(color));
                });
            ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Extend));
        });
        let text = if e.running { e.job.and_then(|j| jobs.live(j)).unwrap_or_default() } else { e.output.clone() };
        if !text.trim().is_empty() {
            let mut out = egui::text::LayoutJob::default();
            out.wrap.max_width = f32::INFINITY;
            out.append(
                text.trim_end(),
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(12.5),
                    color: if e.status.is_some_and(|s| s != 0) { Color32::from_rgb(252, 165, 165) } else { TEXT },
                    ..Default::default()
                },
            );
            ui.add(egui::Label::new(out).wrap_mode(egui::TextWrapMode::Extend));
        }
    }

    fn history(&mut self, ui: &egui::Ui) {
        let (up, down) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown)));
        if !(up || down) {
            return;
        }
        let n = self.entries.len();
        if n == 0 {
            return;
        }
        let pos = match (self.history_pos, up) {
            (None, _) => n - 1,
            (Some(p), true) => p.saturating_sub(1),
            (Some(p), false) => (p + 1).min(n - 1),
        };
        self.history_pos = Some(pos);
        if let Some(e) = self.entries.get(pos) {
            self.input = e.cmd.clone();
        }
    }
}

fn took(d: Option<Duration>) -> String {
    match d {
        Some(d) if d.as_secs_f32() < 1.0 => format!("{}ms", d.as_millis()),
        Some(d) => format!("{:.1}s", d.as_secs_f32()),
        None => String::new(),
    }
}

fn tiny(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(11.5).color(TEXT))
            .fill(Color32::from_rgb(24, 30, 58))
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(7)),
    )
}
