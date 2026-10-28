//! Review: spaced repetition of quiz questions and drills of the mock-exam tasks the learner
//! missed, so weak spots come back until they stick.

use std::time::Duration;

use egui::{Margin, RichText};
use poddojo_core::review::{self, Item};
use poddojo_core::{CheckResult, now};
use poddojo_runner::lab;

use super::{INSTRUCTIONS_MIN, TERMINAL_MIN, accent_button, ago, domain_title, title};
use crate::icons::{self, Icon};
use crate::jobs::Done;
use crate::markdown::{self, Action};
use crate::theme::{self, ChoiceState, Tokens};
use crate::{App, Page};

/// A weak spot: what was missed, the lesson it comes from (id, title) and how often it lapsed.
type WeakSpot = (String, Option<(String, String)>, u32);

/// Questions per review session.
const SESSION: usize = 25;

/// A run through review cards, one question at a time.
#[derive(Default)]
pub struct CardSession {
    pub keys: Vec<String>,
    pub pos: usize,
    /// The choice picked for the current card (revealed at once).
    pub picked: Option<usize>,
    pub right: usize,
    /// Practice outside the schedule: cards not yet due only record misses.
    pub practice: bool,
}

/// One mock-exam task, set up fresh and timed, graded on demand.
#[derive(Default)]
pub struct TaskDrill {
    pub key: String,
    pub exam: String,
    pub task: String,
    /// Set when the setup finished; the clock starts then.
    pub started_at: Option<i64>,
    pub results: Option<Vec<CheckResult>>,
    pub solution: bool,
    /// The outcome went into the review deck already.
    pub recorded: bool,
    pub log: String,
}

impl TaskDrill {
    pub fn ready(&self) -> bool {
        self.started_at.is_some()
    }
}

#[derive(Default)]
pub struct ReviewState {
    pub session: Option<CardSession>,
    pub drill: Option<TaskDrill>,
}

/// The suggested time for a task: the exam's pace (120 minutes for 100 points), at least 4 minutes.
fn target_minutes(weight: u32) -> i64 {
    ((weight as f32 * 1.2).round() as i64).max(4)
}

