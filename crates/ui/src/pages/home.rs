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
                    if streak > 0 { format!("   ·   {streak}-day streak") } else { String::new() }
                ),
            );

            self.continue_card(ui, &t);
            ui.add_space(12.0);
            self.review_strip(ui, &t);
            self.labs_card(ui, &t);

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
        let (done, total) = self.progress.done_count(&self.catalog);
        theme::card(t)
            .stroke(egui::Stroke::new(1.5, t.accent.gamma_multiply(0.8)))
            .inner_margin(egui::Margin::same(22))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    let ring = 150.0;
                    ui.allocate_ui_with_layout(
                        egui::vec2((ui.available_width() - ring - 30.0).max(200.0), 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            if let Some(exam) = self.progress.active_exam.clone() {
                                ui.label(RichText::new("EXAM IN PROGRESS").size(12.0).strong().color(t.amber));
                                let name =
                                    self.catalog.exam(&exam.exam_id).map_or(exam.exam_id.clone(), |e| e.title.clone());
                                ui.label(RichText::new(name).size(24.0).strong().color(strong(t)));
                                ui.add_space(8.0);
                                if accent_button(ui, t, "Back to the exam").clicked() {
                                    self.page = Page::Exams;
                                }
                                return;
                            }
                            let Some(l) = resume.clone() else {
                                ui.label(RichText::new("EVERY LESSON DONE").size(12.0).strong().color(t.good));
                                ui.label(
                                    RichText::new("Now prove it under exam conditions")
                                        .size(24.0)
                                        .strong()
                                        .color(strong(t)),
                                );
                                ui.add_space(8.0);
                                if accent_button(ui, t, "Take a mock exam").clicked() {
                                    self.page = Page::Exams;
                                }
                                return;
                            };
                            let p = self.progress.lesson(&l.id).cloned().unwrap_or_default();
                            ui.label(
                                RichText::new(if p.last_seen > 0 {
                                    "CONTINUE WHERE YOU LEFT OFF"
                                } else {
                                    "START HERE"
                                })
                                .size(12.0)
                                .strong()
                                .color(t.accent),
                            );
                            ui.label(RichText::new(&l.title).size(26.0).strong().color(strong(t)));
                            ui.label(
                                RichText::new(format!(
                                    "{} · {} · ~{} min{}",
                                    if l.track == "ckad" { "CKAD" } else { "Docker Foundations" },
                                    self.catalog.module_title(&l),
                                    l.minutes,
                                    if p.last_seen > 0 {
                                        format!(" · opened {}", ago(p.last_seen))
                                    } else {
                                        String::new()
                                    }
                                ))
                                .color(t.muted),
                            );
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 18.0;
                                checkpoint(ui, t, "Concepts", p.read);
                                if !l.quiz.is_empty() {
                                    checkpoint(
                                        ui,
                                        t,
                                        &format!("Quiz · {} questions", l.quiz.len()),
                                        p.quiz.is_some_and(|q| q.passed()),
                                    );
                                }
                                if let Some(lab) = &l.lab
                                    && !lab.steps.is_empty()
                                {
                                    let n = p.steps.len().min(lab.steps.len());
                                    checkpoint(
                                        ui,
                                        t,
                                        &format!("Lab · {n}/{} steps", lab.steps.len()),
                                        n == lab.steps.len(),
                                    );
                                }
                            });
                            ui.add_space(14.0);
                            ui.horizontal(|ui| {
                                let lab_open = l.has_lab() && (p.prepared_at.is_some() || !p.steps.is_empty());
                                if accent_button(ui, t, if lab_open { "Resume lab" } else { "Continue" }).clicked() {
                                    self.open_lesson(&l.id);
                                }
                                let next: Vec<(String, String)> = self
                                    .catalog
                                    .ordered_lessons()
                                    .skip_while(|x| x.id != l.id)
                                    .skip(1)
                                    .filter(|x| !self.progress.is_done(x))
                                    .take(2)
                                    .map(|x| (x.id.clone(), x.title.clone()))
                                    .collect();
                                if !next.is_empty() {
                                    ui.add_space(10.0);
                                    ui.label(RichText::new("Up next").color(t.muted));
                                    for (k, (id, title)) in next.into_iter().enumerate() {
                                        if k > 0 {
                                            ui.label(RichText::new("·").color(t.muted));
                                        }
                                        if ui.link(RichText::new(title).color(t.text)).clicked() {
                                            self.open_lesson(&id);
                                        }
                                    }
                                }
                            });
                        },
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        let f = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                        theme::ring(
                            ui,
                            t,
                            f,
                            ring,
                            &format!("{:.0}%", f * 100.0),
                            &format!("{done} of {total} lessons"),
                            t.accent,
                        );
                    });
                });
            });
    }

    /// Labs started but not finished, so they can be resumed in one click.
    fn labs_card(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        let labs: Vec<poddojo_core::Lesson> =
            self.progress.labs_in_progress(&self.catalog).into_iter().take(4).cloned().collect();
        if labs.is_empty() {
            return;
        }
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Labs in progress").size(16.0).strong());
                ui.label(
                    RichText::new("pick up exactly where you stopped: prepared state, hints and last checks are kept")
                        .size(12.5)
                        .color(t.muted),
                );
            });
            ui.add_space(6.0);
            for l in &labs {
                let p = self.progress.lesson(&l.id).cloned().unwrap_or_default();
                let n = l.lab.as_ref().map_or(0, |x| x.steps.len());
                let done = p.steps.len().min(n);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 9.0, t.amber.gamma_multiply(0.14));
                    crate::icons::paint(ui.painter(), rect.shrink(9.0), crate::icons::Icon::Terminal, t.amber);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&l.title).size(14.5).strong().color(strong(t)));
                        let when = p.prepared_at.map_or(String::new(), |ts| format!(" · prepared {}", ago(ts)));
                        ui.label(RichText::new(format!("{done}/{n} steps{when}")).size(12.5).color(t.muted));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if accent_button(ui, t, "Resume lab").clicked() {
                            self.open_lesson(&l.id);
                            self.learn.tab = crate::LessonTab::Lab;
                        }
                        let w = 140.0;
                        theme::bar(ui, t, if n == 0 { 0.0 } else { done as f32 / n as f32 }, w, t.amber);
                    });
                });
                ui.add_space(4.0);
            }
        });
        ui.add_space(12.0);
    }

    /// Due reviews, when there are any: the daily habit that keeps what was learned.
    fn review_strip(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        let stats = self.progress.deck.stats(&self.catalog, poddojo_core::now());
        let due = stats.due_questions + stats.due_tasks;
        if due == 0 {
            return;
        }
        theme::card(t).stroke(egui::Stroke::new(1.2, t.amber.gamma_multiply(0.7))).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                crate::icons::show(ui, crate::icons::Icon::Repeat, 24.0, t.amber);
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("{due} due for review")).size(16.0).strong());
                    let mut parts = Vec::new();
                    if stats.due_questions > 0 {
                        parts.push(format!("{} question(s)", stats.due_questions));
                    }
                    if stats.due_tasks > 0 {
                        parts.push(format!("{} mock-exam task drill(s)", stats.due_tasks));
                    }
                    ui.label(
                        RichText::new(format!("{}: a few minutes now saves relearning later.", parts.join(" and ")))
                            .size(12.5)
                            .color(t.muted),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if accent_button(ui, t, "Review now").clicked() {
                        self.page = Page::Review;
                        if stats.due_questions > 0 {
                            self.start_session(false);
                        }
                    }
                });
            });
        });
        ui.add_space(12.0);
    }

    fn readiness_card(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        use poddojo_core::Verdict;
        let r = self.progress.exam_readiness(&self.catalog);
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let color = match r.verdict {
                Verdict::Ready => t.good,
                Verdict::Close => t.accent,
                Verdict::NotYet => t.amber,
                Verdict::KeepStudying | Verdict::TakeAMock => t.muted,
            };
            ui.label(RichText::new("WOULD YOU PASS THE CKAD TODAY?").size(11.5).strong().color(t.muted));
            ui.horizontal(|ui| {
                ui.label(RichText::new(r.verdict.title()).size(22.0).strong().color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match r.recent {
                    Some(p) => {
                        ui.label(RichText::new("recent fresh mocks").size(11.5).color(t.muted));
                        ui.label(RichText::new(format!("{p:.0}%")).size(22.0).strong().color(color));
                    }
                    None => {
                        ui.label(RichText::new("no mock yet").size(11.5).color(t.muted));
                    }
                });
            });
            ui.label(RichText::new(&r.advice).size(13.0));
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "Curriculum {:.0}% · {} of {} mocks taken fresh · pass mark 66%, aim for 80%",
                    r.coverage,
                    r.fresh.len(),
                    self.catalog.exams.len()
                ))
                .size(12.0)
                .color(t.muted),
            );
            ui.add_space(6.0);
            let w = (ui.available_width() - 8.0).max(80.0);
            for d in self.progress.readiness(&self.catalog) {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&d.title).size(13.0));
                    if d.weak() {
                        theme::chip(ui, "weak", t.amber, t.amber.gamma_multiply(0.12));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let exam = d.recent_exam.map_or("no mock".to_string(), |e| format!("mocks {e:.0}%"));
                        ui.label(
                            RichText::new(format!(
                                "{}/{} lessons · {exam} · {}% of exam",
                                d.lessons_done, d.lessons_total, d.weight
                            ))
                            .size(11.0)
                            .color(t.muted),
                        );
                    });
                });
                theme::bar(ui, t, d.coverage(), w, t.accent);
                if let Some(e) = d.recent_exam {
                    theme::bar(ui, t, e / 100.0, w, if d.weak() { t.amber } else { t.good });
                }
            }
        });
    }

    fn environment_card(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        use crate::icons::{self, Icon};
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Your lab").size(16.0).strong());
            ui.add_space(8.0);
            // Machine.
            let (m_color, m_title, m_sub) = match self.target() {
                Some(target) => {
                    let place = if target.is_remote() {
                        target.endpoint.trim_start_matches("ssh://").to_string()
                    } else {
                        "this computer".into()
                    };
                    match self.engine.get(&target.name) {
                        Some(Ok(i)) => (
                            t.good,
                            format!("{} · {place}", target.name),
                            format!(
                                "Docker {} · {} · {} CPUs · {:.1} GB · {} running / {} containers",
                                i.server_version,
                                i.arch,
                                i.cpus,
                                i.memory as f64 / 1e9,
                                i.running,
                                i.containers
                            ),
                        ),
                        Some(Err(e)) => (t.bad, format!("{} · {place}", target.name), format!("Not answering: {e}")),
                        None => (t.muted, format!("{} · {place}", target.name), "checking…".into()),
                    }
                }
                None => (
                    t.muted,
                    "No machine yet".into(),
                    self.targets_error.clone().unwrap_or_else(|| "Looking for Docker…".into()),
                ),
            };
            status_row(ui, t, Icon::Server, m_color, &m_title, &m_sub);
            ui.add_space(8.0);
            // Cluster.
            let (c_color, c_title, c_sub) = match &self.cluster {
                Some(Status::Running { nodes }) => (
                    t.good,
                    "kind-poddojo is running".to_string(),
                    format!("{} nodes · Kubernetes {}", nodes.len(), nodes.first().map_or("", |n| n.version.as_str())),
                ),
                Some(Status::Missing) => {
                    (t.amber, "No lab cluster yet".into(), "The CKAD track needs one: about a minute to create".into())
                }
                Some(Status::Unreachable(e)) => (t.bad, "Lab cluster unreachable".into(), e.clone()),
                None => (t.muted, "Lab cluster".into(), "checking…".into()),
            };
            status_row(ui, t, Icon::Cluster, c_color, &c_title, &c_sub);
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if matches!(self.cluster, Some(Status::Missing)) && accent_button(ui, t, "Create lab cluster").clicked()
                {
                    self.page = Page::Cluster;
                }
                if icons::button(ui, Icon::Terminal, "Open terminal").clicked() {
                    self.open_terminal();
                }
                if icons::button(ui, Icon::Container, "Explore Docker").clicked() {
                    self.page = Page::Docker;
                }
            });
            ui.add_space(14.0);
            ui.label(RichText::new("Last 14 days").size(12.5).color(t.muted));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let today = chrono::Local::now().date_naive();
                for back in (0..14).rev() {
                    let day = today - chrono::Duration::days(back);
                    let active = self.progress.active_days.contains(&day.format("%Y-%m-%d").to_string());
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 4.0, if active { t.accent } else { t.border });
                    resp.on_hover_text(day.format("%a %d %b").to_string());
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

