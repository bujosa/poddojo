use std::collections::BTreeSet;

use egui::{Color32, Margin, RichText, Stroke, pos2, vec2};

use super::{ago, title};
use crate::App;
use crate::theme::{self, Tokens};

/// Weeks of activity shown in the heatmap.
const WEEKS: i64 = 18;

impl App {
    pub fn progress_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        let mut open = None;
        let mut review = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let (done, total) = self.progress.done_count(&self.catalog);
            let minutes: u32 = self.progress.lessons.values().map(|p| p.minutes).sum();
            let streak = self.progress.streak(chrono::Local::now().date_naive());
            let deck = self.progress.deck.stats(&self.catalog, poddojo_core::now());
            title(ui, &t, "Progress", "Everything you have done, shared by the app and the `dojo` CLI.");
            ui.columns(5, |cols| {
                stat(&mut cols[0], &t, &format!("{done}/{total}"), "lessons done", t.accent);
                stat(&mut cols[1], &t, &format!("{}h {:02}m", minutes / 60, minutes % 60), "studied", t.text);
                stat(&mut cols[2], &t, &format!("{streak}"), "day streak", t.amber);
                stat(&mut cols[3], &t, &format!("{}", deck.learned), "cards learned", t.good);
                stat(&mut cols[4], &t, &format!("{}", self.progress.exams.len()), "mock attempts", t.text);
            });
            ui.add_space(12.0);
            ui.columns(2, |cols| {
                theme::card(&t).show(&mut cols[0], |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Activity").size(17.0).strong());
                    ui.label(
                        RichText::new(format!(
                            "{} active days in total · the last {WEEKS} weeks",
                            self.progress.active_days.len()
                        ))
                        .size(12.5)
                        .color(t.muted),
                    );
                    ui.add_space(8.0);
                    heatmap(ui, &t, &self.progress.active_days);
                });
                theme::card(&t).show(&mut cols[1], |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Mock exam scores").size(17.0).strong());
                    ui.label(
                        RichText::new(
                            "Filled dots are fresh first attempts, hollow ones retakes. The line is the 66% pass mark.",
                        )
                        .size(12.5)
                        .color(t.muted),
                    );
                    ui.add_space(8.0);
                    let fresh: BTreeSet<i64> = self.progress.fresh_attempts().iter().map(|a| a.finished_at).collect();
                    let mut points: Vec<(i64, f32, bool)> = self
                        .progress
                        .exams
                        .iter()
                        .map(|a| (a.finished_at, a.percent, fresh.contains(&a.finished_at)))
                        .collect();
                    points.sort_by_key(|p| p.0);
                    trend(ui, &t, &points);
                });
            });
            ui.add_space(12.0);
            for track in self.catalog.tracks.clone() {
                let (d, n) = self.progress.track_counts(&self.catalog, &track.id);
                theme::card(&t).inner_margin(Margin::same(18)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&track.title).size(17.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(format!("{d} of {n} done")).color(t.muted));
                        });
                    });
                    let w = ui.available_width();
                    theme::bar(ui, &t, if n == 0 { 0.0 } else { d as f32 / n as f32 }, w, t.accent);
                    ui.add_space(8.0);
                    egui::Grid::new(format!("prog-{}", track.id)).striped(true).spacing([22.0, 7.0]).show(ui, |ui| {
                        for h in ["", "Lesson", "Concepts", "Quiz", "Lab", "Time", "Last seen"] {
                            ui.label(RichText::new(h).small().strong().color(t.muted));
                        }
                        ui.end_row();
                        for id in &track.lessons {
                            let Some(l) = self.catalog.lesson(id) else { continue };
                            let p = self.progress.lesson(id).cloned().unwrap_or_default();
                            theme::status_dot(ui, &t, self.progress.status(l));
                            if ui
                                .add(
                                    egui::Label::new(RichText::new(&l.title).color(t.text)).sense(egui::Sense::click()),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text("Open the lesson")
                                .clicked()
                            {
                                open = Some(id.clone());
                            }
                            ui.label(if p.read { RichText::new("✔ read").color(t.good) } else { RichText::new("") });
                            ui.label(match (l.quiz.is_empty(), p.quiz) {
                                (true, _) => RichText::new("—").color(t.muted),
                                (false, Some(q)) => RichText::new(format!("{}/{}", q.correct, q.total))
                                    .color(if q.passed() { t.good } else { t.amber }),
                                (false, None) => RichText::new(""),
                            });
                            ui.label(match &l.lab {
                                Some(lab) if !lab.steps.is_empty() => {
                                    let full = p.steps.len() >= lab.steps.len();
                                    RichText::new(format!("{}/{}", p.steps.len(), lab.steps.len())).color(if full {
                                        t.good
                                    } else {
                                        t.text
                                    })
                                }
                                _ => RichText::new("—").color(t.muted),
                            });
                            ui.label(if p.minutes > 0 { format!("{} min", p.minutes) } else { String::new() });
                            ui.label(
                                RichText::new(if p.last_seen > 0 { ago(p.last_seen) } else { String::new() })
                                    .small()
                                    .color(t.muted),
                            );
                            ui.end_row();
                        }
                    });
                });
                ui.add_space(10.0);
            }
            theme::card(&t).inner_margin(Margin::same(18)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Exam history").size(17.0).strong());
                if self.progress.exams.is_empty() {
                    ui.label(RichText::new("No mock exams taken yet.").color(t.muted));
                }
                let fresh: BTreeSet<i64> = self.progress.fresh_attempts().iter().map(|a| a.finished_at).collect();
                egui::Grid::new("exam-history").striped(true).spacing([22.0, 7.0]).show(ui, |ui| {
                    for a in self.progress.exams.iter().rev() {
                        ui.label(&a.title);
                        ui.label(RichText::new(format!("{:.1}%", a.percent)).strong().color(if a.passed {
                            t.good
                        } else {
                            t.amber
                        }));
                        ui.label(if a.passed { "pass" } else { "fail" });
                        ui.label(
                            RichText::new(if fresh.contains(&a.finished_at) { "fresh" } else { "retake" })
                                .small()
                                .color(t.muted),
                        );
                        ui.label(format!("{} min", a.minutes_used()));
                        ui.label(RichText::new(ago(a.finished_at)).small().color(t.muted));
                        if ui
                            .small_button("Review")
                            .on_hover_text("Scores per domain and task, with solutions")
                            .clicked()
                        {
                            review = Some(a.clone());
                        }
                        ui.end_row();
                    }
                });
            });
            ui.add_space(8.0);
            ui.label(RichText::new(format!("Saved in {}", self.paths.progress().display())).size(11.5).color(t.muted));
        });
        if let Some(id) = open {
            self.open_lesson(&id);
        }
        if let Some(a) = review {
            self.exam.result = Some(a);
            self.page = crate::Page::Exams;
        }
    }
}

