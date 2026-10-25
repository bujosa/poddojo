//! The app mark, decoded from the PNG rendered by `cargo xtask icon` (assets/app-icon).

static PNG: &[u8] = include_bytes!("../../../assets/app-icon/poddojo-128.png");

pub fn texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory_with_format(PNG, image::ImageFormat::Png).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture("poddojo-logo", color, egui::TextureOptions::LINEAR))
}

/// RGBA pixels for the window icon.
pub fn icon_rgba() -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory_with_format(PNG, image::ImageFormat::Png).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    Some((img.into_raw(), w, h))
}
