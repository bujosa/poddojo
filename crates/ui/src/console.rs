//! The built-in console: type `docker`/`kubectl` commands and see their output, run on the
//! chosen machine with the lab environment. Non-interactive (no `-it`): for that, "Open terminal".

use egui::{Color32, RichText};
use poddojo_core::CommandOutput;
use poddojo_runner::LabEnv;

use crate::jobs::Jobs;
use crate::theme::Tokens;

pub struct Entry {
    /// Stable id: completions find their entry by id, never by position (Clear and the 200-entry
    /// cap shift positions while commands are still running).
    pub id: u64,
    pub cmd: String,
    pub output: String,
    pub status: Option<i32>,
    pub running: bool,
    pub job: Option<u64>,
}

#[derive(Default)]
pub struct Console {
    pub open: bool,
    pub entries: Vec<Entry>,
    pub input: String,
    history_pos: Option<usize>,
    stick_to_bottom: bool,
    next_id: u64,
}

impl Console {
    pub fn push(&mut self, cmd: &str) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.entries.push(Entry {
            id,
            cmd: cmd.to_string(),
            output: String::new(),
            status: None,
            running: true,
            job: None,
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
            e.output = out.combined();
            if out.timed_out {
                e.output.push_str("\n(timed out)");
            }
        }
        self.stick_to_bottom = true;
    }

    /// Draw the console; returns a command to run when the user submits one.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        t: &Tokens,
        jobs: &Jobs,
        env: Option<&LabEnv>,
        machine: &str,
    ) -> Option<String> {
        let mut run = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Console").strong().color(Color32::from_rgb(199, 210, 254)));
            let place = env
                .map(|e| e.workdir.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default())
                .unwrap_or_default();
            ui.label(
                RichText::new(format!("machine {machine} · cluster kind-poddojo · folder {place}"))
                    .size(12.0)
                    .color(Color32::from_rgb(120, 130, 170)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Clear").clicked() {
                    self.entries.retain(|e| e.running);
                }
                if let Some(e) = env
                    && ui
                        .small_button("Copy env")
                        .on_hover_text("Shell exports to paste in your own terminal")
                        .clicked()
                {
                    ui.ctx().copy_text(e.export_script());
                }
            });
        });
        let input_height = 34.0;
        egui::ScrollArea::vertical()
            .id_salt("console-scroll")
            .max_height((ui.available_height() - input_height).max(40.0))
            .auto_shrink([false, false])
            .stick_to_bottom(self.stick_to_bottom)
            .show(ui, |ui| {
                for e in &self.entries {
                    let (mark, color) = match (e.running, e.status) {
                        (true, _) => ("…", t.amber),
                        (false, Some(0)) => ("$", t.good),
                        _ => ("✖", t.bad),
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(mark).monospace().color(color));
                        ui.label(RichText::new(&e.cmd).monospace().color(Color32::WHITE));
                    });
                    let text =
                        if e.running { e.job.and_then(|j| jobs.live(j)).unwrap_or_default() } else { e.output.clone() };
                    if !text.is_empty() {
                        ui.label(RichText::new(text.trim_end()).monospace().color(t.code_text));
                    }
                }
            });
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.input)
                .font(egui::TextStyle::Monospace)
                .hint_text("docker ps   ·   kubectl get pods -A   ·   Enter to run, ↑ for history")
                .desired_width(f32::INFINITY),
        );
        if resp.has_focus() {
            let (up, down) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown)));
            if up || down {
                let n = self.entries.len();
                if n > 0 {
                    let pos = match (self.history_pos, up) {
                        (None, true) => n - 1,
                        (Some(p), true) => p.saturating_sub(1),
                        (Some(p), false) => (p + 1).min(n - 1),
                        (None, false) => n - 1,
                    };
                    self.history_pos = Some(pos);
                    if let Some(e) = self.entries.get(pos) {
                        self.input = e.cmd.clone();
                    }
                }
            }
        }
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let cmd = self.input.trim().to_string();
            if !cmd.is_empty() {
                run = Some(cmd);
                self.input.clear();
                self.history_pos = None;
            }
            resp.request_focus();
        }
        if !self.entries.iter().any(|e| e.running) {
            self.stick_to_bottom = false;
        }
        run
    }
}
