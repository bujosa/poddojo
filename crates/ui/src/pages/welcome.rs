//! The first screen: who is training today? Pick your profile and continue where you left off,
//! create a new one, or practise as a guest.

use egui::{Color32, RichText, Stroke};
use poddojo_core::Progress;
use poddojo_runner::cluster::Status;
use poddojo_runner::profiles::{self, Profile};

use super::{accent_button, ago};
use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};
use crate::{App, LessonTab, avatar};

impl App {
    pub fn welcome_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        if self.welcome.loaded.is_none() {
            let loaded = self
                .profiles
                .list()
                .iter()
                .map(|p| (p.clone(), Progress::load(&self.profiles.paths_for(p).progress()).unwrap_or_default()))
                .collect();
            self.welcome.loaded = Some(loaded);
            if self.profiles.list().is_empty() && self.welcome.new_name.is_empty() {
                self.welcome.new_name = profiles::system_name();
            }
        }
        let loaded = self.welcome.loaded.clone().unwrap_or_default();
        // A living cluster in the background.
        let time = if self.settings.motion { ui.input(|i| i.time) } else { 0.0 };
        crate::mark::node_field(ui.painter(), ui.max_rect(), time, t.accent);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let width = ui.available_width().min(900.0);
            let pad = ((ui.available_width() - width) / 2.0).max(0.0);
            ui.horizontal(|ui| {
                ui.add_space(pad);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.add_space(28.0);
                    ui.vertical_centered(|ui| {
                        crate::mark::show(ui, 112.0, 0.0, true, self.settings.motion);
                        ui.add_space(8.0);
                        ui.label(RichText::new("PodDojo").size(36.0).strong().color(strong(&t)));
                        ui.label(
                            RichText::new("Docker and the CKAD, hands-on, on your own machines.")
                                .size(16.0)
                                .color(t.muted),
                        );
                        ui.add_space(26.0);
                        ui.label(
                            RichText::new(if loaded.is_empty() { "Let's set you up" } else { "Who's training today?" })
                                .size(22.0)
                                .strong()
                                .color(strong(&t)),
                        );
                    });
                    ui.add_space(14.0);
                    for pair in loaded.chunks(2) {
                        ui.columns(2, |cols| {
                            for (i, (p, progress)) in pair.iter().enumerate() {
                                if let Some(col) = cols.get_mut(i) {
                                    self.profile_card(col, &t, p, progress);
                                }
                            }
                        });
                        ui.add_space(12.0);
                    }
                    ui.columns(2, |cols| {
                        if let Some(col) = cols.get_mut(0) {
                            self.new_profile_card(col, &t, loaded.is_empty());
                        }
                        if let Some(col) = cols.get_mut(1) {
                            self.guest_card(col, &t);
                        }
                    });
                    ui.add_space(22.0);
                    ui.vertical_centered(|ui| {
                        let machine = self.target().map(|m| m.name).unwrap_or_else(|| "no machine yet".into());
                        let cluster = match &self.cluster {
                            Some(Status::Running { nodes }) => {
                                format!("lab cluster {}", nodes.first().map_or("", |n| n.version.as_str()))
                            }
                            Some(Status::Missing) => "no lab cluster yet".into(),
                            Some(Status::Unreachable(_)) => "lab cluster unreachable".into(),
                            None => "checking the lab cluster…".into(),
                        };
                        ui.label(RichText::new(format!("Machine {machine}  ·  {cluster}")).size(12.5).color(t.muted));
                    });
                    ui.add_space(20.0);
                });
            });
        });
        self.remove_dialog(ui.ctx(), &t);
    }

    fn profile_card(&mut self, ui: &mut egui::Ui, t: &Tokens, p: &Profile, progress: &Progress) {
        let (done, total) = progress.done_count(&self.catalog);
        let readiness = progress.exam_readiness(&self.catalog);
        let lab = progress.labs_in_progress(&self.catalog).first().map(|l| (*l).clone());
        let resume = progress.resume(&self.catalog).cloned();
        let last = self.profiles.last().is_some_and(|l| l.id == p.id);
        theme::card(t)
            .stroke(Stroke::new(if last { 1.6 } else { 1.0 }, if last { t.accent } else { t.border }))
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    avatar(ui, p, 52.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&p.name).size(19.0).strong().color(strong(t)));
                        let seen = progress
                            .last_active()
                            .map_or("not started yet".to_string(), |ts| format!("last active {}", ago(ts)));
                        ui.label(RichText::new(seen).size(12.5).color(t.muted));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui
                            .add(
                                egui::Label::new(RichText::new("Remove").size(11.5).color(t.muted))
                                    .sense(egui::Sense::click()),
                            )
                            .clicked()
                        {
                            self.welcome.confirm_remove = Some(p.id.clone());
                        }
                    });
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{done}/{total} lessons")).size(13.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("CKAD: {}", readiness.verdict.title())).size(13.0).color(
                            match readiness.verdict {
                                poddojo_core::Verdict::Ready => t.good,
                                poddojo_core::Verdict::Close => t.accent,
                                _ => t.muted,
                            },
                        ));
                    });
                });
                let w = ui.available_width();
                theme::bar(ui, t, if total == 0 { 0.0 } else { done as f32 / total as f32 }, w, avatar_color(p));
                ui.add_space(10.0);
                let (kind, title) = match (&lab, &resume) {
                    (Some(l), _) => {
                        let lp = progress.lesson(&l.id).cloned().unwrap_or_default();
                        let steps = l.lab.as_ref().map_or(0, |x| x.steps.len());
                        (format!("Lab in progress · {}/{steps} steps", lp.steps.len().min(steps)), l.title.clone())
                    }
                    (None, Some(l)) => ("Next lesson".to_string(), l.title.clone()),
                    (None, None) => ("All lessons done".to_string(), "Take a mock exam".to_string()),
                };
                ui.label(RichText::new(kind.to_uppercase()).size(11.0).strong().color(if lab.is_some() {
                    t.amber
                } else {
                    t.accent
                }));
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                ui.label(RichText::new(title).size(15.0).color(t.text));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let label = if lab.is_some() { "Resume lab" } else { "Continue" };
                    if accent_button(ui, t, label).clicked() {
                        self.enter(p.clone());
                        if let Some(l) = &lab {
                            self.open_lesson(&l.id);
                            self.learn.tab = LessonTab::Lab;
                        } else if let Some(l) = &resume {
                            self.open_lesson(&l.id);
                        }
                    }
                    if icons::button(ui, Icon::Home, "Home").clicked() {
                        self.enter(p.clone());
                    }
                });
            });
    }

    fn new_profile_card(&mut self, ui: &mut egui::Ui, t: &Tokens, first: bool) {
        egui::Frame::new()
            .fill(t.card.gamma_multiply(0.6))
            .stroke(Stroke::new(1.0, if first { t.accent } else { t.border }))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
                    ui.painter().circle_stroke(rect.center(), 19.0, Stroke::new(1.5, t.accent));
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "+",
                        egui::FontId::proportional(22.0),
                        t.accent,
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(if first { "Create your profile" } else { "New profile" })
                                .size(17.0)
                                .strong()
                                .color(strong(t)),
                        );
                        ui.label(
                            RichText::new("Your own progress, quizzes, labs and exam history.")
                                .size(12.5)
                                .color(t.muted),
                        );
                    });
                });
                ui.add_space(10.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.welcome.new_name)
                        .hint_text("Your name")
                        .desired_width(f32::INFINITY)
                        .margin(egui::Margin::symmetric(10, 8)),
                );
                ui.add_space(8.0);
                let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let ready = !self.welcome.new_name.trim().is_empty();
                ui.add_enabled_ui(ready, |ui| {
                    if (accent_button(ui, t, "Create and start").clicked() || enter) && ready {
                        match self.profiles.create(&self.welcome.new_name) {
                            Ok(p) => {
                                self.welcome.new_name.clear();
                                self.enter(p);
                            }
                            Err(e) => self.toast(format!("Could not create the profile: {e}"), false),
                        }
                    }
                });
            });
    }

    fn guest_card(&mut self, ui: &mut egui::Ui, t: &Tokens) {
        egui::Frame::new()
            .fill(t.card.gamma_multiply(0.6))
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    avatar(ui, &Profile::guest(), 40.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Guest mode").size(17.0).strong().color(strong(t)));
                        ui.label(RichText::new("Try a lesson, a lab or an exam without saving anything.").size(12.5).color(t.muted));
                    });
                });
                ui.add_space(10.0);
                ui.label(RichText::new("Labs still run on your machine and lab cluster. Guest progress is cleared the next time guest mode starts.").size(12.5).color(t.muted));
                ui.add_space(8.0);
                if icons::button(ui, Icon::Play, "Start as guest").clicked() {
                    self.enter(Profile::guest());
                }
            });
    }

    fn remove_dialog(&mut self, ctx: &egui::Context, t: &Tokens) {
        let Some(id) = self.welcome.confirm_remove.clone() else { return };
        let name = self.profiles.list().iter().find(|p| p.id == id).map_or(id.clone(), |p| p.name.clone());
        let mut open = true;
        egui::Window::new("Remove profile?").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.label(format!("{name} disappears from this list. Their progress folder is kept on disk (renamed), so it can be recovered."));
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Remove").color(t.bad)).clicked() {
                    if let Err(e) = self.profiles.remove(&id) {
                        self.toast(format!("Could not remove the profile: {e}"), false);
                    }
                    self.welcome.loaded = None;
                    open = false;
                }
                if ui.button("Cancel").clicked() {
                    open = false;
                }
            });
        });
        if !open {
            self.welcome.confirm_remove = None;
        }
    }
}

fn avatar_color(p: &Profile) -> Color32 {
    crate::AVATARS.get(p.color as usize % crate::AVATARS.len()).copied().unwrap_or(Color32::GRAY)
}

fn strong(t: &Tokens) -> Color32 {
    if t.dark { Color32::WHITE } else { Color32::BLACK }
}
