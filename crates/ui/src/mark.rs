//! The PodDojo mark, drawn live: a pod of containers (teal cubes) with the learner (the belted
//! amber cube) on top. It breathes, and packets travel between the nodes like traffic in a
//! cluster. `energy` (0 = idle, 1 = busy) speeds it up and adds a glow, so the mark doubles as the
//! app's activity indicator. Same geometry as `assets/app-icon/poddojo.svg`.

use egui::{Color32, Pos2, Rect, Shape, Stroke, pos2, vec2};

const TEAL: [Color32; 3] =
    [Color32::from_rgb(153, 246, 228), Color32::from_rgb(45, 212, 191), Color32::from_rgb(15, 118, 110)];
const AMBER: [Color32; 3] =
    [Color32::from_rgb(253, 230, 138), Color32::from_rgb(251, 191, 36), Color32::from_rgb(217, 119, 6)];
const INK: Color32 = Color32::from_rgb(11, 16, 38);
const BELT: Color32 = Color32::from_rgb(30, 27, 46);

/// One isometric cube: top-face centre `c` (SVG units), drawn left, right, then top.
fn cube(painter: &egui::Painter, map: &impl Fn(f32, f32) -> Pos2, c: (f32, f32), colors: [Color32; 3], ink: Stroke) {
    let (w, h, s) = (130.0, 75.0, 150.0);
    let (x, y) = c;
    let left = vec![map(x - w, y), map(x, y + h), map(x, y + h + s), map(x - w, y + s)];
    let right = vec![map(x + w, y), map(x, y + h), map(x, y + h + s), map(x + w, y + s)];
    let top = vec![map(x, y - h), map(x + w, y), map(x, y + h), map(x - w, y)];
    painter.add(Shape::convex_polygon(left, colors[1], ink));
    painter.add(Shape::convex_polygon(right, colors[2], ink));
    painter.add(Shape::convex_polygon(top, colors[0], ink));
}

/// Paint the mark in `rect` at `time` seconds.
pub fn paint(painter: &egui::Painter, rect: Rect, time: f64, energy: f32, tile: bool) {
    let size = rect.width().min(rect.height());
    let k = size / 1024.0;
    let origin = rect.center() - vec2(512.0, 512.0) * k;
    let map = |x: f32, y: f32| pos2(origin.x + x * k, origin.y + y * k);
    let speed = 1.0 + 2.2 * energy as f64;
    let t = time * speed;
    if tile {
        painter.rect_filled(rect, size * 0.22, Color32::from_rgb(19, 24, 64));
        let glow = 0.18 + 0.25 * energy + 0.06 * (t * 1.3).sin() as f32;
        painter.circle_filled(
            map(512.0, 450.0),
            size * 0.42,
            Color32::from_rgb(45, 212, 191).gamma_multiply(glow.clamp(0.0, 1.0) * 0.35),
        );
    } else if energy > 0.01 {
        painter.circle_filled(
            rect.center(),
            size * 0.55,
            Color32::from_rgb(45, 212, 191).gamma_multiply(0.12 * energy),
        );
    }
    let ink = Stroke::new((6.0 * k).max(0.6), INK);
    // Breathing: each node bobs a little, out of phase.
    let bob = |phase: f64| ((t * 1.7 + phase).sin() as f32) * 10.0 * (0.6 + 0.6 * energy);
    let nodes =
        [(512.0, 445.0 + bob(0.0)), (382.0, 520.0 + bob(1.3)), (642.0, 520.0 + bob(2.6)), (512.0, 595.0 + bob(3.9))];
    for &n in &nodes {
        cube(painter, &map, n, TEAL, ink);
    }
    // Packets: small lights running between the nodes' top faces.
    let links = [(1usize, 3usize), (3, 2), (2, 0), (0, 1)];
    for (i, (a, b)) in links.iter().enumerate() {
        let (Some(pa), Some(pb)) = (nodes.get(*a), nodes.get(*b)) else { continue };
        let f = ((t * 0.45 + i as f64 * 0.25).fract()) as f32;
        let p = map(pa.0 + (pb.0 - pa.0) * f, pa.1 + (pb.1 - pa.1) * f);
        let alpha = (f * std::f32::consts::PI).sin();
        painter.circle_filled(
            p,
            (13.0 * k).max(1.0),
            Color32::WHITE.gamma_multiply(0.55 * alpha * (0.6 + 0.4 * energy)),
        );
    }
    // The learner floats above the pod, wearing the belt.
    let lift = ((t * 1.1).sin() as f32 + 1.0) * 14.0 * (0.7 + 0.5 * energy);
    let a = (512.0, 295.0 + nodes.first().map_or(0.0, |n| n.1 - 445.0) - lift);
    cube(painter, &map, a, AMBER, ink);
    let (x, y) = a;
    let (w, h) = (130.0, 75.0);
    let (b0, b1) = (63.0, 93.0);
    painter.add(Shape::convex_polygon(
        vec![map(x - w, y + b0), map(x, y + h + b0), map(x, y + h + b1), map(x - w, y + b1)],
        BELT,
        ink,
    ));
    painter.add(Shape::convex_polygon(
        vec![map(x + w, y + b0), map(x, y + h + b0), map(x, y + h + b1), map(x + w, y + b1)],
        BELT,
        ink,
    ));
    painter.rect_filled(
        Rect::from_center_size(map(x, y + h + (b0 + b1) / 2.0), vec2(34.0 * k, 30.0 * k)),
        7.0 * k,
        Color32::from_rgb(42, 38, 64),
    );
}