fn strong(t: &Tokens) -> egui::Color32 {
    if t.dark { egui::Color32::WHITE } else { egui::Color32::BLACK }
}

/// "✔ Concepts" or "○ Quiz": one of the three things that complete a lesson.
fn checkpoint(ui: &mut egui::Ui, t: &Tokens, label: &str, done: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
        let c = rect.center();
        if done {
            ui.painter().circle_filled(c, 8.0, t.good);
            crate::icons::paint(ui.painter(), rect.shrink(4.0), crate::icons::Icon::Check, t.bg);
        } else {
            ui.painter().circle_stroke(c, 7.5, egui::Stroke::new(1.4, t.muted));
        }
        ui.label(RichText::new(label).color(if done { t.good } else { t.text }));
    });
}

/// An icon tile with a status colour, a title and a detail line.
fn status_row(ui: &mut egui::Ui, t: &Tokens, icon: crate::icons::Icon, color: egui::Color32, title: &str, sub: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 10.0, color.gamma_multiply(0.14));
        crate::icons::paint(ui.painter(), rect.shrink(10.0), icon, color);
        ui.vertical(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            ui.label(RichText::new(title).size(15.0).strong().color(strong(t)));
            ui.label(RichText::new(sub).size(12.5).color(t.muted));
        });
    });
}
