use egui::RichText;

use super::{ago, title};
use crate::App;
use crate::theme;

impl App {
    pub fn progress_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        let mut open = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let (done, total) = self.progress.done_count(&self.catalog);
            let minutes: u32 = self.progress.lessons.values().map(|p| p.minutes).sum();
            title(
                ui,
                &t,
                "Progress",
                &format!(
                    "{done} of {total} lessons · {}h {:02}m studied · {} active days · saved in {}",
                    minutes / 60,
                    minutes % 60,
                    self.progress.active_days.len(),
                    self.paths.progress().display()
                ),
            );
            for track in self.catalog.tracks.clone() {
                theme::card(&t).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(&track.title).size(17.0).strong());
                    egui::Grid::new(format!("prog-{}", track.id)).striped(true).spacing([18.0, 6.0]).show(ui, |ui| {
                        for h in ["", "Lesson", "Concepts", "Quiz", "Lab", "Time", "Last seen"] {
                            ui.label(RichText::new(h).small().strong().color(t.muted));
                        }
                        ui.end_row();
                        for id in &track.lessons {
                            let Some(l) = self.catalog.lesson(id) else { continue };
                            let p = self.progress.lesson(id).cloned().unwrap_or_default();
                            theme::status_dot(ui, &t, self.progress.status(l));
                            if ui.link(&l.title).clicked() {
                                open = Some(id.clone());
                            }
                            ui.label(if p.read { "read" } else { "" });
                            ui.label(match (l.quiz.is_empty(), p.quiz) {
                                (true, _) => "—".to_string(),
                                (false, Some(q)) => format!("{}/{}", q.correct, q.total),
                                (false, None) => String::new(),
                            });
                            ui.label(match &l.lab {
                                Some(lab) if !lab.steps.is_empty() => format!("{}/{}", p.steps.len(), lab.steps.len()),
                                _ => "—".into(),
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
                ui.add_space(8.0);
            }
            theme::card(&t).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Exam history").size(17.0).strong());
                if self.progress.exams.is_empty() {
                    ui.label(RichText::new("No mock exams taken yet.").color(t.muted));
                }
                egui::Grid::new("exam-history").striped(true).spacing([18.0, 6.0]).show(ui, |ui| {
                    for a in self.progress.exams.iter().rev() {
                        ui.label(&a.title);
                        ui.label(RichText::new(format!("{:.1}%", a.percent)).strong().color(if a.passed {
                            t.good
                        } else {
                            t.amber
                        }));
                        ui.label(if a.passed { "pass" } else { "fail" });
                        ui.label(format!("{} min", a.minutes_used()));
                        ui.label(RichText::new(ago(a.finished_at)).small().color(t.muted));
                        ui.end_row();
                    }
                });
            });
        });
        if let Some(id) = open {
            self.open_lesson(&id);
        }
    }
}
