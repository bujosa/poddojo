//! Repository tasks: `cargo xtask ci` (every gate) and `cargo xtask icon` (render the app icon).

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("ci") => ci(),
        Some("icon") => icon(),
        _ => Err("usage: cargo xtask <ci|icon>".to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

fn step(name: &str, program: &str, args: &[&str]) -> Result<(), String> {
    eprintln!("── {name}");
    let status = Command::new(program).args(args).status().map_err(|e| format!("{name}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{name} failed")) }
}

fn ci() -> Result<(), String> {
    step("fmt", "cargo", &["fmt", "--all", "--check"])?;
    step("clippy", "cargo", &["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])?;
    step("test", "cargo", &["test", "--workspace", "-q"])?;
    step("content", "cargo", &["run", "-q", "-p", "dojo", "--", "validate", "content"])?;
    eprintln!("── all gates passed");
    Ok(())
}

fn icon() -> Result<(), String> {
    let svg = std::fs::read("assets/app-icon/poddojo.svg").map_err(|e| e.to_string())?;
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default()).map_err(|e| e.to_string())?;
    for size in [16u32, 32, 64, 128, 256, 512, 1024] {
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).ok_or("bad size")?;
        let scale = size as f32 / tree.size().width();
        resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
        let path = format!("assets/app-icon/poddojo-{size}.png");
        pixmap.save_png(&path).map_err(|e| e.to_string())?;
        eprintln!("wrote {path}");
    }
    Ok(())
}
