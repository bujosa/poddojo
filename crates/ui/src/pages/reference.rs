//! Reference: cheat sheets and copy-ready YAML to keep at hand while practising, the in-app
//! counterpart of the kubernetes.io pages allowed in the exam.

use egui::{Margin, RichText, Stroke};

use super::title;
use crate::App;
use crate::icons::{self, Icon};
use crate::markdown::{self, Action};
use crate::theme::{self, Tokens};

#[derive(Default)]
pub struct ReferenceState {
    pub page: Option<String>,
    pub filter: String,
    pub scroll_to: Option<usize>,
    /// The floating reference window (⌘K) is open.
    pub overlay: bool,
}

impl App {
    pub fn reference_page(&mut self, ui: &mut egui::Ui) {
        let t = self.t;
        if self.catalog.reference.is_empty() {
            title(ui, &t, "Reference", "No reference pages in this build.");
            return;
        }
        let current = self
            .reference
            .page
            .clone()
            .filter(|id| self.catalog.reference_page(id).is_some())
            .unwrap_or_else(|| self.catalog.reference.first().map(|p| p.id.clone()).unwrap_or_default());
        super::split_left(ui, 280.0, |ui, left| {
            if left {
                self.reference_list(ui, &t, &current);
            } else {
                self.reference_body(ui, &t, &current);
            }
        });
    }

    fn reference_list(&mut self, ui: &mut egui::Ui, t: &Tokens, current: &str) {
        ui.label(RichText::new("Reference").size(22.0).strong());
        ui.label(RichText::new("Keep it open while you practise.").size(12.5).color(t.muted));
        ui.add_space(8.0);
        egui::Frame::new()
            .fill(t.card_hover.gamma_multiply(0.5))
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(10)
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    icons::show(ui, Icon::Search, 15.0, t.muted);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.reference.filter)
                            .frame(egui::Frame::NONE)
                            .hint_text("Search: probe, NetworkPolicy, jsonpath…")
                            .desired_width(f32::INFINITY),
                    );
                });
            });
        ui.add_space(8.0);
        let filter = self.reference.filter.trim().to_lowercase();
        egui::ScrollArea::vertical().id_salt("ref-list").auto_shrink([false, false]).show(ui, |ui| {
            for page in &self.catalog.reference {
                let hits = if filter.is_empty() { 0 } else { page.body.to_lowercase().matches(&filter).count() };
                if !filter.is_empty() && hits == 0 && !page.title.to_lowercase().contains(&filter) {
                    continue;
                }
                let selected = page.id == current;
                let resp = egui::Frame::new()
                    .fill(if selected { t.accent_soft } else { egui::Color32::TRANSPARENT })
                    .corner_radius(10)
                    .inner_margin(Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(&page.title).size(14.5).strong().color(if selected {
                            t.accent
                        } else {
                            t.text
                        }));
                        let sub = if hits > 0 { format!("{hits} match(es)") } else { page.summary.clone() };
                        ui.label(RichText::new(sub).size(12.0).color(t.muted));
                    })
                    .response
                    .interact(egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &page.title));
                if resp.clicked() {
                    self.reference.page = Some(page.id.clone());
                    self.reference.scroll_to = None;
                    // Jump to the first section that mentions the search.
                    if !filter.is_empty() {
                        self.reference.scroll_to = section_with(&page.body, &filter);
                    }
                }
                ui.add_space(2.0);
            }
        });
    }

    fn reference_body(&mut self, ui: &mut egui::Ui, t: &Tokens, current: &str) {
        let Some(page) = self.catalog.reference_page(current).cloned() else { return };
        let target = self.reference.scroll_to.take();
        let mut run = None;
        egui::ScrollArea::vertical().id_salt(("ref-body", current.to_string())).auto_shrink([false, false]).show(
            ui,
            |ui| {
                let width = ui.available_width().min(900.0);
                ui.set_max_width(width);
                theme::card(t).inner_margin(Margin::symmetric(28, 22)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(&page.title).size(26.0).strong().color(super::strong_text(t)));
                    if !page.summary.is_empty() {
                        ui.label(RichText::new(&page.summary).color(t.muted));
                    }
                    ui.add_space(10.0);
                    if let Some(Action::Run(cmd)) = markdown::show_scrolled(ui, t, &page.body, &page.id, target) {
                        run = Some(cmd);
                    }
                });
                ui.add_space(24.0);
            },
        );
        if let Some(cmd) = run {
            self.run_in_terminal(&cmd);
        }
    }
}

impl App {
    /// The reference in a movable window over the workspace: the in-app counterpart of the docs
    /// browser tab the exam allows next to the terminal.
    pub fn reference_overlay(&mut self, ctx: &egui::Context) {
        if !self.reference.overlay || self.catalog.reference.is_empty() || self.profile.is_none() {
            return;
        }
        let t = self.t;
        let mut open = true;
        let current = self
            .reference
            .page
            .clone()
            .filter(|id| self.catalog.reference_page(id).is_some())
            .unwrap_or_else(|| self.catalog.reference.first().map(|p| p.id.clone()).unwrap_or_default());
        let mut run = None;
        egui::Window::new("Reference")
            .open(&mut open)
            .default_size([620.0, 680.0])
            .default_pos(ctx.content_rect().right_top() + egui::vec2(-660.0, 70.0))
            .resizable(true)
            .collapsible(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let title = self.catalog.reference_page(&current).map_or("", |p| p.title.as_str()).to_string();
                    egui::ComboBox::from_id_salt("ref-overlay-page").selected_text(title).width(260.0).show_ui(
                        ui,
                        |ui| {
                            for p in &self.catalog.reference {
                                if ui.selectable_label(p.id == current, &p.title).clicked() {
                                    self.reference.page = Some(p.id.clone());
                                    self.reference.scroll_to = None;
                                }
                            }
                        },
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.reference.filter)
                            .hint_text("Find in page")
                            .desired_width(160.0),
                    );
                    let needle = self.reference.filter.trim().to_lowercase();
                    if !needle.is_empty() && ui.button("Go").clicked() {
                        // The first page (current first) with a section that mentions it.
                        let pages =
                            std::iter::once(current.clone()).chain(self.catalog.reference.iter().map(|p| p.id.clone()));
                        for id in pages {
                            if let Some(p) = self.catalog.reference_page(&id)
                                && let Some(s) = section_with(&p.body, &needle)
                            {
                                self.reference.page = Some(id);
                                self.reference.scroll_to = Some(s);
                                break;
                            }
                        }
                    }
                });
                ui.separator();
                let Some(page) =
                    self.catalog.reference_page(self.reference.page.as_deref().unwrap_or(current.as_str()))
                else {
                    return;
                };
                let page = page.clone();
                let target = self.reference.scroll_to.take();
                egui::ScrollArea::vertical().id_salt(("ref-overlay", page.id.clone())).show(ui, |ui| {
                    if let Some(Action::Run(cmd)) =
                        markdown::show_scrolled(ui, &t, &page.body, &format!("ov-{}", page.id), target)
                    {
                        run = Some(cmd);
                    }
                });
            });
        self.reference.overlay = open;
        if let Some(cmd) = run {
            self.run_in_terminal(&cmd);
        }
    }
}

/// The index of the first `##`/`###` section whose text mentions `needle` (lowercase).
fn section_with(body: &str, needle: &str) -> Option<usize> {
    let mut section: Option<usize> = None;
    let mut in_code = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
        }
        if !in_code && (line.starts_with("## ") || line.starts_with("### ")) {
            section = Some(section.map_or(0, |s| s + 1));
        }
        if line.to_lowercase().contains(needle) {
            return section;
        }
    }
    None
}
