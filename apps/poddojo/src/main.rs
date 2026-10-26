//! PodDojo desktop app: a native egui window (glow renderer, no web view).
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

struct Shell(poddojo_ui::App);

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.frame(ui);
    }
}

fn main() -> eframe::Result {
    // Development aid for screenshots: `PODDOJO_WINDOW=1100x760`.
    let size = std::env::var("PODDOJO_WINDOW")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some([w.trim().parse::<f32>().ok()?, h.trim().parse::<f32>().ok()?])
        })
        .unwrap_or([1360.0, 880.0]);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("PodDojo")
        .with_app_id("dev.bujosa.poddojo")
        .with_inner_size(size)
        .with_min_inner_size([960.0, 600.0]);
    if let Some((rgba, width, height)) = poddojo_ui::icon_rgba() {
        viewport = viewport.with_icon(egui::IconData { rgba, width, height });
    }
    let options = eframe::NativeOptions { viewport, renderer: eframe::Renderer::Glow, ..Default::default() };
    eframe::run_native("PodDojo", options, Box::new(|cc| Ok(Box::new(Shell(poddojo_ui::App::new(&cc.egui_ctx))))))
}
