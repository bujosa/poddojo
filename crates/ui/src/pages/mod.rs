//! One file per page; each adds a `fn …_page(&mut self, ui)` to [`crate::App`].

mod cluster;
mod docker;
mod exams;
mod home;
mod learn;
mod progress;
mod reference;
mod review;
mod setup;
mod welcome;

use egui::RichText;

use crate::theme::Tokens;

pub use reference::ReferenceState;
pub use review::ReviewState;

/// Headline text: white on dark, black on light.
pub fn strong_text(t: &Tokens) -> egui::Color32 {
    if t.dark { egui::Color32::WHITE } else { egui::Color32::BLACK }
}

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
        86400..172800 => "yesterday".into(),
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

/// Workspace minimums: instructions stay readable, the terminal keeps ~70 columns.
pub const INSTRUCTIONS_MIN: f32 = 420.0;
pub const TERMINAL_MIN: f32 = 560.0;
/// Below this window width the exam's task list folds into a drawer.
pub const DRAWER_BELOW: f32 = 1200.0;

/// Space between panes; the divider's grab area.
pub const DIVIDER: f32 = 14.0;

/// Two panes side by side with a draggable divider. `left` is the left pane's width (0: `default`);
/// it stays at least `min_left` wide and leaves `min_right` for the right pane when the space allows
/// (otherwise the space is shared in proportion). Double-click the divider for the default width.
/// Returns both rectangles and whether the learner just finished dragging (time to save `left`).
pub fn resizable_split(
    ui: &mut egui::Ui,
    t: &Tokens,
    id: &str,
    total: egui::Rect,
    left: &mut f32,
    default: f32,
    (min_left, min_right): (f32, f32),
) -> (egui::Rect, egui::Rect, bool) {
    let room = (total.width() - DIVIDER).max(0.0);
    let (lo, hi) = if room >= min_left + min_right {
        (min_left, room - min_right)
    } else {
        let share = room * min_left / (min_left + min_right);
        (share, share)
    };
    let mut width = if *left > 0.0 { *left } else { default }.clamp(lo, hi);
    let handle =
        egui::Rect::from_min_size(egui::pos2(total.min.x + width, total.min.y), egui::vec2(DIVIDER, total.height()));
    let resp = ui
        .interact(handle, ui.id().with(("divider", id)), egui::Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
        .on_hover_text("Drag to resize · double-click to reset");
    if resp.dragged() {
        width = (width + resp.drag_delta().x).clamp(lo, hi);
        *left = width;
    }
    let mut save = resp.drag_stopped();
    if resp.double_clicked() {
        width = default.clamp(lo, hi);
        *left = 0.0;
        save = true;
    }
    // A hairline that lights up with a grip while hovered or dragged.
    let active = resp.hovered() || resp.dragged();
    let x = total.min.x + width + DIVIDER / 2.0;
    let line = if active { t.accent } else { t.border };
    ui.painter().vline(x, total.y_range().shrink(6.0), egui::Stroke::new(if active { 2.0 } else { 1.0 }, line));
    if active {
        let c = egui::pos2(x, total.center().y);
        ui.painter().rect_filled(egui::Rect::from_center_size(c, egui::vec2(6.0, 36.0)), 3.0, t.accent);
    }
    let left_rect = egui::Rect::from_min_size(total.min, egui::vec2(width, total.height()));
    let right_rect = egui::Rect::from_min_max(egui::pos2(total.min.x + width + DIVIDER, total.min.y), total.max);
    (left_rect, right_rect, save)
}
