use std::time::Instant;

use egui::{Color32, Margin, RichText, Stroke};
use poddojo_core::{Lab, Lesson, LessonStatus, Needs, QuizScore, now};
use poddojo_runner::lab;

use super::{INSTRUCTIONS_MIN, TERMINAL_MIN, accent_button, domain_title};
use crate::icons::{self, Icon};
use crate::jobs::Done;
use crate::markdown::{self, Action};
use crate::theme::{self, ChoiceState, Tokens};
use crate::{App, LearnState, LessonTab, Page};

impl App {
    pub fn open_lesson(&mut self, id: &str) {
        let Some(l) = self.catalog.lesson(id).cloned() else { return };
        self.progress.open_lesson(&l, now());
        self.save_progress();
        let p = self.progress.lesson(id).cloned().unwrap_or_default();
        let lab_in_progress =
            l.has_lab() && !self.progress.is_done(&l) && (p.prepared_at.is_some() || !p.steps.is_empty());
        let tab = if lab_in_progress {
            LessonTab::Lab
        } else if !p.read {
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
            // Resume the lab session exactly where it was left.
            results: p.results.clone(),
            hints: p.hints.clone(),
            prepared: p.prepared_at.is_some(),
            ..Default::default()
        };
        self.page = Page::Learn;
    }

