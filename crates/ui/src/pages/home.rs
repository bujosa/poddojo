use egui::RichText;
use poddojo_runner::cluster::Status;

use super::{accent_button, ago, title};
use crate::theme::{self, Tokens};
use crate::{App, Page};

impl App {
    pub fn home(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let streak = self.progress.streak(chrono::Local::now().date_naive());
            let greeting = if self.progress.lessons.is_empty() { "Welcome to the dojo" } else { "Welcome back" };
            title(
                ui,
                &t,
                greeting,
                &format!(
                    "Train with real containers on your own machines. Pass the CKAD.{}",
                    if streak > 0 { format!("   🔥 {streak}-day streak") } else { String::new() }
                ),
            );

            self.continue_card(ui, &t);
            ui.add_space(12.0);

            ui.columns(2, |cols| {
                self.readiness_card(&mut cols[0], &t);
                self.environment_card(&mut cols[1], &t);
            });
            ui.add_space(12.0);
            ui.columns(2, |cols| {
                self.tracks_card(&mut cols[0], &t);
                self.exams_card(&mut cols[1], &t);
            });
        });
    }

    fn continue_card(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        let resume = self.progress.resume(&self.catalog).cloned();
        theme::card(t).fill(t.card).stroke(egui::Stroke::new(1.5, t.accent)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some(exam) = self.progress.active_exam.clone() {
                ui.label(RichText::new("⏱ Exam in progress").color(t.amber).strong());
                let name = self.catalog.exam(&exam.exam_id).map_or(exam.exam_id.clone(), |e| e.title.clone());
                ui.label(RichText::new(name).size(20.0).strong());
                if accent_button(ui, t, "Back to the exam").clicked() {
                    self.page = Page::Exams;
                }
                return;
            }
            match resume {
                Some(l) => {
                    let p = self.progress.lesson(&l.id).cloned().unwrap_or_default();
                    ui.label(
                        RichText::new(if p.last_seen > 0 { "Continue where you left off" } else { "Start here" })
                            .color(t.accent)
                            .strong(),
                    );
                    ui.label(RichText::new(&l.title).size(22.0).strong());
                    ui.label(
                        RichText::new(format!(
                            "{} · {} · ~{} min",
                            self.catalog.track(&l.track).map_or("", |t| t.title.as_str()),
                            self.catalog.module_title(&l),
                            l.minutes
                        ))
                        .color(t.muted),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let step = |done: bool, text: &str| {
                            RichText::new(format!("{} {text}", if done { "✔" } else { "○" })).color(if done {
                                t.good
                            } else {
                                t.muted
                            })
                        };
                        ui.label(step(p.read, "Concepts"));
                        if !l.quiz.is_empty() {
                            ui.label(step(p.quiz.is_some_and(|q| q.passed()), &format!("Quiz ({} q)", l.quiz.len())));
                        }
                        if let Some(lab) = &l.lab {
                            ui.label(step(
                                p.steps.len() == lab.steps.len() && !lab.steps.is_empty(),
                                &format!("Lab {}/{}", p.steps.len(), lab.steps.len()),
                            ));
                        }
                        if p.last_seen > 0 {
                            ui.label(RichText::new(format!("· last opened {}", ago(p.last_seen))).color(t.muted));
                        }
                    });
                    ui.add_space(6.0);
                    if accent_button(ui, t, "▶  Continue").clicked() {
                        self.open_lesson(&l.id);
                    }
                }
                None if !self.catalog.lessons.is_empty() => {
                    ui.label(RichText::new("Every lesson is done 🥋").size(20.0).strong());
                    ui.label("Now prove it under exam conditions.");
                    if accent_button(ui, t, "Take a mock exam").clicked() {
                        self.page = Page::Exams;
                    }
                }
                None => {
                    ui.label("No lessons found.");
                }
            }
        });
    }

    fn readiness_card(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let overall = self.progress.overall_readiness(&self.catalog);
            ui.horizontal(|ui| {
                ui.label(RichText::new("CKAD readiness").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{overall:.0}%")).size(22.0).strong().color(if overall >= 66.0 {
                        t.good
                    } else {
                        t.amber
                    }));
                });
            });
            ui.label(
                RichText::new(format!(
                    "Weighted like the exam (curriculum {}). Lessons count half, mock exams the other half.",
                    poddojo_core::CKAD_CURRICULUM_VERSION
                ))
                .small()
                .color(t.muted),
            );
            ui.add_space(4.0);
            let w = (ui.available_width() - 8.0).max(80.0);
            for r in self.progress.readiness(&self.catalog) {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&r.title).size(13.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{}% of exam", r.weight)).size(11.0).color(t.muted));
                    });
                });
                theme::bar(ui, t, r.score(), w, t.accent);
            }
        });
    }

    fn environment_card(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Your lab").strong());
            ui.add_space(4.0);
            match self.target() {
                Some(target) => {
                    let info = self.engine.get(&target.name);
                    ui.label(RichText::new(format!("🖧 {}", target.name)).size(17.0).strong());
                    ui.label(
                        RichText::new(if target.is_remote() {
                            target.endpoint.clone()
                        } else {
                            "this computer".into()
                        })
                        .color(t.muted),
                    );
                    match info {
                        Some(Ok(i)) => {
                            ui.label(
                                RichText::new(format!(
                                    "Docker {} · {} · {} · {} CPUs · {:.1} GB",
                                    i.server_version,
                                    i.os,
                                    i.arch,
                                    i.cpus,
                                    i.memory as f64 / 1e9
                                ))
                                .color(t.good),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} containers ({} running) · {} images",
                                    i.containers, i.running, i.images
                                ))
                                .color(t.muted),
                            );
                        }
                        Some(Err(e)) => {
                            ui.label(RichText::new(format!("Not answering: {e}")).color(t.bad));
                        }
                        None => {
                            ui.label(RichText::new("checking…").color(t.muted));
                        }
                    }
                }
                None => {
                    ui.label(
                        RichText::new(self.targets_error.clone().unwrap_or_else(|| "Looking for Docker…".into()))
                            .color(t.muted),
                    );
                }
            }
            ui.add_space(6.0);
            match &self.cluster {
                Some(Status::Running { nodes }) => {
                    ui.label(
                        RichText::new(format!(
                            "☸ Lab cluster running · {} nodes · {}",
                            nodes.len(),
                            nodes.first().map_or("", |n| n.version.as_str())
                        ))
                        .color(t.good),
                    );
                }
                Some(Status::Missing) => {
                    ui.label(RichText::new("☸ No lab cluster yet: the CKAD track needs one").color(t.amber));
                    if ui.button("Create the lab cluster").clicked() {
                        self.page = Page::Cluster;
                    }
                }
                Some(Status::Unreachable(e)) => {
                    ui.label(RichText::new(format!("☸ Lab cluster unreachable: {e}")).color(t.bad));
                }
                None => {}
            }
            ui.horizontal(|ui| {
                if ui.button("Open terminal").clicked() {
                    self.open_terminal();
                }
                if ui.button("Explore Docker").clicked() {
                    self.page = Page::Docker;
                }
            });
        });
    }

    fn tracks_card(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Tracks").strong());
            let tracks = self.catalog.tracks.clone();
            for track in &tracks {
                let (d, n) = self.progress.track_counts(&self.catalog, &track.id);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&track.title).size(15.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{d}/{n}")).color(t.muted));
                    });
                });
                ui.label(RichText::new(&track.summary).small().color(t.muted));
                let w = ui.available_width();
                theme::bar(ui, t, if n == 0 { 0.0 } else { d as f32 / n as f32 }, w, t.accent);
            }
            ui.add_space(6.0);
            if ui.button("Browse lessons").clicked() {
                self.page = Page::Learn;
            }
        });
    }

    fn exams_card(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Mock exams").strong());
            ui.label(
                RichText::new(
                    "Timed, performance-based, graded on your lab cluster, like the real CKAD (66% to pass).",
                )
                .small()
                .color(t.muted),
            );
            ui.add_space(4.0);
            for e in self.catalog.exams.clone() {
                ui.horizontal(|ui| {
                    ui.label(&e.title);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        match self.progress.best_attempt(&e.id) {
                            Some(a) => {
                                ui.label(RichText::new(format!("best {:.0}%", a.percent)).color(if a.passed {
                                    t.good
                                } else {
                                    t.amber
                                }));
                            }
                            None => {
                                ui.label(RichText::new("not taken").color(t.muted));
                            }
                        }
                    });
                });
            }
            if let Some(last) = self.progress.exams.last() {
                ui.label(
                    RichText::new(format!("Last: {} · {:.0}% · {}", last.title, last.percent, ago(last.finished_at)))
                        .small()
                        .color(t.muted),
                );
            }
            ui.add_space(6.0);
            if ui.button("Go to exams").clicked() {
                self.page = Page::Exams;
            }
        });
    }
}
