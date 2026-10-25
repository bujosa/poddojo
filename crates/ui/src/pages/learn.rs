use std::time::Instant;

use egui::{Color32, RichText};
use poddojo_core::{Lesson, LessonStatus, Needs, QuizScore, now};
use poddojo_runner::lab;

use super::{accent_button, domain_title, section};
use crate::jobs::Done;
use crate::markdown::{self, Action};
use crate::theme::{self, Tokens};
use crate::{App, LearnState, LessonTab, Page};

impl App {
    pub fn open_lesson(&mut self, id: &str) {
        let Some(l) = self.catalog.lesson(id).cloned() else { return };
        self.progress.open_lesson(&l, now());
        self.save_progress();
        let p = self.progress.lesson(id).cloned().unwrap_or_default();
        let tab = if !p.read {
            LessonTab::Concepts
        } else if !l.quiz.is_empty() && !p.quiz.is_some_and(|q| q.passed()) {
            LessonTab::Quiz
        } else if l.has_lab() {
            LessonTab::Lab
        } else {
            LessonTab::Concepts
        };
        self.learn = LearnState {
            lesson: Some(id.to_string()),
            tab,
            answers: vec![None; l.quiz.len()],
            opened_at: Some(Instant::now()),
            scroll_to_top: true,
            ..Default::default()
        };
        self.page = Page::Learn;
    }

