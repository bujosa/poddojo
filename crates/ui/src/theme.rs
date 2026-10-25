//! Colours and spacing. Every colour the UI paints comes from [`Tokens`].

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Visuals};

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub bg: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub amber: Color32,
    pub good: Color32,
    pub bad: Color32,
    pub warn: Color32,
    pub code_bg: Color32,
    pub code_text: Color32,
    pub dark: bool,
}

impl Tokens {
    pub const DARK: Tokens = Tokens {
        bg: Color32::from_rgb(11, 14, 30),
        panel: Color32::from_rgb(16, 20, 40),
        card: Color32::from_rgb(23, 28, 54),
        card_hover: Color32::from_rgb(31, 37, 70),
        border: Color32::from_rgb(42, 49, 88),
        text: Color32::from_rgb(226, 232, 245),
        muted: Color32::from_rgb(139, 149, 182),
        accent: Color32::from_rgb(45, 212, 191),
        accent_soft: Color32::from_rgb(19, 78, 74),
        amber: Color32::from_rgb(251, 191, 36),
        good: Color32::from_rgb(52, 211, 153),
        bad: Color32::from_rgb(248, 113, 113),
        warn: Color32::from_rgb(251, 191, 36),
        code_bg: Color32::from_rgb(8, 10, 22),
        code_text: Color32::from_rgb(199, 210, 254),
        dark: true,
    };

    pub const LIGHT: Tokens = Tokens {
        bg: Color32::from_rgb(244, 246, 251),
        panel: Color32::from_rgb(235, 238, 247),
        card: Color32::WHITE,
        card_hover: Color32::from_rgb(240, 249, 248),
        border: Color32::from_rgb(214, 219, 233),
        text: Color32::from_rgb(17, 24, 39),
        muted: Color32::from_rgb(98, 108, 135),
        accent: Color32::from_rgb(13, 148, 136),
        accent_soft: Color32::from_rgb(204, 251, 241),
        amber: Color32::from_rgb(217, 119, 6),
        good: Color32::from_rgb(5, 150, 105),
        bad: Color32::from_rgb(220, 38, 38),
        warn: Color32::from_rgb(217, 119, 6),
        code_bg: Color32::from_rgb(15, 23, 42),
        code_text: Color32::from_rgb(226, 232, 240),
        dark: false,
    };

    pub fn get(dark: bool) -> Self {
        if dark { Self::DARK } else { Self::LIGHT }
    }
}

pub fn apply(ctx: &egui::Context, t: &Tokens, zoom: f32) {
    let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = t.panel;
    v.window_fill = t.card;
    v.extreme_bg_color = t.code_bg;
    v.faint_bg_color = t.card;
    v.override_text_color = Some(t.text);
    v.hyperlink_color = t.accent;
    v.selection.bg_fill = t.accent_soft;
    v.selection.stroke = Stroke::new(1.0, t.accent);
    v.window_corner_radius = CornerRadius::same(12);
    v.window_stroke = Stroke::new(1.0, t.border);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(8);
    }
    v.widgets.inactive.weak_bg_fill = t.card_hover;
    // Radio buttons, checkboxes and slider rails draw with bg_fill: keep them clearly visible.
    v.widgets.inactive.bg_fill = if t.dark { Color32::from_rgb(52, 61, 104) } else { Color32::from_rgb(214, 220, 236) };
    v.widgets.inactive.fg_stroke = Stroke::new(1.2, t.text);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, t.border);
    v.widgets.hovered.weak_bg_fill = t.accent_soft;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.accent);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
    let theme = if t.dark { egui::Theme::Dark } else { egui::Theme::Light };
    ctx.set_theme(theme);
    ctx.set_visuals_of(theme, v);
    ctx.set_zoom_factor(zoom.clamp(0.6, 2.0));
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 7.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.window_margin = Margin::same(14);
        s.text_styles.insert(TextStyle::Heading, FontId::new(24.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(14.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Monospace, FontId::new(13.5, FontFamily::Monospace));
        s.text_styles.insert(TextStyle::Small, FontId::new(12.0, FontFamily::Proportional));
    });
}

/// A rounded card.
pub fn card(t: &Tokens) -> egui::Frame {
    egui::Frame::new()
        .fill(t.card)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(16))
}

/// A small coloured pill (status chips, domain tags).
pub fn chip(ui: &mut egui::Ui, text: &str, fg: Color32, bg: Color32) {
    egui::Frame::new().fill(bg).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(8, 2)).show(
        ui,
        |ui| {
            ui.label(egui::RichText::new(text).size(12.0).color(fg));
        },
    );
}

/// A thin progress bar in the accent colour.
pub fn bar(ui: &mut egui::Ui, t: &Tokens, fraction: f32, width: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 8.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(4), t.border);
    let mut fill = rect;
    fill.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    if fill.width() > 0.5 {
        p.rect_filled(fill, CornerRadius::same(4), color);
    }
}