/// The mark as a widget. Returns the response so callers can react to hover.
pub fn show(ui: &mut egui::Ui, size: f32, energy: f32, tile: bool, motion: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    let hover = if resp.hovered() { 0.5 } else { 0.0 };
    let energy = (energy + hover).clamp(0.0, 1.0);
    let time = if motion { ui.input(|i| i.time) } else { 0.0 };
    paint(ui.painter(), rect, time, energy, tile);
    // Small marks only move while something happens (or on hover); the big welcome mark always
    // breathes. Idle frames cost nothing.
    if motion && (energy > 0.01 || size >= 64.0) {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }
    resp
}

/// A slow field of drifting nodes linked when close: a cluster in the background of the
/// welcome screen. Deterministic (no randomness), cheap (≈40 nodes).
pub fn node_field(painter: &egui::Painter, rect: Rect, time: f64, color: Color32) {
    const N: usize = 42;
    let mut pts: Vec<Pos2> = Vec::with_capacity(N);
    for i in 0..N {
        let f = i as f64;
        let sx = (f * 12.9898).sin() * 43758.5453;
        let sy = (f * 78.233).sin() * 12543.1234;
        let bx = sx.fract().abs() as f32;
        let by = sy.fract().abs() as f32;
        let dx = ((time * 0.05 + f * 0.7).sin() * 0.035) as f32;
        let dy = ((time * 0.04 + f * 1.3).cos() * 0.035) as f32;
        pts.push(pos2(
            rect.min.x + (bx + dx).rem_euclid(1.0) * rect.width(),
            rect.min.y + (by + dy).rem_euclid(1.0) * rect.height(),
        ));
    }
    let max = rect.width().min(rect.height()) * 0.17;
    for (i, a) in pts.iter().enumerate() {
        for b in pts.iter().skip(i + 1) {
            let d = a.distance(*b);
            if d < max {
                painter.line_segment([*a, *b], Stroke::new(1.0, color.gamma_multiply(0.22 * (1.0 - d / max))));
            }
        }
    }
    for (i, p) in pts.iter().enumerate() {
        let pulse = 0.5 + 0.5 * ((time * 1.2 + i as f64).sin() as f32);
        painter.circle_filled(*p, 2.0 + 1.2 * pulse, color.gamma_multiply(0.25 + 0.3 * pulse));
    }
}
