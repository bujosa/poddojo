//! PodDojo's icons, drawn in code from simple primitives (original, no icon font or emoji, so
//! they look the same on every platform and in both themes).

use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2, pos2, vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Home,
    Book,
    Container,
    Cluster,
    Stopwatch,
    Chart,
    Gear,
    Terminal,
    Console,
    Server,
    Check,
    Play,
    Refresh,
    Flag,
    Search,
    Repeat,
    Docs,
}

/// Paint `icon` in `rect` (square) with `color`.
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let s = rect.width().min(rect.height());
    let c = rect.center();
    // Coordinates below are on a 24×24 grid centred on the rect.
    let p = |x: f32, y: f32| -> Pos2 { pos2(c.x + (x - 12.0) * s / 24.0, c.y + (y - 12.0) * s / 24.0) };
    let w = (s / 12.0).max(1.2);
    let st = Stroke::new(w, color);
    let line = |a: Pos2, b: Pos2| painter.line_segment([a, b], st);
    let poly = |pts: Vec<Pos2>| painter.add(egui::Shape::closed_line(pts, st));
    let rrect = |r: Rect, rad: f32| painter.rect_stroke(r, rad * s / 24.0, st, egui::StrokeKind::Middle);
    match icon {
        Icon::Home => {
            poly(vec![p(3.5, 11.0), p(12.0, 4.0), p(20.5, 11.0), p(20.5, 20.0), p(3.5, 20.0)]);
            rrect(Rect::from_min_max(p(9.5, 14.0), p(14.5, 20.0)), 1.0);
        }
        Icon::Book => {
            poly(vec![p(12.0, 6.5), p(3.0, 5.0), p(3.0, 18.5), p(12.0, 20.0)]);
            poly(vec![p(12.0, 6.5), p(21.0, 5.0), p(21.0, 18.5), p(12.0, 20.0)]);
            line(p(12.0, 6.5), p(12.0, 20.0));
        }
        Icon::Container => {
            rrect(Rect::from_min_max(p(3.0, 6.0), p(21.0, 18.0)), 1.5);
            for x in [7.5, 12.0, 16.5] {
                line(p(x, 9.0), p(x, 15.0));
            }
        }
        Icon::Cluster => {
            let nodes = [p(12.0, 4.5), p(4.5, 18.0), p(19.5, 18.0)];
            line(nodes[0], nodes[1]);
            line(nodes[1], nodes[2]);
            line(nodes[2], nodes[0]);
            for n in nodes {
                painter.circle(n, s * 3.0 / 24.0, Color32::TRANSPARENT, st);
                painter.circle_filled(n, s * 1.6 / 24.0, color);
            }
        }
        Icon::Stopwatch => {
            painter.circle_stroke(p(12.0, 13.5), s * 7.5 / 24.0, st);
            line(p(12.0, 13.5), p(12.0, 9.0));
            line(p(12.0, 13.5), p(15.0, 15.5));
            line(p(10.0, 3.5), p(14.0, 3.5));
            line(p(12.0, 3.5), p(12.0, 6.0));
            line(p(18.0, 6.5), p(19.5, 5.0));
        }
        Icon::Chart => {
            line(p(3.5, 20.5), p(20.5, 20.5));
            for (x, top) in [(7.0, 14.0), (12.0, 9.0), (17.0, 5.0)] {
                rrect(Rect::from_min_max(p(x - 1.8, top), p(x + 1.8, 20.5)), 0.5);
            }
        }
        Icon::Gear => {
            painter.circle_stroke(c, s * 5.0 / 24.0, st);
            painter.circle_stroke(c, s * 2.2 / 24.0, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let d = vec2(a.cos(), a.sin());
                painter.line_segment([c + d * (s * 5.0 / 24.0), c + d * (s * 8.5 / 24.0)], Stroke::new(w * 1.6, color));
            }
        }
        Icon::Terminal => {
            rrect(Rect::from_min_max(p(3.0, 4.5), p(21.0, 19.5)), 2.0);
            line(p(7.0, 9.5), p(10.0, 12.0));
            line(p(10.0, 12.0), p(7.0, 14.5));
            line(p(12.0, 15.0), p(17.0, 15.0));
        }
        Icon::Console => {
            for (y, x2) in [(7.0, 20.0), (12.0, 15.0), (17.0, 18.0)] {
                line(p(4.0, y), p(x2, y));
            }
        }
        Icon::Server => {
            rrect(Rect::from_min_max(p(3.5, 4.5), p(20.5, 11.0)), 1.5);
            rrect(Rect::from_min_max(p(3.5, 13.0), p(20.5, 19.5)), 1.5);
            painter.circle_filled(p(7.0, 7.75), s * 1.1 / 24.0, color);
            painter.circle_filled(p(7.0, 16.25), s * 1.1 / 24.0, color);
        }
        Icon::Check => {
            painter
                .add(egui::Shape::line(vec![p(5.0, 12.5), p(10.0, 17.5), p(19.0, 7.0)], Stroke::new(w * 1.3, color)));
        }
        Icon::Play => {
            painter.add(egui::Shape::convex_polygon(
                vec![p(7.0, 5.0), p(19.0, 12.0), p(7.0, 19.0)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Refresh => {
            let r = s * 7.0 / 24.0;
            let pts: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let a = 0.6 + i as f32 / 20.0 * 4.9;
                    c + vec2(a.cos(), a.sin()) * r
                })
                .collect();
            if let Some(&end) = pts.last() {
                painter.add(egui::Shape::line(pts, st));
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        end + vec2(-0.5, -3.5) * (s / 24.0),
                        end + vec2(3.8, 1.2) * (s / 24.0),
                        end + vec2(-2.5, 2.6) * (s / 24.0),
                    ],
                    color,
                    Stroke::NONE,
                ));
            }
        }
        Icon::Flag => {
            line(p(5.0, 3.5), p(5.0, 21.0));
            poly(vec![p(5.0, 4.0), p(19.0, 4.0), p(15.5, 8.5), p(19.0, 13.0), p(5.0, 13.0)]);
        }
        Icon::Search => {
            painter.circle_stroke(p(10.5, 10.5), s * 6.0 / 24.0, st);
            line(p(15.0, 15.0), p(20.0, 20.0));
        }
        Icon::Repeat => {
            // Two arrows chasing each other round a loop.
            let r = s * 7.0 / 24.0;
            for (from, to) in [(0.35f32, 2.75f32), (3.5, 5.9)] {
                let pts: Vec<Pos2> = (0..=12)
                    .map(|i| {
                        let a = from + (to - from) * i as f32 / 12.0;
                        c + vec2(a.cos(), a.sin()) * r
                    })
                    .collect();
                let end = pts[pts.len() - 1];
                let dir = vec2(-(to.sin()), to.cos());
                let side = vec2(to.cos(), to.sin());
                painter.add(egui::Shape::line(pts, st));
                painter.add(egui::Shape::convex_polygon(
                    vec![end + dir * (s * 3.0 / 24.0), end + side * (s * 2.6 / 24.0), end - side * (s * 2.6 / 24.0)],
                    color,
                    Stroke::NONE,
                ));
            }
        }
        Icon::Docs => {
            poly(vec![p(5.0, 3.0), p(15.0, 3.0), p(19.0, 7.0), p(19.0, 21.0), p(5.0, 21.0)]);
            line(p(15.0, 3.0), p(15.0, 7.0));
            line(p(15.0, 7.0), p(19.0, 7.0));
            for y in [11.0, 14.5, 18.0] {
                line(p(8.0, y), p(16.0, y));
            }
        }
    }
}

/// An icon as a widget of `size` points.
pub fn show(ui: &mut egui::Ui, icon: Icon, size: f32, color: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint(ui.painter(), rect, icon, color);
    resp
}

/// A button with an icon and a label, drawn like egui's buttons.
pub fn button(ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, ui.visuals().text_color());
    let pad = ui.spacing().button_padding;
    let icon_size = 15.0;
    let gap = if label.is_empty() { 0.0 } else { 7.0 };
    let size = vec2(pad.x * 2.0 + icon_size + gap + galley.size().x, (galley.size().y + pad.y * 2.0).max(28.0));
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label));
    if ui.is_rect_visible(rect) {
        let v = ui.style().interact(&resp);
        ui.painter().rect(rect, v.corner_radius, v.weak_bg_fill, v.bg_stroke, egui::StrokeKind::Inside);
        let icon_rect =
            Rect::from_center_size(pos2(rect.min.x + pad.x + icon_size / 2.0, rect.center().y), Vec2::splat(icon_size));
        paint(ui.painter(), icon_rect, icon, v.text_color());
        let text_pos = pos2(icon_rect.max.x + gap, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(text_pos, galley, v.text_color());
    }
    resp
}