/// A lesson status dot drawn with the painter (no font glyph needed): done = filled, in progress
/// = ring with a centre, not started = ring.
pub fn status_dot(ui: &mut egui::Ui, t: &Tokens, status: poddojo_core::LessonStatus) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
    let c = rect.center();
    let p = ui.painter();
    match status {
        poddojo_core::LessonStatus::Done => {
            p.circle_filled(c, 6.0, t.good);
            p.line_segment([c + egui::vec2(-3.0, 0.2), c + egui::vec2(-0.8, 2.6)], Stroke::new(1.8, t.bg));
            p.line_segment([c + egui::vec2(-0.8, 2.6), c + egui::vec2(3.2, -2.4)], Stroke::new(1.8, t.bg));
        }
        poddojo_core::LessonStatus::InProgress => {
            p.circle_stroke(c, 5.5, Stroke::new(1.6, t.amber));
            p.circle_filled(c, 2.6, t.amber);
        }
        poddojo_core::LessonStatus::NotStarted => {
            p.circle_stroke(c, 5.5, Stroke::new(1.4, t.muted));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceState {
    Idle,
    Picked,
    Correct,
    Wrong,
    /// The right answer the learner did not pick.
    Missed,
}

/// A quiz answer as a full-width card with a letter badge.
pub fn choice(
    ui: &mut egui::Ui,
    t: &Tokens,
    letter: char,
    text: &str,
    state: ChoiceState,
    enabled: bool,
) -> egui::Response {
    let width = ui.available_width();
    let font = egui::FontId::proportional(15.0);
    let galley = ui.painter().layout(text.to_string(), font, t.text, width - 64.0);
    let height = (galley.size().y + 20.0).max(42.0);
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(width, height),
        if enabled { egui::Sense::click() } else { egui::Sense::hover() },
    );
    let hovered = enabled && resp.hovered();
    let (stroke, fill, badge) = match state {
        ChoiceState::Picked => (t.accent, t.accent_soft, t.accent),
        ChoiceState::Correct | ChoiceState::Missed => (t.good, t.good.gamma_multiply(0.14), t.good),
        ChoiceState::Wrong => (t.bad, t.bad.gamma_multiply(0.14), t.bad),
        ChoiceState::Idle if hovered => (t.accent.gamma_multiply(0.7), t.card_hover, t.muted),
        ChoiceState::Idle => (t.border, t.card_hover.gamma_multiply(0.6), t.muted),
    };
    let p = ui.painter();
    p.rect(
        rect,
        10.0,
        fill,
        Stroke::new(if state == ChoiceState::Idle { 1.0 } else { 1.6 }, stroke),
        egui::StrokeKind::Inside,
    );
    let badge_rect =
        egui::Rect::from_center_size(egui::pos2(rect.min.x + 24.0, rect.center().y), egui::vec2(26.0, 26.0));
    if state == ChoiceState::Idle {
        p.rect_stroke(badge_rect, 7.0, Stroke::new(1.2, badge), egui::StrokeKind::Inside);
        p.text(badge_rect.center(), egui::Align2::CENTER_CENTER, letter, egui::FontId::proportional(13.0), badge);
    } else {
        p.rect_filled(badge_rect, 7.0, badge);
        let mark = match state {
            ChoiceState::Correct | ChoiceState::Missed => "✔".to_string(),
            ChoiceState::Wrong => "✖".to_string(),
            _ => letter.to_string(),
        };
        p.text(badge_rect.center(), egui::Align2::CENTER_CENTER, mark, egui::FontId::proportional(13.0), t.bg);
    }
    p.galley(egui::pos2(rect.min.x + 48.0, rect.center().y - galley.size().y / 2.0), galley, t.text);
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// A pill-shaped tab: selected tabs are filled with the accent, finished ones get a check.
pub fn tab(ui: &mut egui::Ui, t: &Tokens, label: &str, selected: bool, done: bool) -> egui::Response {
    let text = if done { format!("✔  {label}") } else { label.to_string() };
    let font = egui::FontId::proportional(14.5);
    let galley = ui.painter().layout_no_wrap(text, font, t.text);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(galley.size().x + 28.0, 34.0), egui::Sense::click());
    let hovered = resp.hovered();
    let (fill, fg) = if selected {
        (t.accent, if t.dark { Color32::from_rgb(4, 30, 28) } else { Color32::WHITE })
    } else if hovered {
        (t.card_hover, t.text)
    } else {
        (Color32::TRANSPARENT, if done { t.good } else { t.muted })
    };
    ui.painter().rect_filled(rect, 17.0, fill);
    ui.painter().galley_with_override_text_color(
        egui::pos2(rect.min.x + 14.0, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A progress ring with a label in the middle.
pub fn ring(ui: &mut egui::Ui, t: &Tokens, fraction: f32, size: f32, label: &str, sub: &str, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let c = rect.center();
    let r = size / 2.0 - 6.0;
    let p = ui.painter();
    p.circle_stroke(c, r, Stroke::new(9.0, t.border));
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.001 {
        let n = (64.0 * f).ceil().max(2.0) as usize;
        let pts: Vec<egui::Pos2> = (0..=n)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * f * i as f32 / n as f32;
                c + egui::vec2(a.cos(), a.sin()) * r
            })
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(9.0, color)));
    }
    p.text(
        c - egui::vec2(0.0, 7.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(size * 0.2),
        t.text,
    );
    p.text(
        c + egui::vec2(0.0, size * 0.13),
        egui::Align2::CENTER_CENTER,
        sub,
        egui::FontId::proportional(12.0),
        t.muted,
    );
}
