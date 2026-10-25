//! One file per page; each adds a `fn …_page(&mut self, ui)` to [`crate::App`].

mod cluster;
mod docker;
mod exams;
mod home;
mod learn;
mod progress;
mod setup;

use egui::RichText;

use crate::theme::Tokens;

pub fn title(ui: &mut egui::Ui, t: &Tokens, text: &str, subtitle: &str) {
    ui.label(RichText::new(text).size(26.0).strong().color(if t.dark {
        egui::Color32::WHITE
    } else {
        egui::Color32::BLACK
    }));
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).color(t.muted));
    }
    ui.add_space(10.0);
}

pub fn accent_button(ui: &mut egui::Ui, t: &Tokens, text: &str) -> egui::Response {
    let fg = if t.dark { egui::Color32::from_rgb(4, 30, 28) } else { egui::Color32::WHITE };
    ui.add(egui::Button::new(RichText::new(text).strong().color(fg)).fill(t.accent).min_size(egui::vec2(0.0, 32.0)))
}

pub fn domain_title(id: &str) -> &'static str {
    poddojo_core::DOMAINS.iter().find(|d| d.id == id).map_or("", |d| d.title)
}

pub fn ago(ts: i64) -> String {
    let secs = (poddojo_core::now() - ts).max(0);
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", secs / 60),
        3600..86400 => format!("{} h ago", secs / 3600),
        _ => format!("{} days ago", secs / 86400),
    }
}

/// Two columns with a fixed-width left side, laid out from exact rectangles (no panel sizing
/// memory, so the right side always gets exactly the width that is left).
/// `draw` is called twice: with `true` for the left column, then `false` for the right one.
pub fn split_left(ui: &mut egui::Ui, left_width: f32, mut draw: impl FnMut(&mut egui::Ui, bool)) {
    let total = ui.available_rect_before_wrap();
    let gap = 16.0;
    let lw = left_width.min(total.width() * 0.45);
    let left_rect = egui::Rect::from_min_size(total.min, egui::vec2(lw, total.height()));
    let right_rect = egui::Rect::from_min_max(egui::pos2(total.min.x + lw + gap, total.min.y), total.max);
    ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
        ui.set_clip_rect(left_rect.intersect(ui.clip_rect()));
        ui.set_width(lw);
        draw(ui, true);
    });
    ui.scope_builder(egui::UiBuilder::new().max_rect(right_rect), |ui| {
        ui.set_clip_rect(right_rect.intersect(ui.clip_rect()));
        draw(ui, false);
    });
    ui.allocate_rect(total, egui::Sense::hover());
}