fn stat(ui: &mut egui::Ui, t: &Tokens, value: &str, label: &str, color: Color32) {
    theme::card(t).inner_margin(Margin::symmetric(16, 14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(value).size(26.0).strong().color(color));
        ui.label(RichText::new(label).size(12.5).color(t.muted));
    });
}

/// A GitHub-style grid: one column per week, Monday on top, today in the last column.
fn heatmap(ui: &mut egui::Ui, t: &Tokens, days: &BTreeSet<String>) {
    use chrono::Datelike;
    let today = chrono::Local::now().date_naive();
    let monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
    let start = monday - chrono::Duration::weeks(WEEKS - 1);
    let gap = 3.0;
    let cell = ((ui.available_width() - gap * (WEEKS as f32 - 1.0)) / WEEKS as f32).clamp(8.0, 18.0);
    let size = vec2(WEEKS as f32 * (cell + gap), 7.0 * (cell + gap));
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter();
    let mut hovered = None;
    for w in 0..WEEKS {
        for d in 0..7 {
            let day = start + chrono::Duration::days(w * 7 + d);
            if day > today {
                continue;
            }
            let min = pos2(rect.min.x + w as f32 * (cell + gap), rect.min.y + d as f32 * (cell + gap));
            let r = egui::Rect::from_min_size(min, vec2(cell, cell));
            let active = days.contains(&day.format("%Y-%m-%d").to_string());
            let fill = if active { t.accent } else { t.card_hover };
            p.rect_filled(r, 3.0, fill);
            if day == today {
                p.rect_stroke(r, 3.0, Stroke::new(1.2, t.text.gamma_multiply(0.6)), egui::StrokeKind::Outside);
            }
            if resp.hover_pos().is_some_and(|h| r.contains(h)) {
                hovered = Some((day, active));
            }
        }
    }
    if let Some((day, active)) = hovered {
        resp.on_hover_text(format!("{} · {}", day.format("%a %d %b"), if active { "studied" } else { "no activity" }));
    }
}

/// Scores over time with the pass mark.
fn trend(ui: &mut egui::Ui, t: &Tokens, points: &[(i64, f32, bool)]) {
    let height = 150.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), egui::Sense::hover());
    let p = ui.painter();
    let plot = rect.shrink2(vec2(30.0, 10.0));
    for (pct, label) in [(0.0, "0"), (50.0, "50"), (100.0, "100")] {
        let y = plot.bottom() - plot.height() * pct / 100.0;
        p.hline(plot.x_range(), y, Stroke::new(1.0, t.border.gamma_multiply(0.6)));
        p.text(pos2(rect.min.x, y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(10.5), t.muted);
    }
    let pass_y = plot.bottom() - plot.height() * 0.66;
    p.hline(plot.x_range(), pass_y, Stroke::new(1.4, t.amber.gamma_multiply(0.8)));
    p.text(
        pos2(plot.left() + 4.0, pass_y - 2.0),
        egui::Align2::LEFT_BOTTOM,
        "pass 66%",
        egui::FontId::proportional(10.5),
        t.amber,
    );
    if points.is_empty() {
        p.text(
            plot.center(),
            egui::Align2::CENTER_CENTER,
            "No attempts yet",
            egui::FontId::proportional(13.0),
            t.muted,
        );
        return;
    }
    let n = points.len();
    let x = |i: usize| if n == 1 { plot.center().x } else { plot.left() + plot.width() * i as f32 / (n - 1) as f32 };
    let y = |v: f32| plot.bottom() - plot.height() * v.clamp(0.0, 100.0) / 100.0;
    let line: Vec<egui::Pos2> = points.iter().enumerate().map(|(i, pt)| pos2(x(i), y(pt.1))).collect();
    p.add(egui::Shape::line(line.clone(), Stroke::new(2.0, t.accent.gamma_multiply(0.7))));
    for (pt, pos) in points.iter().zip(line) {
        let color = if pt.1 >= 66.0 { t.good } else { t.bad };
        if pt.2 {
            p.circle_filled(pos, 4.5, color);
        } else {
            p.circle_stroke(pos, 4.5, Stroke::new(1.8, color));
        }
    }
}
