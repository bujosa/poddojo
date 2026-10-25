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
    v.widgets.inactive.bg_fill = t.card_hover;
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
