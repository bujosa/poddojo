//! PodDojo desktop app: a native egui window (glow renderer, no web view).
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

struct Shell(poddojo_ui::App);

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.frame(ui);
    }
}

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("PodDojo")
        .with_app_id("dev.bujosa.poddojo")
        .with_inner_size([1360.0, 880.0])
        .with_min_inner_size([960.0, 600.0]);
    if let Some((rgba, width, height)) = poddojo_ui::icon_rgba() {
        viewport = viewport.with_icon(egui::IconData { rgba, width, height });
    }
    let options = eframe::NativeOptions { viewport, renderer: eframe::Renderer::Glow, ..Default::default() };
    eframe::run_native("PodDojo", options, Box::new(|cc| Ok(Box::new(Shell(poddojo_ui::App::new(&cc.egui_ctx))))))
}