    pub fn learn_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        let lesson = self.learn.lesson.as_ref().and_then(|id| self.catalog.lesson(id)).cloned();
        if let Some(l) = &lesson
            && self.learn.tab == LessonTab::Lab
            && l.has_lab()
        {
            self.workspace(ui, &t, l);
            return;
        }
        let Some(l) = lesson else {
            self.syllabus(ui, &t);
            return;
        };
        if self.tree_hidden {
            self.lesson_view(ui, &t, &l);
            return;
        }
        super::split_left(ui, 320.0, |ui, left| {
            if left {
                self.lesson_tree(ui, &t);
            } else {
                self.lesson_view(ui, &t, &l);
            }
        });
    }

    // -------------------------------------------------------------------------------------------
    // Syllabus: every track and module at a glance, one click to any lesson.

    fn syllabus(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        let mut open = None;
        egui::ScrollArea::vertical().id_salt("syllabus").auto_shrink([false, false]).show(ui, |ui| {
            let width = (ui.available_width() - 18.0).min(1180.0);
            ui.set_max_width(width);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Learn").size(28.0).strong().color(strong(t)));
                    ui.label(
                        RichText::new(
                            "Docker first if containers are new to you, then the CKAD track, domain by domain.",
                        )
                        .color(t.muted),
                    );
                });
            });
            ui.add_space(12.0);
            if let Some(l) = self.progress.resume(&self.catalog).cloned() {
                theme::card(t).stroke(Stroke::new(1.4, t.accent.gamma_multiply(0.7))).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 10.0, t.accent_soft);
                        icons::paint(ui.painter(), rect.shrink(11.0), Icon::Play, t.accent);
                        ui.vertical(|ui| {
                            ui.label(RichText::new("UP NEXT").size(11.0).strong().color(t.accent));
                            ui.label(RichText::new(&l.title).size(17.0).strong().color(strong(t)));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if accent_button(ui, t, "Continue").clicked() {
                                open = Some(l.id.clone());
                            }
                            ui.label(
                                RichText::new(format!("{} · ~{} min", self.catalog.module_title(&l), l.minutes))
                                    .color(t.muted),
                            );
                        });
                    });
                });
                ui.add_space(18.0);
            }
            let cols = if width > 1050.0 { 3 } else { 2 };
            for track in self.catalog.tracks.clone() {
                let (d, n) = self.progress.track_counts(&self.catalog, &track.id);
                let minutes: u32 =
                    track.lessons.iter().filter_map(|id| self.catalog.lesson(id)).map(|l| l.minutes).sum();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&track.title).size(21.0).strong().color(strong(t)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{d}/{n} lessons · about {} h", (minutes + 30) / 60)).color(t.muted),
                        );
                    });
                });
                ui.label(RichText::new(&track.summary).color(t.muted));
                ui.add_space(4.0);
                let w = ui.available_width();
                theme::bar(ui, t, if n == 0 { 0.0 } else { d as f32 / n as f32 }, w, t.accent);
                ui.add_space(12.0);
                let modules: Vec<_> = track
                    .modules
                    .iter()
                    .filter(|m| {
                        track.lessons.iter().any(|id| self.catalog.lesson(id).is_some_and(|l| l.module == m.id))
                    })
                    .cloned()
                    .collect();
                for row in modules.chunks(cols) {
                    ui.columns(cols, |columns| {
                        for (module, col) in row.iter().zip(columns.iter_mut()) {
                            let lessons: Vec<poddojo_core::Lesson> = track
                                .lessons
                                .iter()
                                .filter_map(|id| self.catalog.lesson(id))
                                .filter(|l| l.module == module.id)
                                .cloned()
                                .collect();
                            if let Some(id) = self.module_card(col, t, module, &lessons) {
                                open = Some(id);
                            }
                        }
                    });
                    ui.add_space(12.0);
                }
                ui.add_space(14.0);
            }
        });
        if let Some(id) = open {
            self.open_lesson(&id);
        }
    }

    fn module_card(
        &self,
        ui: &mut egui::Ui,
        t: &Tokens,
        module: &poddojo_core::Module,
        lessons: &[Lesson],
    ) -> Option<String> {
        let mut open = None;
        let done = lessons.iter().filter(|l| self.progress.is_done(l)).count();
        theme::card(t).inner_margin(Margin::same(16)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.label(RichText::new(&module.title).size(15.5).strong().color(strong(t)));
            });
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{done}/{} done", lessons.len()))
                        .size(12.0)
                        .color(if done == lessons.len() { t.good } else { t.muted }),
                );
            });
            if !module.summary.is_empty() {
                ui.label(RichText::new(&module.summary).size(12.5).color(t.muted));
            }
            ui.add_space(6.0);
            for l in lessons {
                let status = self.progress.status(l);
                if lesson_row_minutes(ui, t, &l.title, status, l.minutes).clicked() {
                    open = Some(l.id.clone());
                }
            }
        });
        open
    }

    // -------------------------------------------------------------------------------------------
    // Lesson list

    fn lesson_tree(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        let mut open = None;
        let mut back = false;
        ui.horizontal(|ui| {
            back = icons::button(ui, Icon::Book, "All lessons")
                .on_hover_text("The syllabus: every track and module")
                .clicked();
            if ui.button("Hide list").on_hover_text("More room for the lesson").clicked() {
                self.tree_hidden = true;
            }
        });
        if back {
            self.learn = LearnState::default();
            return;
        }
        ui.add_space(6.0);
        egui::Frame::new()
            .fill(t.card_hover.gamma_multiply(0.5))
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(10)
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    icons::show(ui, Icon::Search, 15.0, t.muted);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.tree_filter)
                            .frame(egui::Frame::NONE)
                            .hint_text("Search lessons")
                            .desired_width(f32::INFINITY),
                    );
                });
            });
        ui.add_space(6.0);
        let filter = self.tree_filter.trim().to_lowercase();
        egui::ScrollArea::vertical().id_salt("tree").auto_shrink([false, false]).show(ui, |ui| {
            for track in &self.catalog.tracks {
                let (d, n) = self.progress.track_counts(&self.catalog, &track.id);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if track.id == "ckad" { "CKAD" } else { "Docker Foundations" })
                            .strong()
                            .size(15.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{d}/{n}")).size(12.0).color(t.muted));
                    });
                });
                let w = ui.available_width();
                theme::bar(ui, t, if n == 0 { 0.0 } else { d as f32 / n as f32 }, w, t.accent);
                ui.add_space(4.0);
                for module in &track.modules {
                    let ids: Vec<&String> = track
                        .lessons
                        .iter()
                        .filter(|id| {
                            self.catalog.lesson(id).is_some_and(|l| {
                                l.module == module.id && (filter.is_empty() || l.title.to_lowercase().contains(&filter))
                            })
                        })
                        .collect();
                    if ids.is_empty() {
                        continue;
                    }
                    let done = ids
                        .iter()
                        .filter(|id| self.catalog.lesson(id).is_some_and(|l| self.progress.is_done(l)))
                        .count();
                    let contains_open = ids.iter().any(|id| self.learn.lesson.as_deref() == Some(id.as_str()));
                    let header =
                        RichText::new(format!("{}   {done}/{}", module.title, ids.len())).size(13.5).color(t.text);
                    egui::CollapsingHeader::new(header)
                        .id_salt(format!("{}-{}", track.id, module.id))
                        .open(if filter.is_empty() { None } else { Some(true) })
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
                                if lesson_row(
                                    ui,
                                    t,
                                    &l.title,
                                    self.progress.status(l),
                                    self.learn.lesson.as_deref() == Some(id.as_str()),
                                )
                                .clicked()
                                {
                                    open = Some(id.clone());
                                }
                            }
                        });
                }
            }
        });
        if let Some(id) = open {
            self.open_lesson(&id);
        }
    }

    // -------------------------------------------------------------------------------------------
    // Lesson

    fn header(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson, compact: bool) {
        ui.horizontal_wrapped(|ui| {
            if self.tree_hidden
                && !compact
                && icons::button(ui, Icon::Book, "Lessons").on_hover_text("Show the lesson list").clicked()
            {
                self.tree_hidden = false;
            }
            ui.label(RichText::new(self.catalog.module_title(l)).color(t.muted));
            ui.label(RichText::new(format!("· ~{} min", l.minutes)).color(t.muted));
            if let Some(d) = &l.domain {
                theme::chip(ui, &format!("CKAD · {}", domain_title(d)), t.accent, t.accent_soft);
            }
            match l.needs {
                Needs::Cluster => theme::chip(ui, "lab cluster", t.amber, t.card_hover),
                Needs::Docker => theme::chip(ui, &format!("runs on {}", self.target_name()), t.muted, t.card_hover),
                Needs::None => {}
            }
        });
        ui.label(RichText::new(&l.title).size(if compact { 22.0 } else { 28.0 }).strong().color(strong(t)));
        ui.add_space(4.0);
        let p = self.progress.lesson(&l.id).cloned().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let mut cur = self.learn.tab;
            if theme::tab(ui, t, "Concepts", cur == LessonTab::Concepts, p.read).clicked() {
                cur = LessonTab::Concepts;
            }
            if !l.quiz.is_empty()
                && theme::tab(
                    ui,
                    t,
                    &format!("Quiz · {}", l.quiz.len()),
                    cur == LessonTab::Quiz,
                    p.quiz.is_some_and(|q| q.passed()),
                )
                .clicked()
            {
                cur = LessonTab::Quiz;
            }
            if let Some(lab) = &l.lab
                && !lab.steps.is_empty()
            {
                let done = p.steps.len() >= lab.steps.len();
                if theme::tab(
                    ui,
                    t,
                    &format!("Lab · {}/{}", p.steps.len().min(lab.steps.len()), lab.steps.len()),
                    cur == LessonTab::Lab,
                    done,
                )
                .clicked()
                {
                    cur = LessonTab::Lab;
                }
            }
            if cur != self.learn.tab {
                self.learn.tab = cur;
                self.learn.scroll_to_top = true;
            }
        });
        ui.add_space(6.0);
    }

    fn lesson_view(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson) {
        // Reading column of a comfortable width; on wide screens a side rail uses the rest of the
        // space (progress, next action, sections, the CKAD domain), and the pair is centred.
        let total = ui.available_rect_before_wrap();
        let rail_w = 300.0;
        let gap = 28.0;
        let wide = total.width() >= 1050.0;
        let col_w = if wide { (total.width() - rail_w - gap).min(860.0) } else { total.width() };
        let group_w = if wide { col_w + gap + rail_w } else { col_w };
        let x0 = total.min.x + ((total.width() - group_w) / 2.0).max(0.0);
        let col = egui::Rect::from_min_size(egui::pos2(x0, total.min.y), egui::vec2(col_w, total.height()));
        ui.scope_builder(egui::UiBuilder::new().max_rect(col), |ui| {
            ui.set_clip_rect(col.intersect(ui.clip_rect()));
            self.header(ui, t, l, false);
            ui.separator();
            let reset_scroll = std::mem::take(&mut self.learn.scroll_to_top);
            let mut area = egui::ScrollArea::vertical()
                .id_salt(format!("lesson-{}-{:?}", l.id, self.learn.tab))
                .auto_shrink([false, false]);
            if reset_scroll {
                area = area.vertical_scroll_offset(0.0);
            }
            area.show(ui, |ui| {
                ui.set_max_width(col_w - 14.0);
                ui.add_space(6.0);
                match self.learn.tab {
                    LessonTab::Concepts => self.concepts(ui, t, l),
                    LessonTab::Quiz | LessonTab::Lab => self.quiz(ui, t, l),
                }
                ui.add_space(18.0);
                self.lesson_nav(ui, t, l);
                ui.add_space(24.0);
            });
        });
        if wide {
            let rail = egui::Rect::from_min_size(
                egui::pos2(x0 + col_w + gap, total.min.y),
                egui::vec2(rail_w, total.height()),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(rail), |ui| {
                ui.set_clip_rect(rail.intersect(ui.clip_rect()));
                egui::ScrollArea::vertical().id_salt(format!("rail-{}", l.id)).auto_shrink([false, false]).show(
                    ui,
                    |ui| {
                        ui.set_width(rail_w - 10.0);
                        self.lesson_rail(ui, t, l);
                    },
                );
            });
        }
        ui.allocate_rect(total, egui::Sense::hover());
    }

    /// The side rail: where the learner stands in this lesson, the next action, the sections of the
    /// page and what part of the CKAD this is.
    fn lesson_rail(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson) {
        // Long section titles and competencies wrap inside the rail instead of widening it.
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        let p = self.progress.lesson(&l.id).cloned().unwrap_or_default();
        ui.add_space(4.0);
        theme::card(t).inner_margin(Margin::same(16)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("YOUR PROGRESS").size(11.5).strong().color(t.muted));
            ui.add_space(6.0);
            rail_check(ui, t, "Concepts", if p.read { "read".to_string() } else { "to read".to_string() }, p.read);
            if !l.quiz.is_empty() {
                let (txt, ok) = match p.quiz {
                    Some(q) => (format!("best {}/{}", q.correct, q.total), q.passed()),
                    None => (format!("{} questions", l.quiz.len()), false),
                };
                rail_check(ui, t, "Quiz", txt, ok);
            }
            if let Some(lab) = &l.lab
                && !lab.steps.is_empty()
            {
                let n = p.steps.len().min(lab.steps.len());
                rail_check(ui, t, "Lab", format!("{n}/{} steps", lab.steps.len()), n == lab.steps.len());
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("~{} min · {} min studied", l.minutes, p.minutes)).size(12.5).color(t.muted),
            );
            ui.add_space(10.0);
            match self.learn.tab {
                LessonTab::Concepts => {
                    if !p.read
                        && ui.add_sized([ui.available_width(), 32.0], egui::Button::new("Mark as read")).clicked()
                    {
                        self.progress.mark_read(l, now());
                        self.save_progress();
                    }
                    let next = if !l.quiz.is_empty() {
                        Some((LessonTab::Quiz, "Take the quiz"))
                    } else if l.has_lab() {
                        Some((LessonTab::Lab, "Start the lab"))
                    } else {
                        None
                    };
                    if let Some((tab, label)) = next
                        && wide_accent(ui, t, label).clicked()
                    {
                        if !p.read {
                            self.progress.mark_read(l, now());
                            self.save_progress();
                        }
                        self.learn.tab = tab;
                        self.learn.scroll_to_top = true;
                    }
                }
                LessonTab::Quiz | LessonTab::Lab => {
                    let answered = self.learn.answers.iter().filter(|a| a.is_some()).count();
                    if self.learn.submitted {
                        if l.has_lab() && wide_accent(ui, t, "Start the lab").clicked() {
                            self.learn.tab = LessonTab::Lab;
                            self.learn.scroll_to_top = true;
                        }
                    } else {
                        ui.label(RichText::new(format!("{answered} of {} answered", l.quiz.len())).size(13.0));
                        let w = ui.available_width();
                        theme::bar(
                            ui,
                            t,
                            if l.quiz.is_empty() { 0.0 } else { answered as f32 / l.quiz.len() as f32 },
                            w,
                            t.accent,
                        );
                        ui.add_space(8.0);
                        let ready = answered == l.quiz.len();
                        ui.add_enabled_ui(ready, |ui| {
                            if wide_accent(ui, t, "Submit answers").clicked() {
                                self.submit_quiz(l);
                            }
                        });
                    }
                }
            }
        });
        if self.learn.tab == LessonTab::Concepts {
            let sections = markdown::headings(&l.body);
            if !sections.is_empty() {
                ui.add_space(12.0);
                theme::card(t).inner_margin(Margin::same(16)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("ON THIS PAGE").size(11.5).strong().color(t.muted));
                    ui.add_space(4.0);
                    for (i, (level, title)) in sections.iter().enumerate() {
                        let text = if *level == 3 { format!("    {title}") } else { title.clone() };
                        let resp = ui.add(
                            egui::Label::new(
                                RichText::new(text).size(if *level == 3 { 13.0 } else { 14.0 }).color(t.text),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            self.learn.scroll_to_section = Some(i);
                        }
                        ui.add_space(2.0);
                    }
                });
            }
        }
        if let Some(d) = l.domain.as_deref().and_then(|id| poddojo_core::DOMAINS.iter().find(|x| x.id == id)) {
            ui.add_space(12.0);
            theme::card(t).inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("CKAD DOMAIN").size(11.5).strong().color(t.muted));
                ui.label(RichText::new(d.title).size(14.5).strong().color(strong(t)));
                ui.label(RichText::new(format!("{}% of the exam", d.weight)).size(12.5).color(t.accent));
                ui.add_space(4.0);
                for c in d.competencies {
                    ui.label(RichText::new(format!("·  {c}")).size(12.5).color(t.muted));
                }
            });
        }
    }

    fn submit_quiz(&mut self, l: &Lesson) {
        let score = self.progress.record_quiz_answers(l, &self.learn.answers, now());
        self.save_progress();
        self.learn.submitted = true;
        self.toast(
            format!(
                "Quiz: {}/{}{}",
                score.correct,
                score.total,
                if score.passed() { " · passed" } else { " · 70% needed · misses come back in Review" }
            ),
            score.passed(),
        );
    }

    fn concepts(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson) {
        if !l.objectives.is_empty() {
            theme::card(t).stroke(Stroke::new(1.0, t.accent.gamma_multiply(0.45))).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("YOU WILL BE ABLE TO").size(12.0).strong().color(t.accent));
                ui.add_space(2.0);
                for o in &l.objectives {
                    ui.horizontal_top(|ui| {
                        icons::show(ui, Icon::Check, 15.0, t.accent);
                        ui.label(o);
                    });
                }
            });
        }
        let target = self.learn.scroll_to_section.take();
        if let Some(Action::Run(cmd)) = markdown::show_scrolled(ui, t, &l.body, &l.id, target) {
            self.run_in_terminal(&cmd);
        }
        ui.add_space(14.0);
        let read = self.progress.lesson(&l.id).is_some_and(|p| p.read);
        let next = if !l.quiz.is_empty() {
            Some((LessonTab::Quiz, "Take the quiz"))
        } else if l.has_lab() {
            Some((LessonTab::Lab, "Start the lab"))
        } else {
            None
        };
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if read {
                    icons::show(ui, Icon::Check, 18.0, t.good);
                    ui.label(RichText::new("Concepts read").color(t.good));
                } else {
                    ui.label(RichText::new("Done reading?").color(t.muted));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((tab, label)) = next
                        && accent_button(ui, t, label).clicked()
                    {
                        if !read {
                            self.progress.mark_read(l, now());
                            self.save_progress();
                        }
                        self.learn.tab = tab;
                        self.learn.scroll_to_top = true;
                    }
                    if !read && ui.button("Mark as read").clicked() {
                        self.progress.mark_read(l, now());
                        self.save_progress();
                    }
                });
            });
        });
    }

    fn quiz(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson) {
        if self.learn.answers.len() != l.quiz.len() {
            self.learn.answers = vec![None; l.quiz.len()];
        }
        let submitted = self.learn.submitted;
        let answered = self.learn.answers.iter().filter(|a| a.is_some()).count();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{answered} of {} answered", l.quiz.len())).color(t.muted));
            if let Some(best) = self.progress.lesson(&l.id).and_then(|p| p.quiz) {
                ui.label(RichText::new("·").color(t.muted));
                ui.label(RichText::new(format!("best {}/{}", best.correct, best.total)).color(if best.passed() {
                    t.good
                } else {
                    t.muted
                }));
            }
            ui.label(RichText::new("· 70% to pass").color(t.muted));
        });
        ui.add_space(6.0);
        for (i, q) in l.quiz.iter().enumerate() {
            theme::card(t).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(format!("QUESTION {}", i + 1)).size(11.5).strong().color(t.muted));
                let w = ui.available_width();
                ui.label(crate::markdown::inline_job(&q.q, 16.5, strong(t), t.card_hover, w));
                ui.add_space(8.0);
                let picked_now = self.learn.answers.get(i).copied().flatten();
                for (pos, j) in q.order(&l.id).into_iter().enumerate() {
                    let Some(choice) = q.choices.get(j) else { continue };
                    let state = match (submitted, picked_now == Some(j), j == q.answer) {
                        (false, true, _) => ChoiceState::Picked,
                        (false, false, _) => ChoiceState::Idle,
                        (true, true, true) => ChoiceState::Correct,
                        (true, true, false) => ChoiceState::Wrong,
                        (true, false, true) => ChoiceState::Missed,
                        (true, false, false) => ChoiceState::Idle,
                    };
                    let letter = (b'A' + pos as u8) as char;
                    if theme::choice(ui, t, letter, choice, state, !submitted).clicked()
                        && let Some(a) = self.learn.answers.get_mut(i)
                    {
                        *a = Some(j);
                    }
                    ui.add_space(4.0);
                }
                if submitted {
                    let right = picked_now == Some(q.answer);
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .fill(if right { t.good } else { t.amber }.gamma_multiply(0.1))
                        .corner_radius(8)
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(if right { "Correct" } else { "Not quite" }).strong().color(if right {
                                    t.good
                                } else {
                                    t.amber
                                }),
                            );
                            ui.label(RichText::new(&q.why).color(t.text));
                        });
                }
            });
            ui.add_space(8.0);
        }
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if !submitted {
                    let all = answered == l.quiz.len();
                    ui.label(
                        RichText::new(if all { "Ready when you are." } else { "Answer every question to submit." })
                            .color(t.muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(all, |ui| {
                            if accent_button(ui, t, "Submit answers").clicked() {
                                self.submit_quiz(l);
                            }
                        });
                    });
                } else {
                    let correct = l.quiz.iter().zip(&self.learn.answers).filter(|(q, a)| **a == Some(q.answer)).count();
                    let passed = QuizScore { correct: correct as u32, total: l.quiz.len() as u32 }.passed();
                    ui.label(
                        RichText::new(format!("{correct}/{} correct", l.quiz.len()))
                            .size(17.0)
                            .strong()
                            .color(if passed { t.good } else { t.amber }),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if l.has_lab() && accent_button(ui, t, "Start the lab").clicked() {
                            self.learn.tab = LessonTab::Lab;
                            self.learn.scroll_to_top = true;
                        }
                        if ui.button("Retake").clicked() {
                            self.learn.answers = vec![None; l.quiz.len()];
                            self.learn.submitted = false;
                            self.learn.scroll_to_top = true;
                        }
                    });
                }
            });
        });
    }

    // -------------------------------------------------------------------------------------------
    // Lab workspace: steps on the left, terminal on the right.

    fn workspace(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson) {
        let Some(labdef) = l.lab.clone() else { return };
        ui.horizontal(|ui| {
            if icons::button(ui, Icon::Book, "Lessons").on_hover_text("Back to the lesson list").clicked() {
                self.learn.tab = LessonTab::Concepts;
            }
            ui.add_space(6.0);
            ui.vertical(|ui| self.header(ui, t, l, true));
        });
        ui.add_space(4.0);
        let total = ui.available_rect_before_wrap();
        let mut split = self.settings.lab_split;
        let default = (total.width() * 0.38).clamp(INSTRUCTIONS_MIN, 620.0);
        let (left, right, save) =
            super::resizable_split(ui, t, "lab", total, &mut split, default, (INSTRUCTIONS_MIN, TERMINAL_MIN));
        self.settings.lab_split = split;
        if save {
            self.save_settings();
        }
        let left_w = left.width();
        ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
            ui.set_clip_rect(left.intersect(ui.clip_rect()));
            let reset = std::mem::take(&mut self.learn.scroll_to_top);
            let mut area = egui::ScrollArea::vertical().id_salt(format!("steps-{}", l.id)).auto_shrink([false, false]);
            if reset {
                area = area.vertical_scroll_offset(0.0);
            }
            area.show(ui, |ui| {
                ui.set_width((left_w - 12.0).max(0.0));
                self.lab_toolbar(ui, t, l, &labdef);
                ui.add_space(10.0);
                self.lab_steps(ui, t, l, &labdef);
                ui.add_space(12.0);
                self.lesson_nav(ui, t, l);
                ui.add_space(12.0);
            });
        });
        ui.scope_builder(egui::UiBuilder::new().max_rect(right), |ui| {
            let chips = self.terminal_chips();
            self.terminal(ui, &chips);
        });
        ui.allocate_rect(total, egui::Sense::hover());
    }

    fn lab_toolbar(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson, labdef: &Lab) {
        let ctx = ui.ctx().clone();
        let env = self.env_for(&l.id);
        let done = self.progress.lesson(&l.id).map(|p| p.steps.len()).unwrap_or(0).min(labdef.steps.len());
        let n = labdef.steps.len();
        theme::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{done} of {n} steps")).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let prepared_at = self.progress.lesson(&l.id).and_then(|p| p.prepared_at);
                    let (dot, text) = match (self.learn.prepared, prepared_at) {
                        (_, Some(ts)) => (t.good, format!("prepared {}", super::ago(ts))),
                        (true, None) => (t.good, "lab ready".to_string()),
                        (false, None) => (t.muted, "not prepared yet".to_string()),
                    };
                    ui.label(RichText::new(text).size(12.0).color(dot));
                    let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, dot);
                });
            });
            let w = ui.available_width();
            theme::bar(ui, t, if n == 0 { 0.0 } else { done as f32 / n as f32 }, w, t.good);
            ui.add_space(6.0);
            if !labdef.summary.is_empty() {
                ui.label(RichText::new(&labdef.summary).color(t.text));
            }
            if l.needs == Needs::Cluster && !self.cluster_running() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("This lab needs the lab cluster, which is not running.").color(t.amber));
                    if ui.button("Open Cluster page").clicked() {
                        self.page = Page::Cluster;
                    }
                });
            }
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                let busy = self.jobs.busy(&format!("lab {}", l.id));
                let ready = env.is_some() && !busy;
                if ui
                    .add_enabled(ready, egui::Button::new(RichText::new("Prepare lab").strong()))
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
                if ui.add_enabled(ready, egui::Button::new("Check all")).clicked() {
                    let passed = self.progress.lesson(&l.id).map(|p| p.steps.clone()).unwrap_or_default();
                    for i in (0..n).filter(|i| !passed.contains(i)) {
                        self.check_step(&ctx, l, i);
                    }
                }
                if ui
                    .add_enabled(ready, egui::Button::new("Reset"))
                    .on_hover_text("Remove what this lab created and start over")
                    .clicked()
                    && let Some(env) = env.clone()
                {
                    let lesson = l.clone();
                    self.jobs.spawn(&ctx, format!("lab {} reset", l.id), move |_| {
                        let (ok, log) = lab::reset(&lesson, &env);
                        Done::Reset { lesson: lesson.id.clone(), ok, log }
                    });
                }
                if busy {
                    ui.spinner();
                }
            });
            if let Some(e) = &env {
                ui.label(RichText::new(format!("Lab folder {}", e.workdir.display())).size(11.5).color(t.muted));
            }
            if !self.learn.lab_log.is_empty() {
                egui::CollapsingHeader::new(RichText::new("Setup log").size(12.0).color(t.muted))
                    .id_salt("lablog")
                    .show(ui, |ui| {
                        ui.label(RichText::new(self.learn.lab_log.trim_end()).monospace().size(12.0));
                    });
            }
        });
    }

    fn lab_steps(&mut self, ui: &mut egui::Ui, t: &Tokens, l: &Lesson, labdef: &Lab) {
        let ctx = ui.ctx().clone();
        let passed = self.progress.lesson(&l.id).map(|p| p.steps.clone()).unwrap_or_default();
        let current = (0..labdef.steps.len()).find(|i| !passed.contains(i));
        for (i, step) in labdef.steps.iter().enumerate() {
            let done = passed.contains(&i);
            let is_current = current == Some(i);
            let open = is_current || self.learn.expanded.contains(&i);
            let (stroke, fill) = if is_current {
                (Stroke::new(1.6, t.accent), t.card)
            } else if done {
                (Stroke::new(1.0, t.good.gamma_multiply(0.6)), t.card)
            } else {
                (Stroke::new(1.0, t.border), t.card.gamma_multiply(0.7))
            };
            egui::Frame::new()
                .fill(fill)
                .stroke(stroke)
                .corner_radius(12)
                .inner_margin(Margin::symmetric(14, 12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let header = ui.horizontal(|ui| {
                        step_badge(ui, t, i + 1, done, is_current);
                        ui.label(RichText::new(&step.title).size(16.0).strong().color(if done || is_current {
                            strong(t)
                        } else {
                            t.muted
                        }));
                        if !is_current {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                chevron(ui, t, open);
                            });
                        }
                    });
                    if !is_current && header.response.interact(egui::Sense::click()).clicked() {
                        if open {
                            self.learn.expanded.remove(&i);
                        } else {
                            self.learn.expanded.insert(i);
                        }
                    }
                    if !open {
                        return;
                    }
                    ui.add_space(4.0);
                    if let Some(Action::Run(cmd)) = markdown::show(ui, t, &step.task, &format!("{}-task-{i}", l.id)) {
                        self.run_in_terminal(&cmd);
                    }
                    let shown = self.learn.hints.get(&i).copied().unwrap_or(0);
                    for (h, hint) in step.hints.iter().enumerate().take(shown) {
                        egui::Frame::new()
                            .fill(t.amber.gamma_multiply(0.1))
                            .corner_radius(8)
                            .inner_margin(Margin::symmetric(10, 6))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(RichText::new(format!("Hint {}  ·  {hint}", h + 1)).color(t.text));
                            });
                        ui.add_space(3.0);
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let checking = self.jobs.busy(&format!("lab {} check {i}", l.id));
                        let label = if done { "Check again" } else { "Check" };
                        let resp = if is_current { accent_button(ui, t, label) } else { ui.button(label) };
                        if resp.clicked() && !checking {
                            self.check_step(&ctx, l, i);
                        }
                        if checking {
                            ui.spinner();
                        }
                        if shown < step.hints.len()
                            && ui.button(format!("Hint {}/{}", shown + 1, step.hints.len())).clicked()
                        {
                            self.learn.hints.insert(i, shown + 1);
                            self.progress.record_hints(l, i, shown + 1);
                            self.save_progress();
                        }
                        if !step.solution.is_empty() {
                            let showing = self.learn.solutions.contains(&i);
                            if ui
                                .button(if showing { "Hide solution" } else { "Solution" })
                                .on_hover_text("Try first: struggling a bit is how it sticks")
                                .clicked()
                            {
                                if showing {
                                    self.learn.solutions.remove(&i);
                                } else {
                                    self.learn.solutions.insert(i);
                                }
                            }
                        }
                    });
                    if let Some(results) = self.learn.results.get(&i) {
                        ui.add_space(4.0);
                        for r in results {
                            ui.horizontal_top(|ui| {
                                let (c, txt) = if r.passed {
                                    (t.good, r.name.clone())
                                } else {
                                    (t.bad, format!("{}: {}", r.name, r.detail))
                                };
                                let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 18.0), egui::Sense::hover());
                                ui.painter().circle_filled(rect.center(), 4.0, c);
                                ui.label(RichText::new(txt).size(13.5).color(if r.passed { t.text } else { t.bad }));
                            });
                        }
                    }
                    if self.learn.solutions.contains(&i) {
                        ui.add_space(4.0);
                        egui::Frame::new().fill(t.card_hover).corner_radius(8).inner_margin(10).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if let Some(Action::Run(cmd)) =
                                markdown::show(ui, t, &step.solution, &format!("{}-sol-{i}", l.id))
                            {
                                self.run_in_terminal(&cmd);
                            }
                        });
                    }
                });
            ui.add_space(8.0);
        }
        if self.progress.is_done(l) {
            theme::card(t).stroke(Stroke::new(1.5, t.good)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    icons::show(ui, Icon::Check, 22.0, t.good);
                    ui.label(RichText::new("Lesson complete").size(18.0).strong().color(t.good));
                });
                ui.label("Reset the lab and do it again from memory: speed is what wins the exam.");
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
        let prev = self.catalog.previous_lesson(&l.id).map(|x| (x.id.clone(), x.title.clone()));
        let next = self.catalog.next_lesson(&l.id).map(|x| (x.id.clone(), x.title.clone()));
        ui.separator();
        ui.horizontal(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            if let Some((id, title)) = prev
                && ui.button(format!("Previous: {title}")).on_hover_text("Previous lesson").clicked()
            {
                self.open_lesson(&id);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some((id, title)) = next {
                    let clicked = if self.progress.is_done(l) {
                        accent_button(ui, t, &format!("Next: {title}")).clicked()
                    } else {
                        ui.button(format!("Next: {title}")).on_hover_text("Next lesson").clicked()
                    };
                    if clicked {
                        self.open_lesson(&id);
                    }
                }
            });
        });
    }
}

