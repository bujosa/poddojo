use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use egui::{Color32, RichText};
use poddojo_core::{Exam, ExamAttempt, exam, now};
use poddojo_runner::{cluster, lab};

use super::{DIVIDER, DRAWER_BELOW, INSTRUCTIONS_MIN, TERMINAL_MIN, accent_button, ago, domain_title, title};
use crate::icons::{self, Icon};
use crate::jobs::Done;
use crate::markdown::{self, Action};
use crate::theme::{self, Tokens};
use crate::{App, Drill, Page};

impl App {
    pub fn exams_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        let ctx = ui.ctx().clone();
        if let Some(active) = self.progress.active_exam.clone()
            && let Some(e) = self.catalog.exam(&active.exam_id).cloned()
        {
            self.exam_workspace(ui, &t, &ctx, &e, active.started_at);
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if let Some(result) = self.exam.result.clone() {
                self.exam_result(ui, &t, &result);
                ui.add_space(16.0);
            }
            if self.exam.drill.is_some() {
                self.drill(ui, &t);
                return;
            }
            title(ui, &t, "Exams", "Performance-based mock exams graded on your lab cluster, under the real exam's rules: 2 hours, ~66% to pass, kubernetes.io docs allowed, no hints.");
            if !self.cluster_running() {
                theme::card(&t).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Mock exams need the lab cluster.").color(t.amber));
                    if ui.button("Go to Cluster").clicked() {
                        self.page = Page::Cluster;
                    }
                });
                ui.add_space(8.0);
            }
            for e in self.catalog.exams.clone() {
                theme::card(&t).inner_margin(egui::Margin::same(20)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let best = self.progress.best_attempt(&e.id).cloned();
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2((ui.available_width() - 150.0).max(240.0), 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.label(RichText::new(&e.title).size(19.0).strong().color(strong(&t)));
                                ui.label(
                                    RichText::new(format!("{} tasks · {} minutes · {}% to pass", e.tasks.len(), e.minutes, e.pass_percent))
                                        .color(t.muted),
                                );
                                ui.add_space(6.0);
                                if !e.summary.is_empty() {
                                    ui.label(&e.summary);
                                }
                                ui.add_space(8.0);
                                let mut weights: BTreeMap<&str, u32> = BTreeMap::new();
                                for task in &e.tasks {
                                    *weights.entry(task.domain.as_str()).or_default() += task.weight;
                                }
                                ui.horizontal_wrapped(|ui| {
                                    for d in poddojo_core::DOMAINS {
                                        if let Some(w) = weights.get(d.id) {
                                            theme::chip(ui, &format!("{} · {w}%", short_domain(d.id)), t.muted, t.card_hover);
                                        }
                                    }
                                });
                                let attempts: Vec<&ExamAttempt> = self.progress.exams.iter().filter(|a| a.exam_id == e.id).collect();
                                if !attempts.is_empty() {
                                    ui.add_space(4.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(RichText::new("Attempts").size(12.5).color(t.muted));
                                        for a in attempts.iter().rev().take(6) {
                                            let resp = ui.add(
                                                egui::Label::new(
                                                    RichText::new(format!("{:.0}% · {}", a.percent, ago(a.finished_at)))
                                                        .size(12.5)
                                                        .color(if a.passed { t.good } else { t.amber }),
                                                )
                                                .sense(egui::Sense::click()),
                                            );
                                            if resp.on_hover_text("Review this attempt").clicked() {
                                                self.exam.result = Some((*a).clone());
                                            }
                                        }
                                    });
                                }
                            },
                        );
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            match &best {
                                Some(a) => theme::ring(
                                    ui,
                                    &t,
                                    a.percent / 100.0,
                                    104.0,
                                    &format!("{:.0}%", a.percent),
                                    "best",
                                    if a.passed { t.good } else { t.amber },
                                ),
                                None => theme::ring(ui, &t, 0.0, 104.0, "–", "not taken", t.muted),
                            }
                            ui.add_space(6.0);
                            let busy = self.jobs.busy("exam");
                            ui.add_enabled_ui(self.cluster_running() && !busy, |ui| {
                                if accent_button(ui, &t, if best.is_some() { "Retake" } else { "Start exam" }).clicked() {
                                    self.start_exam(&ctx, &e);
                                }
                            });
                            if busy {
                                ui.spinner();
                            }
                        });
                    });
                });
                ui.add_space(10.0);
            }
            if !self.exam.log.is_empty() {
                egui::CollapsingHeader::new("Last exam log").show(ui, |ui| {
                    ui.label(RichText::new(self.exam.log.trim_end()).monospace().size(12.0));
                });
            }
            ui.add_space(8.0);
            theme::card(&t).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Knowledge drill").size(18.0).strong());
                ui.label(RichText::new("20 random questions from the lessons you have opened. Quick, no cluster needed: good for a coffee break.").color(t.muted));
                if ui.button("Start a drill").clicked() {
                    self.start_drill();
                }
            });
        });
    }

    fn start_drill(&mut self) {
        let mut pool: Vec<(String, usize)> = Vec::new();
        for (id, l) in &self.catalog.lessons {
            if self.progress.lesson(id).is_some() {
                pool.extend((0..l.quiz.len()).map(|i| (id.clone(), i)));
            }
        }
        if pool.len() < 5 {
            pool = self
                .catalog
                .lessons
                .iter()
                .flat_map(|(id, l)| (0..l.quiz.len()).map(move |i| (id.clone(), i)))
                .collect();
        }
        // Shuffle with a small LCG seeded by the clock (no rand dependency).
        let mut seed = (now() as u64).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        for i in (1..pool.len()).rev() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            pool.swap(i, (seed >> 33) as usize % (i + 1));
        }
        pool.truncate(20);
        let n = pool.len();
        self.exam.drill = Some(Drill { questions: pool, answers: vec![None; n], submitted: false });
    }

    fn drill(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        let Some(mut drill) = self.exam.drill.take() else { return };
        title(ui, t, "Knowledge drill", &format!("{} questions", drill.questions.len()));
        let mut close = false;
        for (i, (lesson, qi)) in drill.questions.iter().enumerate() {
            let Some(q) = self.catalog.lesson(lesson).and_then(|l| l.quiz.get(*qi)) else { continue };
            theme::card(t).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(format!("{}. {}", i + 1, q.q)).strong());
                for (pos, j) in q.order(lesson).into_iter().enumerate() {
                    let Some(c) = q.choices.get(j) else { continue };
                    let picked = drill.answers.get(i).copied().flatten() == Some(j);
                    let state = match (drill.submitted, picked, j == q.answer) {
                        (false, true, _) => theme::ChoiceState::Picked,
                        (false, false, _) | (true, false, false) => theme::ChoiceState::Idle,
                        (true, true, true) => theme::ChoiceState::Correct,
                        (true, true, false) => theme::ChoiceState::Wrong,
                        (true, false, true) => theme::ChoiceState::Missed,
                    };
                    if theme::choice(ui, t, (b'A' + pos as u8) as char, c, state, !drill.submitted).clicked()
                        && let Some(a) = drill.answers.get_mut(i)
                    {
                        *a = Some(j);
                    }
                    ui.add_space(4.0);
                }
                if drill.submitted {
                    ui.label(RichText::new(&q.why).small().color(t.muted));
                }
            });
            ui.add_space(4.0);
        }
        ui.horizontal(|ui| {
            if !drill.submitted {
                if ui.button("Submit").clicked() {
                    drill.submitted = true;
                    let right = drill
                        .questions
                        .iter()
                        .zip(&drill.answers)
                        .filter(|((l, qi), a)| {
                            self.catalog.lesson(l).and_then(|l| l.quiz.get(*qi)).is_some_and(|q| **a == Some(q.answer))
                        })
                        .count();
                    self.toast(
                        format!("Drill: {right}/{}", drill.questions.len()),
                        right * 10 >= drill.questions.len() * 7,
                    );
                }
            } else if ui.button("New drill").clicked() {
                close = true;
                self.start_drill();
                return;
            }
            if ui.button("Close").clicked() {
                close = true;
            }
        });
        if !close {
            self.exam.drill = Some(drill);
        } else if self.exam.drill.is_none() {
            self.exam.drill = None;
        }
    }

    fn exam_env(&self, e: &Exam) -> Option<poddojo_runner::LabEnv> {
        self.env_for(&format!("exam-{}", e.id))
    }

    fn start_exam(&mut self, ctx: &egui::Context, e: &Exam) {
        let Some(env) = self.exam_env(e) else { return };
        let exam = e.clone();
        self.exam.log.clear();
        self.jobs.spawn(ctx, "exam start", move |_| {
            if let Err(err) = cluster::guard(&env) {
                return Done::ExamStarted { exam: exam.id.clone(), ok: false, log: err };
            }
            let mut log = match lab::provide(&exam.requires, &env) {
                Ok(log) => log,
                Err(err) => return Done::ExamStarted { exam: exam.id.clone(), ok: false, log: err },
            };
            let _ = lab::run_all(&exam.cleanup, &env, Duration::from_secs(300));
            let setup: Vec<String> =
                exam.setup.iter().chain(exam.tasks.iter().flat_map(|t| &t.setup)).cloned().collect();
            let (ok, more) = lab::run_all(&setup, &env, Duration::from_secs(600));
            log.push_str(&more);
            Done::ExamStarted { exam: exam.id.clone(), ok, log }
        });
    }

    fn grade_exam(&mut self, ctx: &egui::Context, e: &Exam, started_at: i64) {
        let Some(env) = self.exam_env(e) else { return };
        let exam = e.clone();
        self.jobs.spawn(ctx, "exam grade", move |_| {
            let results = exam.tasks.iter().map(|t| (t.id.clone(), lab::run_checks(&t.checks, &env))).collect();
            Done::ExamGraded(exam::grade(&exam, &results, started_at, now()))
        });
    }

    fn exam_workspace(&mut self, ui: &mut egui::Ui, t: &Tokens, ctx: &egui::Context, e: &Exam, started_at: i64) {
        let total_secs = e.minutes as i64 * 60;
        let left = total_secs - (now() - started_at);
        ctx.request_repaint_after(Duration::from_secs(1));
        // Like the real exam, time is up at 0:00: grade what is there.
        if left <= 0 && !self.exam.auto_graded && !self.jobs.busy("exam grade") {
            self.exam.auto_graded = true;
            self.toast("Time is up: grading your work now", false);
            self.grade_exam(ctx, e, started_at);
        }
        // Remember the task on screen so a restart resumes there.
        if let Some(a) = &mut self.progress.active_exam
            && a.current_task != self.exam.task
        {
            a.current_task = self.exam.task;
            self.save_progress();
        }
        let flagged = self.progress.active_exam.as_ref().map(|a| a.flagged.clone()).unwrap_or_default();
        // Timer bar.
        theme::card(t).inner_margin(egui::Margin::symmetric(18, 12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Stopwatch, 26.0, if left < 15 * 60 { t.amber } else { t.accent });
                let (mm, ss) = (left.abs() / 60, left.abs() % 60);
                let text = if left >= 0 { format!("{mm}:{ss:02}") } else { format!("-{mm}:{ss:02}") };
                let color = if left < 0 {
                    t.bad
                } else if left < 15 * 60 {
                    t.amber
                } else {
                    strong(t)
                };
                ui.label(RichText::new(text).size(28.0).strong().monospace().color(color));
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&e.title).size(16.0).strong());
                    ui.label(
                        RichText::new(format!(
                            "Task {} of {} · {} flagged · pass {}%",
                            self.exam.task + 1,
                            e.tasks.len(),
                            flagged.len(),
                            e.pass_percent
                        ))
                        .size(12.5)
                        .color(t.muted),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let grading = self.jobs.busy("exam grade");
                    if ui.add_enabled(!grading, egui::Button::new(RichText::new("Finish and grade").strong())).clicked()
                    {
                        self.exam.confirm_finish = true;
                    }
                    if grading {
                        ui.spinner();
                        ui.label("grading…");
                    }
                });
            });
            let used = (1.0 - left as f32 / total_secs.max(1) as f32).clamp(0.0, 1.0);
            let w = ui.available_width();
            theme::bar(ui, t, used, w, if left < 15 * 60 { t.amber } else { t.accent });
        });
        ui.add_space(10.0);
        let total = ui.available_rect_before_wrap();
        let drawer_was_open = self.exam.drawer;
        let list_w = 230.0;
        // Narrow windows fold the task list into a drawer so the task and the terminal keep room.
        let drawer = ctx.content_rect().width() < DRAWER_BELOW
            || total.width() < list_w + DIVIDER + INSTRUCTIONS_MIN + DIVIDER + TERMINAL_MIN;
        if !drawer {
            self.exam.drawer = false;
        }
        let work = if drawer {
            total
        } else {
            egui::Rect::from_min_max(egui::pos2(total.min.x + list_w + DIVIDER, total.min.y), total.max)
        };
        let mut split = self.settings.exam_split;
        let default = (work.width() * 0.5).max(INSTRUCTIONS_MIN);
        let (task_rect, term, save) =
            super::resizable_split(ui, t, "exam", work, &mut split, default, (INSTRUCTIONS_MIN, TERMINAL_MIN));
        self.settings.exam_split = split;
        if save {
            self.save_settings();
        }
        // Task list.
        if !drawer {
            let list = egui::Rect::from_min_size(total.min, egui::vec2(list_w, total.height()));
            ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
                ui.set_clip_rect(list.intersect(ui.clip_rect()));
                self.task_list(ui, t, e, &flagged);
            });
        }
        // Task.
        ui.scope_builder(egui::UiBuilder::new().max_rect(task_rect), |ui| {
            ui.set_clip_rect(task_rect.intersect(ui.clip_rect()));
            let Some(task) = e.tasks.get(self.exam.task).cloned() else { return };
            egui::ScrollArea::vertical().id_salt(format!("exam-task-{}", task.id)).auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width((task_rect.width() - 12.0).max(0.0));
                if drawer {
                    let label = format!("All tasks · {} of {}", self.exam.task + 1, e.tasks.len());
                    if icons::button(ui, Icon::Book, &label).on_hover_text("Show the task list").clicked() {
                        self.exam.drawer = !self.exam.drawer;
                    }
                    ui.add_space(8.0);
                }
                theme::card(t).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        theme::chip(ui, &format!("{} points", task.weight), t.accent, t.accent_soft);
                        theme::chip(ui, domain_title(&task.domain), t.muted, t.card_hover);
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new(&task.title).size(20.0).strong().color(strong(t)));
                    if let Some(ns) = &task.namespace {
                        egui::Frame::new()
                            .fill(t.amber.gamma_multiply(0.12))
                            .corner_radius(8)
                            .inner_margin(egui::Margin::symmetric(10, 6))
                            .show(ui, |ui| {
                                ui.label(RichText::new(format!("Namespace: {ns}")).monospace().color(t.amber));
                            });
                    }
                    ui.add_space(4.0);
                    if let Some(Action::Run(cmd)) = markdown::show(ui, t, &task.task, &format!("exam-{}", task.id)) {
                        self.run_in_terminal(&cmd);
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let is_flagged = flagged.contains(&task.id);
                    if icons::button(ui, Icon::Flag, if is_flagged { "Unflag" } else { "Flag for review" }).clicked()
                        && let Some(a) = &mut self.progress.active_exam
                    {
                        if is_flagged {
                            a.flagged.remove(&task.id);
                        } else {
                            a.flagged.insert(task.id.clone());
                        }
                        self.save_progress();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.exam.task + 1 < e.tasks.len() && accent_button(ui, t, "Next task").clicked() {
                            self.exam.task += 1;
                        }
                        if self.exam.task > 0 && ui.button("Previous").clicked() {
                            self.exam.task -= 1;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Like the real exam: no hints and no grading until you finish. kubernetes.io/docs is allowed.")
                        .size(12.0)
                        .color(t.muted),
                );
            });
        });
        // Terminal.
        ui.scope_builder(egui::UiBuilder::new().max_rect(term), |ui| {
            let chips = self.terminal_chips();
            self.terminal(ui, &chips);
        });
        ui.allocate_rect(total, egui::Sense::hover());
        if drawer {
            self.task_drawer(ctx, t, e, &flagged, total, drawer_was_open);
        }
        if self.exam.confirm_finish {
            let mut open = true;
            egui::Window::new("Finish the exam?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label("Every task is graded now on the lab cluster. You can't change answers afterwards.");
                    if !flagged.is_empty() {
                        ui.label(RichText::new(format!("{} task(s) still flagged.", flagged.len())).color(t.amber));
                    }
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Grade now").strong()).clicked() {
                            self.grade_exam(ctx, e, started_at);
                            open = false;
                        }
                        if ui.button("Keep working").clicked() {
                            open = false;
                        }
                        if ui.button(RichText::new("Abandon (no score)").color(t.bad)).clicked() {
                            self.progress.active_exam = None;
                            self.save_progress();
                            open = false;
                        }
                    });
                });
            self.exam.confirm_finish = open;
        }
    }

    fn task_list(&mut self, ui: &mut egui::Ui, t: &Tokens, e: &Exam, flagged: &BTreeSet<String>) -> bool {
        let mut picked = false;
        egui::ScrollArea::vertical().id_salt("exam-tasks").auto_shrink([false, false]).show(ui, |ui| {
            for (i, task) in e.tasks.iter().enumerate() {
                if task_row(ui, t, i + 1, &task.title, task.weight, self.exam.task == i, flagged.contains(&task.id))
                    .clicked()
                {
                    self.exam.task = i;
                    if let Some(a) = &mut self.progress.active_exam {
                        a.current_task = i;
                    }
                    picked = true;
                }
            }
        });
        picked
    }

    /// The task list sliding in over the workspace; picking a task or clicking outside closes it.
    fn task_drawer(
        &mut self,
        ctx: &egui::Context,
        t: &Tokens,
        e: &Exam,
        flagged: &BTreeSet<String>,
        area: egui::Rect,
        was_open: bool,
    ) {
        let open = ctx.animate_bool_with_time(egui::Id::new("exam-drawer"), self.exam.drawer, theme::motion(ctx, 0.18));
        if open <= 0.0 {
            return;
        }
        let width = 280.0_f32.min(area.width());
        ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("exam-drawer-scrim"))).rect_filled(
            area,
            12.0,
            Color32::from_black_alpha((90.0 * open) as u8),
        );
        let pos = egui::pos2(area.min.x - (1.0 - open) * (width + 20.0), area.min.y);
        let inner = egui::Area::new(egui::Id::new("exam-drawer-area"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                theme::card(t).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                    ui.set_width(width - 20.0);
                    ui.set_height(area.height() - 20.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Tasks").strong().color(strong(t)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Close").clicked() {
                                self.exam.drawer = false;
                            }
                        });
                    });
                    ui.add_space(6.0);
                    if self.task_list(ui, t, e, flagged) {
                        self.exam.drawer = false;
                    }
                });
            });
        let rect = inner.response.rect;
        let clicked_outside = ctx.input(|i| {
            i.pointer.any_click() && i.pointer.interact_pos().is_some_and(|p| area.contains(p) && !rect.contains(p))
        });
        // `was_open`: the click that opened it this frame does not count as outside.
        if was_open && clicked_outside {
            self.exam.drawer = false;
        }
    }

    fn exam_result(&mut self, ui: &mut egui::Ui, t: &Tokens, a: &ExamAttempt) {
        let exam = self.catalog.exam(&a.exam_id).cloned();
        theme::card(t).stroke(egui::Stroke::new(1.5, if a.passed { t.good } else { t.amber })).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{:.1}%", a.percent)).size(40.0).strong().color(if a.passed {
                    t.good
                } else {
                    t.amber
                }));
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(if a.passed { "PASS 🥋" } else { "Not yet: keep training" }).size(20.0).strong(),
                    );
                    ui.label(RichText::new(format!("{} · {} min used", a.title, a.minutes_used())).color(t.muted));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        self.exam.result = None;
                    }
                });
            });
            ui.add_space(6.0);
            let w = (ui.available_width() * 0.5).max(160.0);
            for (d, s) in &a.domains {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{:<48}", domain_title(d))).size(13.0));
                    theme::bar(ui, t, s.percent() / 100.0, w, if s.percent() >= 66.0 { t.good } else { t.amber });
                    ui.label(RichText::new(format!("{:.0}%", s.percent())).size(13.0));
                });
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Task by task").strong());
            for r in &a.tasks {
                let task = exam.as_ref().and_then(|e| e.tasks.iter().find(|x| x.id == r.id));
                let full = r.earned + 0.01 >= r.possible;
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if full {
                            "✔"
                        } else if r.earned > 0.0 {
                            "~"
                        } else {
                            "✖"
                        })
                        .color(if full {
                            t.good
                        } else if r.earned > 0.0 {
                            t.amber
                        } else {
                            t.bad
                        }),
                    );
                    ui.label(format!("{} · {}", r.id, task.map_or("", |x| x.title.as_str())));
                    ui.label(RichText::new(format!("{:.1}/{:.0}", r.earned, r.possible)).color(t.muted));
                    if task.is_some_and(|x| !x.solution.is_empty()) {
                        let open = self.exam.reveal.contains(&r.id);
                        if ui.small_button(if open { "hide solution" } else { "solution" }).clicked() {
                            if open {
                                self.exam.reveal.remove(&r.id);
                            } else {
                                self.exam.reveal.insert(r.id.clone());
                            }
                        }
                    }
                });
                for c in r.checks.iter().filter(|c| !c.passed) {
                    ui.label(RichText::new(format!("      ✖ {}: {}", c.name, c.detail)).small().color(t.bad));
                }
                if self.exam.reveal.contains(&r.id)
                    && let Some(task) = task
                {
                    egui::Frame::new().fill(t.card_hover).corner_radius(8).inner_margin(10).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        markdown::show(ui, t, &task.solution, &format!("res-{}", r.id));
                    });
                }
            }
        });
    }
}