impl App {
    pub fn review_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        if self.review.drill.as_ref().is_some_and(TaskDrill::ready) {
            self.drill_workspace(ui, &t);
            return;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let width = ui.available_width().min(1100.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(width);
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    if self.review.session.is_some() {
                        self.card_session(ui, &t);
                    } else {
                        self.review_home(ui, &t);
                    }
                });
            });
        });
    }

    fn review_home(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        title(
            ui,
            t,
            "Review",
            "Spaced repetition: what you miss comes back today, what you know comes back later (1, 3, 7, 16, 35 days).",
        );
        let at = now();
        let stats = self.progress.deck.stats(&self.catalog, at);
        if let Some(d) = &self.review.drill {
            theme::card(t).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("Setting up a fresh copy of task {} …", d.task));
                });
                if !d.log.is_empty() {
                    ui.label(RichText::new(d.log.trim_end()).monospace().size(11.5).color(t.muted));
                }
            });
            ui.add_space(10.0);
        }
        // Summary tiles.
        ui.columns(4, |cols| {
            tile(&mut cols[0], t, &stats.due_questions.to_string(), "questions due", t.accent);
            tile(&mut cols[1], t, &stats.due_tasks.to_string(), "task drills due", t.amber);
            tile(&mut cols[2], t, &stats.learned.to_string(), "learned (7+ days)", t.good);
            tile(&mut cols[3], t, &stats.due_tomorrow.to_string(), "due in the next 24 h", t.muted);
        });
        ui.add_space(12.0);
        if self.progress.deck.cards.is_empty() {
            theme::card(t).inner_margin(Margin::same(20)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Your review deck is empty").size(18.0).strong());
                ui.label(
                    RichText::new(
                        "Every quiz question you answer and every mock-exam task you attempt joins it. Take a lesson \
                         quiz to start.",
                    )
                    .color(t.muted),
                );
                ui.add_space(6.0);
                if accent_button(ui, t, "Continue learning").clicked() {
                    self.page = Page::Learn;
                }
            });
            return;
        }
        // Questions.
        theme::card(t).inner_margin(Margin::same(20)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Repeat, 22.0, t.accent);
                ui.label(RichText::new("Questions").size(18.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if stats.due_questions > 0 {
                        let n = stats.due_questions.min(SESSION);
                        if accent_button(ui, t, &format!("Review {n} now")).clicked() {
                            self.start_session(false);
                        }
                    } else if ui.button("Practise weak spots").clicked() {
                        self.start_session(true);
                    }
                });
            });
            ui.label(
                RichText::new(if stats.due_questions > 0 {
                    "One question at a time, with the explanation right after you answer. Keys: 1–4 to answer, Enter for the next."
                } else {
                    "Nothing due: you are up to date. Practising early only records misses, so it never skips a review."
                })
                .color(t.muted),
            );
        });
        ui.add_space(10.0);
        // Task drills.
        let due: Vec<(String, String, String)> = self
            .progress
            .deck
            .due(&self.catalog, at)
            .into_iter()
            .filter_map(|(key, item)| match item {
                Item::Task { exam, task } => Some((key, exam.id.clone(), task.id.clone())),
                Item::Question { .. } => None,
            })
            .collect();
        theme::card(t).inner_margin(Margin::same(20)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Stopwatch, 22.0, t.amber);
                ui.label(RichText::new("Task drills").size(18.0).strong());
            });
            ui.label(
                RichText::new(
                    "Mock-exam tasks you did not fully solve, set up fresh on the lab cluster, one at a time and timed \
                     at exam pace. Solve it without the solution and it moves up; peek and it comes back tomorrow.",
                )
                .color(t.muted),
            );
            ui.add_space(6.0);
            if due.is_empty() {
                ui.label(RichText::new("No task drills due. Missed mock-exam tasks will show up here.").color(t.muted));
            }
            let can_run = self.cluster_running() && self.review.drill.is_none() && self.progress.active_exam.is_none();
            for (key, exam_id, task_id) in due {
                let Some(exam) = self.catalog.exam(&exam_id).cloned() else { continue };
                let Some(task) = exam.tasks.iter().find(|x| x.id == task_id).cloned() else { continue };
                let card = self.progress.deck.cards.get(&key).copied().unwrap_or_default();
                egui::Frame::new()
                    .fill(t.card_hover.gamma_multiply(0.6))
                    .corner_radius(10)
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&task.title).size(15.0).strong());
                                let last = if card.last > 0 {
                                    format!(" · last tried {}", ago(card.last))
                                } else {
                                    String::new()
                                };
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {} · {} points · ~{} min{last}",
                                        exam.title,
                                        domain_title(&task.domain),
                                        task.weight,
                                        target_minutes(task.weight)
                                    ))
                                    .size(12.5)
                                    .color(t.muted),
                                );
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let resp = ui.add_enabled(can_run, egui::Button::new("Drill it"));
                                let resp = if self.cluster_running() {
                                    resp
                                } else {
                                    resp.on_disabled_hover_text("Start the lab cluster first (Lab cluster page)")
                                };
                                if resp.clicked() {
                                    self.start_task_drill(ui.ctx(), &key, &exam_id, &task_id);
                                }
                            });
                        });
                    });
                ui.add_space(6.0);
            }
        });
        ui.add_space(10.0);
        // Weak spots.
        let weak: Vec<WeakSpot> = self
            .progress
            .deck
            .weakest(&self.catalog, 8)
            .into_iter()
            .map(|(item, card)| match item {
                Item::Question { lesson, question, .. } => {
                    (question.q.clone(), Some((lesson.id.clone(), lesson.title.clone())), card.lapses)
                }
                Item::Task { task, .. } => (task.title.clone(), None, card.lapses),
            })
            .collect();
        if !weak.is_empty() {
            theme::card(t).inner_margin(Margin::same(20)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Your weak spots").size(18.0).strong());
                ui.label(
                    RichText::new("Missed most often. Reread the lesson section if one keeps coming back.")
                        .color(t.muted),
                );
                ui.add_space(6.0);
                let mut open = None;
                for (text, lesson, lapses) in weak {
                    ui.horizontal_top(|ui| {
                        theme::chip(ui, &format!("missed {}×", lapses.max(1)), t.amber, t.amber.gamma_multiply(0.12));
                        ui.vertical(|ui| {
                            let w = ui.available_width();
                            ui.add(egui::Label::new(crate::markdown::inline_job(&text, 13.5, t.text, t.card_hover, w)));
                            if let Some((id, title)) = lesson
                                && ui
                                    .add(
                                        egui::Label::new(
                                            RichText::new(format!("{title} ›")).size(12.0).color(t.accent),
                                        )
                                        .sense(egui::Sense::click()),
                                    )
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                            {
                                open = Some(id);
                            }
                        });
                    });
                    ui.add_space(6.0);
                }
                if let Some(id) = open {
                    self.page = Page::Learn;
                    self.open_lesson(&id);
                }
            });
        }
    }

    /// Start a run through the due questions (or, `practice`, the weakest and the not-yet-due).
    pub fn start_session(&mut self, practice: bool) {
        let at = now();
        let mut keys: Vec<String> = self
            .progress
            .deck
            .due(&self.catalog, at)
            .into_iter()
            .filter(|(_, item)| matches!(item, Item::Question { .. }))
            .map(|(k, _)| k)
            .collect();
        if practice {
            let mut rest: Vec<(&String, &poddojo_core::Card)> =
                self.progress.deck.cards.iter().filter(|(k, _)| k.starts_with("q:")).collect();
            rest.sort_by_key(|(_, c)| (c.level, std::cmp::Reverse(c.lapses), c.due));
            keys.extend(rest.into_iter().map(|(k, _)| k.clone()));
            keys.dedup();
        }
        keys.retain(|k| review::resolve(&self.catalog, k).is_some());
        keys.truncate(SESSION);
        if keys.is_empty() {
            self.toast("Nothing to review yet", false);
            return;
        }
        self.review.session = Some(CardSession { keys, practice, ..Default::default() });
    }

    fn card_session(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        let Some(mut s) = self.review.session.take() else { return };
        let total = s.keys.len();
        let mut close = false;
        if s.pos >= total {
            title(ui, t, "Review done", "");
            theme::card(t).inner_margin(Margin::same(24)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let pct = if total == 0 { 0.0 } else { s.right as f32 / total as f32 };
                ui.horizontal(|ui| {
                    theme::ring(
                        ui,
                        t,
                        pct,
                        96.0,
                        &format!("{}/{total}", s.right),
                        "right",
                        if pct >= 0.7 { t.good } else { t.amber },
                    );
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Session complete").size(20.0).strong());
                        ui.label(
                            RichText::new("Misses come back in about 10 minutes; the rest at their next interval.")
                                .color(t.muted),
                        );
                        ui.add_space(8.0);
                        if accent_button(ui, t, "Back to Review").clicked() {
                            close = true;
                        }
                    });
                });
            });
            if !close {
                self.review.session = Some(s);
            }
            return;
        }
        let key = s.keys[s.pos].clone();
        let Some(Item::Question { lesson, question, .. }) = review::resolve(&self.catalog, &key) else {
            s.pos += 1;
            self.review.session = Some(s);
            return;
        };
        let (lesson, question) = (lesson.clone(), question.clone());
        // Header with progress.
        ui.horizontal(|ui| {
            if icons::button(ui, Icon::Repeat, "End session").clicked() {
                close = true;
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("{} of {total}{}", s.pos + 1, if s.practice { " · practice" } else { "" }))
                    .color(t.muted),
            );
        });
        ui.add_space(4.0);
        let w = ui.available_width();
        theme::bar(ui, t, s.pos as f32 / total.max(1) as f32, w, t.accent);
        ui.add_space(12.0);
        let order = question.order(&lesson.id);
        let mut pick = None;
        theme::card(t).inner_margin(Margin::same(24)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(lesson.title.to_uppercase()).size(11.5).strong().color(t.muted));
            ui.add_space(2.0);
            let w = ui.available_width();
            ui.label(crate::markdown::inline_job(&question.q, 19.0, super::strong_text(t), t.card_hover, w));
            ui.add_space(12.0);
            for (pos, j) in order.iter().copied().enumerate() {
                let Some(choice) = question.choices.get(j) else { continue };
                let state = match (s.picked, j == question.answer) {
                    (None, _) => ChoiceState::Idle,
                    (Some(p), true) if p == j => ChoiceState::Correct,
                    (Some(_), true) => ChoiceState::Missed,
                    (Some(p), false) if p == j => ChoiceState::Wrong,
                    (Some(_), false) => ChoiceState::Idle,
                };
                if theme::choice(ui, t, (b'A' + pos as u8) as char, choice, state, s.picked.is_none()).clicked() {
                    pick = Some(j);
                }
                ui.add_space(5.0);
            }
            if let Some(p) = s.picked {
                let right = p == question.answer;
                ui.add_space(6.0);
                egui::Frame::new()
                    .fill(if right { t.good } else { t.amber }.gamma_multiply(0.1))
                    .corner_radius(8)
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(if right { "Correct" } else { "Not quite: this comes back in 10 minutes" })
                                .strong()
                                .color(if right { t.good } else { t.amber }),
                        );
                        ui.label(RichText::new(&question.why).color(t.text));
                    });
            }
        });
        // Keyboard: 1–4 / A–D answer, Enter moves on.
        let typing = ui.memory(|m| m.focused().is_some());
        if s.picked.is_none() && !typing {
            let keys = [
                (egui::Key::Num1, egui::Key::A),
                (egui::Key::Num2, egui::Key::B),
                (egui::Key::Num3, egui::Key::C),
                (egui::Key::Num4, egui::Key::D),
                (egui::Key::Num5, egui::Key::E),
            ];
            for (pos, (n, l)) in keys.iter().enumerate() {
                if ui.input(|i| i.key_pressed(*n) || i.key_pressed(*l))
                    && let Some(j) = order.get(pos)
                {
                    pick = Some(*j);
                }
            }
        }
        if let Some(j) = pick
            && s.picked.is_none()
        {
            s.picked = Some(j);
            let right = j == question.answer;
            if right {
                s.right += 1;
            }
            let due = self.progress.deck.cards.get(&key).is_none_or(|c| c.is_due(now()));
            if !s.practice || due || !right {
                self.progress.record_review(&key, right, now());
                self.save_progress();
            }
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if s.picked.is_some() {
                let next = accent_button(ui, t, if s.pos + 1 < total { "Next question" } else { "Finish" }).clicked()
                    || (!typing && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                if next {
                    s.pos += 1;
                    s.picked = None;
                }
            }
            ui.label(
                RichText::new("Keys: 1–4 or A–D to answer · Enter for the next question").size(12.0).color(t.muted),
            );
            ui.add_space(8.0);
            if ui.button(format!("Open lesson: {}", lesson.title)).clicked() {
                close = true;
                self.page = Page::Learn;
                self.open_lesson(&lesson.id);
            }
        });
        if !close {
            self.review.session = Some(s);
        }
    }

    fn start_task_drill(&mut self, ctx: &egui::Context, key: &str, exam_id: &str, task_id: &str) {
        let Some(exam) = self.catalog.exam(exam_id).cloned() else { return };
        let Some(env) = self.env_for(&format!("exam-{exam_id}")) else {
            self.toast("Choose a machine on the Setup page first", false);
            return;
        };
        self.review.drill = Some(TaskDrill {
            key: key.to_string(),
            exam: exam_id.to_string(),
            task: task_id.to_string(),
            ..Default::default()
        });
        let (key, task) = (key.to_string(), task_id.to_string());
        self.jobs.spawn(ctx, "drill prepare", move |_| {
            let (ok, log) = lab::prepare_task(&exam, &task, &env);
            Done::DrillReady { key, ok, log }
        });
    }

    pub fn drill_ready(&mut self, key: String, ok: bool, log: String) {
        let Some(d) = self.review.drill.as_mut().filter(|d| d.key == key) else { return };
        d.log = log;
        if ok {
            d.started_at = Some(now());
            self.page = Page::Review;
            self.toast("Task ready: the clock is running", true);
        } else {
            let log = d.log.clone();
            self.review.drill = None;
            self.toast("Drill setup failed", false);
            self.exam.log = log;
        }
    }

    pub fn drill_checked(&mut self, key: String, results: Vec<CheckResult>) {
        let Some(d) = self.review.drill.as_mut().filter(|d| d.key == key) else { return };
        let solved = !results.is_empty() && results.iter().all(|r| r.passed);
        d.results = Some(results);
        if solved && !d.recorded {
            d.recorded = true;
            let clean = !d.solution;
            self.progress.record_review(&key, clean, now());
            self.save_progress();
            self.toast(
                if clean { "Solved: this task moves up a box" } else { "Solved with the solution: it comes back soon" },
                clean,
            );
        } else if !solved {
            self.toast("Not yet: see which checks fail", false);
        }
    }

    fn drill_workspace(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        let ctx = ui.ctx().clone();
        let Some(d) = self.review.drill.as_ref() else { return };
        let Some(exam) = self.catalog.exam(&d.exam).cloned() else {
            self.review.drill = None;
            return;
        };
        let Some(task) = exam.tasks.iter().find(|x| x.id == d.task).cloned() else {
            self.review.drill = None;
            return;
        };
        let (key, started, results, solution, recorded) =
            (d.key.clone(), d.started_at.unwrap_or_else(now), d.results.clone(), d.solution, d.recorded);
        ctx.request_repaint_after(Duration::from_secs(1));
        let target = target_minutes(task.weight) * 60;
        let used = now() - started;
        let checking = self.jobs.busy("drill check");
        // Timer bar.
        let mut end = false;
        theme::card(t).inner_margin(Margin::symmetric(18, 12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let over = used > target;
                icons::show(ui, Icon::Stopwatch, 26.0, if over { t.amber } else { t.accent });
                ui.label(
                    RichText::new(format!("{}:{:02}", used / 60, used % 60))
                        .size(28.0)
                        .strong()
                        .monospace()
                        .color(if over { t.amber } else { super::strong_text(t) }),
                );
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("Drill · {}", exam.title)).size(16.0).strong());
                    ui.label(
                        RichText::new(format!(
                            "Exam pace for {} points: {} min{}",
                            task.weight,
                            target / 60,
                            if over { " · over time, keep going" } else { "" }
                        ))
                        .size(12.5)
                        .color(t.muted),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("End drill").clicked() {
                        end = true;
                    }
                    if ui.add_enabled(!checking, egui::Button::new(RichText::new("Check my work").strong())).clicked() {
                        self.check_drill(&ctx, &key, &exam.id, &task.id);
                    }
                    if checking {
                        ui.spinner();
                    }
                });
            });
            let w = ui.available_width();
            theme::bar(
                ui,
                t,
                (used as f32 / target.max(1) as f32).clamp(0.0, 1.0),
                w,
                if used > target { t.amber } else { t.accent },
            );
        });
        if end {
            // Ending without solving counts as a miss, so the task comes back soon.
            if !recorded {
                self.progress.record_review(&key, false, now());
                self.save_progress();
            }
            self.review.drill = None;
            return;
        }
        ui.add_space(10.0);
        let total = ui.available_rect_before_wrap();
        let mut split = self.settings.exam_split;
        let default = (total.width() * 0.5).max(INSTRUCTIONS_MIN);
        let (task_rect, term, save) =
            super::resizable_split(ui, t, "drill", total, &mut split, default, (INSTRUCTIONS_MIN, TERMINAL_MIN));
        self.settings.exam_split = split;
        if save {
            self.save_settings();
        }
        ui.scope_builder(egui::UiBuilder::new().max_rect(task_rect), |ui| {
            ui.set_clip_rect(task_rect.intersect(ui.clip_rect()));
            egui::ScrollArea::vertical().id_salt(format!("drill-{key}")).auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width((task_rect.width() - 12.0).max(0.0));
                theme::card(t).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        theme::chip(ui, &format!("{} points", task.weight), t.accent, t.accent_soft);
                        theme::chip(ui, domain_title(&task.domain), t.muted, t.card_hover);
                    });
                    ui.add_space(4.0);
                    ui.label(RichText::new(&task.title).size(20.0).strong().color(super::strong_text(t)));
                    if let Some(ns) = &task.namespace {
                        ui.label(RichText::new(format!("Namespace: {ns}")).monospace().color(t.amber));
                    }
                    ui.add_space(4.0);
                    if let Some(Action::Run(cmd)) = markdown::show(ui, t, &task.task, &format!("drill-{key}")) {
                        self.run_in_terminal(&cmd);
                    }
                });
                if let Some(results) = &results {
                    ui.add_space(8.0);
                    theme::card(t).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let passed = results.iter().filter(|r| r.passed).count();
                        ui.label(
                            RichText::new(format!("{passed}/{} checks pass", results.len()))
                                .strong()
                                .color(if passed == results.len() { t.good } else { t.amber }),
                        );
                        for r in results {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(if r.passed { "✔" } else { "✖" }).color(if r.passed {
                                    t.good
                                } else {
                                    t.bad
                                }));
                                ui.label(&r.name);
                                if !r.passed && !r.detail.is_empty() {
                                    ui.label(RichText::new(&r.detail).size(12.0).color(t.muted));
                                }
                            });
                        }
                    });
                }
                ui.add_space(8.0);
                if solution {
                    theme::card(t).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("WORKED SOLUTION").size(11.5).strong().color(t.amber));
                        if let Some(Action::Run(cmd)) =
                            markdown::show(ui, t, &task.solution, &format!("drill-sol-{key}"))
                        {
                            self.run_in_terminal(&cmd);
                        }
                    });
                } else if ui
                    .button("Show the worked solution")
                    .on_hover_text("Peeking is fine, but the task then comes back tomorrow")
                    .clicked()
                    && let Some(d) = self.review.drill.as_mut()
                {
                    d.solution = true;
                }
            });
        });
        ui.scope_builder(egui::UiBuilder::new().max_rect(term), |ui| {
            let chips = self.terminal_chips();
            self.terminal(ui, &chips);
        });
        ui.allocate_rect(total, egui::Sense::hover());
    }

    fn check_drill(&mut self, ctx: &egui::Context, key: &str, exam_id: &str, task_id: &str) {
        let Some(task) = self.catalog.exam(exam_id).and_then(|e| e.tasks.iter().find(|x| x.id == task_id)).cloned()
        else {
            return;
        };
        let Some(env) = self.env_for(&format!("exam-{exam_id}")) else { return };
        let key = key.to_string();
        self.jobs.spawn(ctx, "drill check", move |_| Done::DrillChecked {
            key,
            results: lab::run_checks(&task.checks, &env),
        });
    }
}

fn tile(ui: &mut egui::Ui, t: &Tokens, value: &str, label: &str, color: egui::Color32) {
    theme::card(t).inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(value).size(28.0).strong().color(color));
        ui.label(RichText::new(label).size(12.5).color(t.muted));
    });
}