fn strong(t: &Tokens) -> Color32 {
    if t.dark { Color32::WHITE } else { Color32::BLACK }
}

/// A lesson in the list: status dot + wrapped title, highlighted when open.
fn lesson_row(ui: &mut egui::Ui, t: &Tokens, title: &str, status: LessonStatus, selected: bool) -> egui::Response {
    let width = ui.available_width();
    let galley = ui.painter().layout(title.to_string(), egui::FontId::proportional(13.5), t.text, width - 34.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, galley.size().y + 12.0), egui::Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 8.0, t.accent_soft);
    } else if resp.hovered() {
        p.rect_filled(rect, 8.0, t.card);
    }
    let dot_rect =
        egui::Rect::from_center_size(egui::pos2(rect.min.x + 12.0, rect.min.y + 6.0 + 9.0), egui::vec2(14.0, 14.0));
    paint_status(p, t, dot_rect, status);
    let color = if selected {
        t.accent
    } else if status == LessonStatus::Done {
        t.muted
    } else {
        t.text
    };
    p.galley_with_override_text_color(egui::pos2(rect.min.x + 26.0, rect.min.y + 6.0), galley, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn paint_status(p: &egui::Painter, t: &Tokens, rect: egui::Rect, status: LessonStatus) {
    let c = rect.center();
    match status {
        LessonStatus::Done => {
            p.circle_filled(c, 6.0, t.good);
            icons::paint(p, rect.shrink(3.0), Icon::Check, t.bg);
        }
        LessonStatus::InProgress => {
            p.circle_stroke(c, 5.5, Stroke::new(1.6, t.amber));
            p.circle_filled(c, 2.6, t.amber);
        }
        LessonStatus::NotStarted => {
            p.circle_stroke(c, 5.5, Stroke::new(1.3, t.muted));
        }
    }
}

/// The numbered circle in front of a lab step.
fn step_badge(ui: &mut egui::Ui, t: &Tokens, n: usize, done: bool, current: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::hover());
    let c = rect.center();
    let p = ui.painter();
    if done {
        p.circle_filled(c, 12.0, t.good);
        icons::paint(p, rect.shrink(6.0), Icon::Check, t.bg);
    } else if current {
        p.circle_filled(c, 12.0, t.accent);
        p.text(c, egui::Align2::CENTER_CENTER, n.to_string(), egui::FontId::proportional(13.0), t.bg);
    } else {
        p.circle_stroke(c, 11.0, Stroke::new(1.4, t.muted));
        p.text(c, egui::Align2::CENTER_CENTER, n.to_string(), egui::FontId::proportional(13.0), t.muted);
    }
}