    pub fn learn_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        let lesson = self.learn.lesson.as_ref().and_then(|id| self.catalog.lesson(id)).cloned();
        super::split_left(ui, 330.0, |ui, left| match (left, &lesson) {
            (true, _) => self.lesson_tree(ui, &t),
            (false, Some(l)) => self.lesson_view(ui, &t, l),
            (false, None) => {
                super::title(
                    ui,
                    &t,
                    "Pick a lesson",
                    "Docker first if containers are new to you, then the CKAD track domain by domain.",
                );
                if let Some(l) = self.progress.resume(&self.catalog).cloned()
                    && accent_button(ui, &t, &format!("▶  Continue: {}", l.title)).clicked()
                {
                    self.open_lesson(&l.id);
                }
            }
        });
    }

    fn lesson_tree(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        let mut open = None;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        egui::ScrollArea::vertical().id_salt("tree").auto_shrink([false, false]).show(ui, |ui| {
            for track in &self.catalog.tracks {
                let (d, n) = self.progress.track_counts(&self.catalog, &track.id);
                ui.add_space(6.0);
                ui.label(RichText::new(&track.title).strong().size(15.0));
                ui.label(RichText::new(format!("{d} of {n} done")).size(11.0).color(t.muted));
                for module in &track.modules {
                    let ids: Vec<&String> = track
                        .lessons
                        .iter()
                        .filter(|id| self.catalog.lesson(id).is_some_and(|l| l.module == module.id))
                        .collect();
                    if ids.is_empty() {
                        continue;
                    }
                    let contains_open = ids.iter().any(|id| self.learn.lesson.as_deref() == Some(id.as_str()));
                    egui::CollapsingHeader::new(RichText::new(&module.title).size(13.5).color(t.muted))
                        .id_salt(format!("{}-{}", track.id, module.id))
                        .default_open(
                            contains_open
                                || ids.iter().any(|id| {
                                    self.catalog
                                        .lesson(id)
                                        .is_some_and(|l| self.progress.status(l) == LessonStatus::InProgress)
                                }),
                        )
                        .show(ui, |ui| {
                            for id in ids {
                                let Some(l) = self.catalog.lesson(id) else { continue };
                                let status = self.progress.status(l);
                                let selected = self.learn.lesson.as_deref() == Some(id.as_str());
                                ui.horizontal(|ui| {
                                    theme::status_dot(ui, t, status);
                                    if ui.selectable_label(selected, RichText::new(&l.title).size(13.5)).clicked() {
                                        open = Some(id.clone());
                                    }
                                });
                            }
                        });
                }
            }
        });
        if let Some(id) = open {
            self.open_lesson(&id);
        }
    }

    fn lesson_view(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson) {
        // Header
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(self.catalog.module_title(l)).color(t.muted));
            ui.label(RichText::new(format!("· ~{} min", l.minutes)).color(t.muted));
            if let Some(d) = &l.domain {
                theme::chip(ui, &format!("CKAD · {}", domain_title(d)), t.accent, t.accent_soft);
            }
            match l.needs {
                Needs::Cluster => theme::chip(ui, "needs the lab cluster", t.amber, t.card_hover),
                Needs::Docker => theme::chip(ui, &format!("runs on {}", self.target_name()), t.muted, t.card_hover),
                Needs::None => {}
            }
        });
        ui.label(RichText::new(&l.title).size(28.0).strong().color(if t.dark {
            Color32::WHITE
        } else {
            Color32::BLACK
        }));
        ui.add_space(4.0);
        let p = self.progress.lesson(&l.id).cloned().unwrap_or_default();
        ui.horizontal(|ui| {
            let tab = |ui: &mut egui::Ui, cur: &mut LessonTab, me: LessonTab, label: String, done: bool| {
                let text =
                    RichText::new(format!("{}{label}", if done { "✔ " } else { "" })).size(15.0).color(if *cur == me {
                        t.accent
                    } else if done {
                        t.good
                    } else {
                        t.text
                    });
                if ui
                    .add(egui::Button::new(text).fill(if *cur == me { t.accent_soft } else { Color32::TRANSPARENT }))
                    .clicked()
                {
                    *cur = me;
                }
            };
            let mut cur = self.learn.tab;
            tab(ui, &mut cur, LessonTab::Concepts, "Concepts".into(), p.read);
            if !l.quiz.is_empty() {
                tab(
                    ui,
                    &mut cur,
                    LessonTab::Quiz,
                    format!("Quiz · {}", l.quiz.len()),
                    p.quiz.is_some_and(|q| q.passed()),
                );
            }
            if let Some(lab) = &l.lab
                && !lab.steps.is_empty()
            {
                tab(
                    ui,
                    &mut cur,
                    LessonTab::Lab,
                    format!("Lab · {}/{}", p.steps.len(), lab.steps.len()),
                    p.steps.len() == lab.steps.len(),
                );
            }
            self.learn.tab = cur;
        });
        ui.separator();
        let reset_scroll = std::mem::take(&mut self.learn.scroll_to_top);
        let mut area = egui::ScrollArea::vertical()
            .id_salt(format!("lesson-{}-{:?}", l.id, self.learn.tab))
            .auto_shrink([false, false]);
        if reset_scroll {
            area = area.vertical_scroll_offset(0.0);
        }
        area.show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(900.0));
            match self.learn.tab {
                LessonTab::Concepts => self.concepts(ui, t, l),
                LessonTab::Quiz => self.quiz(ui, t, l),
                LessonTab::Lab => self.lab(ui, t, l),
            }
            ui.add_space(16.0);
            self.lesson_nav(ui, t, l);
            ui.add_space(24.0);
        });
    }

    fn concepts(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson) {
        if !l.objectives.is_empty() {
            theme::card(t).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("You will be able to").strong().color(t.accent));
                for o in &l.objectives {
                    ui.label(format!("•  {o}"));
                }
            });
        }
        let ctx = ui.ctx().clone();
        if let Some(Action::Run(cmd)) = markdown::show(ui, t, &l.body, &l.id) {
            self.run_in_console(&ctx, &cmd);
        }
        ui.add_space(12.0);
        let read = self.progress.lesson(&l.id).is_some_and(|p| p.read);
        ui.horizontal(|ui| {
            if read {
                ui.label(RichText::new("✔ Read").color(t.good));
            } else if accent_button(ui, t, "✔  I've read this").clicked() {
                self.progress.mark_read(l, now());
                self.save_progress();
            }
            let next = if !l.quiz.is_empty() {
                Some((LessonTab::Quiz, "Take the quiz →"))
            } else if l.has_lab() {
                Some((LessonTab::Lab, "Start the lab →"))
            } else {
                None
            };
            if let Some((tab, label)) = next
                && ui.button(label).clicked()
            {
                if !read {
                    self.progress.mark_read(l, now());
                    self.save_progress();
                }
                self.learn.tab = tab;
                self.learn.scroll_to_top = true;
            }
        });
    }

    fn quiz(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson) {
        if self.learn.answers.len() != l.quiz.len() {
            self.learn.answers = vec![None; l.quiz.len()];
        }
        if let Some(best) = self.progress.lesson(&l.id).and_then(|p| p.quiz) {
            ui.label(
                RichText::new(format!(
                    "Best score so far: {}/{}{}",
                    best.correct,
                    best.total,
                    if best.passed() { " ✔" } else { "" }
                ))
                .color(if best.passed() { t.good } else { t.muted }),
            );
        }
        let submitted = self.learn.submitted;
        for (i, q) in l.quiz.iter().enumerate() {
            theme::card(t).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(format!("{}. {}", i + 1, q.q)).strong().size(15.5));
                ui.add_space(4.0);
                for (j, choice) in q.choices.iter().enumerate() {
                    let picked = self.learn.answers.get(i).copied().flatten() == Some(j);
                    let color = match (submitted, j == q.answer, picked) {
                        (true, true, _) => t.good,
                        (true, false, true) => t.bad,
                        _ => t.text,
                    };
                    let resp =
                        ui.add_enabled(!submitted, egui::RadioButton::new(picked, RichText::new(choice).color(color)));
                    if resp.clicked()
                        && let Some(a) = self.learn.answers.get_mut(i)
                    {
                        *a = Some(j);
                    }
                }
                if submitted {
                    ui.add_space(4.0);
                    let right = self.learn.answers.get(i).copied().flatten() == Some(q.answer);
                    ui.label(RichText::new(if right { "✔ Correct" } else { "✖ Not quite" }).strong().color(if right {
                        t.good
                    } else {
                        t.bad
                    }));
                    ui.label(RichText::new(&q.why).color(t.muted));
                }
            });
            ui.add_space(6.0);
        }
        ui.horizontal(|ui| {
            if !submitted {
                let all = self.learn.answers.iter().all(Option::is_some);
                if ui.add_enabled(all, egui::Button::new(RichText::new("Submit answers").strong())).clicked() {
                    let correct =
                        l.quiz.iter().zip(&self.learn.answers).filter(|(q, a)| **a == Some(q.answer)).count() as u32;
                    let score = QuizScore { correct, total: l.quiz.len() as u32 };
                    self.progress.record_quiz(l, score, now());
                    self.save_progress();
                    self.learn.submitted = true;
                    self.toast(
                        format!(
                            "Quiz: {correct}/{}{}",
                            l.quiz.len(),
                            if score.passed() { " · passed" } else { " · 70% needed" }
                        ),
                        score.passed(),
                    );
                }
                if !all {
                    ui.label(RichText::new("Answer every question to submit.").color(t.muted));
                }
            } else {
                if ui.button("Retake").clicked() {
                    self.learn.answers = vec![None; l.quiz.len()];
                    self.learn.submitted = false;
                    self.learn.scroll_to_top = true;
                }
                if l.has_lab() && accent_button(ui, t, "Start the lab →").clicked() {
                    self.learn.tab = LessonTab::Lab;
                    self.learn.scroll_to_top = true;
                }
            }
        });
    }

    fn lab(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson) {
        let Some(labdef) = l.lab.clone() else { return };
        let ctx = ui.ctx().clone();
        let env = self.env_for(&l.id);
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !labdef.summary.is_empty() {
                ui.label(&labdef.summary);
            }
            let where_ = match l.needs {
                Needs::Cluster => format!(
                    "Kubernetes commands go to the lab cluster (kind-poddojo); Docker commands to {}.",
                    self.target_name()
                ),
                _ => format!("Commands run on {} with DOCKER_CONTEXT set for you.", self.target_name()),
            };
            ui.label(RichText::new(where_).small().color(t.muted));
            if let Some(e) = &env {
                ui.label(RichText::new(format!("Lab folder: {}", e.workdir.display())).small().color(t.muted));
            }
            if l.needs == Needs::Cluster && !self.cluster_running() {
                ui.label(RichText::new("⚠ This lab needs the lab cluster, which is not running.").color(t.amber));
                if ui.button("Go to Cluster").clicked() {
                    self.page = Page::Cluster;
                }
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let busy = self.jobs.busy(&format!("lab {}", l.id));
                if ui
                    .add_enabled(!busy && env.is_some(), egui::Button::new("⚙ Prepare lab"))
                    .on_hover_text("Write the lab files and run its setup")
                    .clicked()
                    && let Some(env) = env.clone()
                {
                    let lesson = l.clone();
                    self.jobs.spawn(&ctx, format!("lab {} prepare", l.id), move |_| {
                        let (ok, log) = lab::prepare(&lesson, &env);
                        Done::Prepared { lesson: lesson.id.clone(), ok, log }
                    });
                }
                if ui.add_enabled(!busy && env.is_some(), egui::Button::new("✔ Check all steps")).clicked() {
                    for i in 0..labdef.steps.len() {
                        self.check_step(&ctx, l, i);
                    }
                }
                if ui.button("🖥 Open terminal").clicked() {
                    self.open_terminal();
                }
                if ui
                    .add_enabled(!busy && env.is_some(), egui::Button::new("↺ Reset lab"))
                    .on_hover_text("Remove what this lab created and start the steps over")
                    .clicked()
                    && let Some(env) = env.clone()
                {
                    let lesson = l.clone();
                    self.jobs.spawn(&ctx, format!("lab {} reset", l.id), move |_| Done::Reset {
                        lesson: lesson.id.clone(),
                        log: lab::reset(&lesson, &env),
                    });
                }
                if busy {
                    ui.spinner();
                }
            });
            if !self.learn.lab_log.is_empty() {
                egui::CollapsingHeader::new(RichText::new("Lab log").small()).id_salt("lablog").show(ui, |ui| {
                    ui.label(RichText::new(self.learn.lab_log.trim_end()).monospace().size(12.0));
                });
            }
        });
        ui.add_space(8.0);
        let passed = self.progress.lesson(&l.id).map(|p| p.steps.clone()).unwrap_or_default();
        for (i, step) in labdef.steps.iter().enumerate() {
            let done = passed.contains(&i);
            let frame = theme::card(t).stroke(egui::Stroke::new(1.0, if done { t.good } else { t.border }));
            frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if done { "✔" } else { "○" }).size(18.0).color(if done {
                        t.good
                    } else {
                        t.muted
                    }));
                    ui.label(RichText::new(format!("Step {} · {}", i + 1, step.title)).size(17.0).strong());
                });
                if let Some(Action::Run(cmd)) = markdown::show(ui, t, &step.task, &format!("{}-task-{i}", l.id)) {
                    self.run_in_console(&ctx, &cmd);
                }
                let shown = self.learn.hints.get(&i).copied().unwrap_or(0);
                for (h, hint) in step.hints.iter().enumerate().take(shown) {
                    ui.label(RichText::new(format!("💡 Hint {}: {hint}", h + 1)).color(t.amber));
                }
                ui.horizontal(|ui| {
                    let checking = self.jobs.busy(&format!("lab {} check {i}", l.id));
                    if ui.add_enabled(!checking, egui::Button::new(RichText::new("✔ Check").strong())).clicked() {
                        self.check_step(&ctx, l, i);
                    }
                    if checking {
                        ui.spinner();
                    }
                    if shown < step.hints.len()
                        && ui.button(format!("💡 Hint ({}/{})", shown + 1, step.hints.len())).clicked()
                    {
                        self.learn.hints.insert(i, shown + 1);
                    }
                    if !step.solution.is_empty() {
                        let open = self.learn.solutions.contains(&i);
                        if ui
                            .button(if open { "Hide solution" } else { "👁 Solution" })
                            .on_hover_text("Try first: struggling a bit is how it sticks")
                            .clicked()
                        {
                            if open {
                                self.learn.solutions.remove(&i);
                            } else {
                                self.learn.solutions.insert(i);
                            }
                        }
                    }
                });
                if let Some(results) = self.learn.results.get(&i) {
                    for r in results {
                        if r.passed {
                            ui.label(RichText::new(format!("  ✔ {}", r.name)).color(t.good));
                        } else {
                            ui.label(RichText::new(format!("  ✖ {}: {}", r.name, r.detail)).color(t.bad));
                        }
                    }
                }
                if self.learn.solutions.contains(&i) {
                    egui::Frame::new().fill(t.card_hover).corner_radius(8).inner_margin(10).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if let Some(Action::Run(cmd)) =
                            markdown::show(ui, t, &step.solution, &format!("{}-sol-{i}", l.id))
                        {
                            self.run_in_console(&ctx, &cmd);
                        }
                    });
                }
            });
            ui.add_space(8.0);
        }
        if self.progress.is_done(l) {
            theme::card(t).stroke(egui::Stroke::new(1.5, t.good)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("🥋 Lesson complete").size(18.0).strong().color(t.good));
                ui.label(
                    "Reset the lab whenever you want to practise it again from scratch; speed matters on exam day.",
                );
            });
        }
    }

    pub fn check_step(&mut self, ctx: &egui::Context, l: &Lesson, i: usize) {
        let Some(env) = self.env_for(&l.id) else { return };
        let Some(step) = l.lab.as_ref().and_then(|lab| lab.steps.get(i)) else { return };
        let checks = step.checks.clone();
        let id = l.id.clone();
        let needs = l.needs;
        self.jobs.spawn(ctx, format!("lab {} check {i}", l.id), move |_| {
            let results = match lab::preflight(needs, &env) {
                Ok(()) => lab::run_checks(&checks, &env),
                Err(e) => vec![poddojo_core::CheckResult { name: "lab cluster".into(), passed: false, detail: e }],
            };
            Done::Checked { lesson: id, step: i, results }
        });
    }

    fn lesson_nav(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson) {
        ui.separator();
        let prev = self.catalog.previous_lesson(&l.id).map(|x| (x.id.clone(), x.title.clone()));
        let next = self.catalog.next_lesson(&l.id).map(|x| (x.id.clone(), x.title.clone()));
        ui.horizontal(|ui| {
            if let Some((id, title)) = prev
                && ui.button(format!("← {title}")).clicked()
            {
                self.open_lesson(&id);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some((id, title)) = next {
                    let done = self.progress.is_done(l);
                    let clicked = if done {
                        accent_button(ui, t, &format!("{title} →")).clicked()
                    } else {
                        ui.button(format!("{title} →")).clicked()
                    };
                    if clicked {
                        self.open_lesson(&id);
                    }
                }
            });
        });
        section(ui, t, "");
    }
}