fn strong(t: &Tokens) -> Color32 {
    if t.dark { Color32::WHITE } else { Color32::BLACK }
}

/// A task in the exam's task list: number, title, weight and a flag mark.
fn task_row(
    ui: &mut egui::Ui,
    t: &Tokens,
    n: usize,
    title: &str,
    weight: u32,
    selected: bool,
    flagged: bool,
) -> egui::Response {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 38.0), egui::Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 9.0, t.accent_soft);
    } else if resp.hovered() {
        p.rect_filled(rect, 9.0, t.card);
    }
    let badge = egui::Rect::from_center_size(egui::pos2(rect.min.x + 18.0, rect.center().y), egui::vec2(24.0, 24.0));
    p.rect_filled(badge, 7.0, if selected { t.accent } else { t.card_hover });
    p.text(
        badge.center(),
        egui::Align2::CENTER_CENTER,
        n.to_string(),
        egui::FontId::proportional(12.0),
        if selected { t.bg } else { t.muted },
    );
    let text_w = width - if flagged { 104.0 } else { 84.0 };
    let mut job = egui::text::LayoutJob::single_section(
        title.to_string(),
        egui::TextFormat {
            font_id: egui::FontId::proportional(13.5),
            color: if selected { t.accent } else { t.text },
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width: text_w,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    p.galley(egui::pos2(rect.min.x + 38.0, rect.center().y - galley.size().y / 2.0), galley, t.text);
    p.text(
        egui::pos2(rect.max.x - 10.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        format!("{weight}%"),
        egui::FontId::proportional(12.0),
        t.muted,
    );
    if flagged {
        icons::paint(
            p,
            egui::Rect::from_center_size(egui::pos2(rect.max.x - 46.0, rect.center().y), egui::vec2(14.0, 14.0)),
            Icon::Flag,
            t.amber,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn short_domain(id: &str) -> &'static str {
    match id {
        "design-build" => "Design & Build",
        "deployment" => "Deployment",
        "observability" => "Observability",
        "config-security" => "Config & Security",
        "networking" => "Networking",
        _ => "Other",
    }
}