/// A small chevron: down when open, right when closed.
fn chevron(ui: &mut egui::Ui, t: &Tokens, open: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
    let c = rect.center();
    let pts = if open {
        vec![c + egui::vec2(-4.0, -2.0), c + egui::vec2(0.0, 2.5), c + egui::vec2(4.0, -2.0)]
    } else {
        vec![c + egui::vec2(-2.0, -4.0), c + egui::vec2(2.5, 0.0), c + egui::vec2(-2.0, 4.0)]
    };
    ui.painter().add(egui::Shape::line(pts, Stroke::new(1.6, t.muted)));
}

/// A lesson in a module card: status dot, title and minutes.
fn lesson_row_minutes(
    ui: &mut egui::Ui,
    t: &Tokens,
    title: &str,
    status: LessonStatus,
    minutes: u32,
) -> egui::Response {
    let width = ui.available_width();
    let galley = ui.painter().layout(title.to_string(), egui::FontId::proportional(14.0), t.text, width - 80.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, galley.size().y + 14.0), egui::Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 8.0, t.card_hover);
    }
    paint_status(
        p,
        t,
        egui::Rect::from_center_size(egui::pos2(rect.min.x + 12.0, rect.min.y + 7.0 + 9.0), egui::vec2(14.0, 14.0)),
        status,
    );
    let color = if status == LessonStatus::Done { t.muted } else { t.text };
    p.galley_with_override_text_color(egui::pos2(rect.min.x + 28.0, rect.min.y + 7.0), galley, color);
    p.text(
        egui::pos2(rect.max.x - 8.0, rect.min.y + 16.0),
        egui::Align2::RIGHT_CENTER,
        format!("{minutes} min"),
        egui::FontId::proportional(12.0),
        t.muted,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A progress line in the side rail: label, status text and a dot.
fn rail_check(ui: &mut egui::Ui, t: &Tokens, label: &str, status: String, done: bool) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
        if done {
            ui.painter().circle_filled(rect.center(), 8.0, t.good);
            icons::paint(ui.painter(), rect.shrink(4.0), Icon::Check, t.bg);
        } else {
            ui.painter().circle_stroke(rect.center(), 7.5, Stroke::new(1.4, t.muted));
        }
        ui.label(RichText::new(label).size(14.0).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(status).size(12.5).color(if done { t.good } else { t.muted }));
        });
    });
}

/// A full-width accent button for the side rail.
fn wide_accent(ui: &mut egui::Ui, t: &Tokens, text: &str) -> egui::Response {
    let fg = if t.dark { Color32::from_rgb(4, 30, 28) } else { Color32::WHITE };
    ui.add_sized([ui.available_width(), 34.0], egui::Button::new(RichText::new(text).strong().color(fg)).fill(t.accent))
}
