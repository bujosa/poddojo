//! Repository tasks: `cargo xtask ci` (every gate), `cargo xtask icon` (render the app icon) and
//! `cargo xtask bundle [--install]` (build PodDojo.app for macOS; `--install` copies it to
//! ~/Applications so it opens from the Dock and Spotlight).

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("ci") => ci(),
        Some("icon") => icon(),
        Some("bundle") => bundle(args.iter().any(|a| a == "--install")),
        _ => Err("usage: cargo xtask <ci|icon|bundle [--install]>".to_string()),
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

const INFO_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>PodDojo</string>
  <key>CFBundleDisplayName</key><string>PodDojo</string>
  <key>CFBundleIdentifier</key><string>dev.bujosa.poddojo</string>
  <key>CFBundleVersion</key><string>VERSION</string>
  <key>CFBundleShortVersionString</key><string>VERSION</string>
  <key>CFBundleExecutable</key><string>poddojo</string>
  <key>CFBundleIconFile</key><string>PodDojo</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.education</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
"#;

fn bundle(install: bool) -> Result<(), String> {
    step("build", "cargo", &["build", "--release", "-p", "poddojo", "-p", "dojo"])?;
    let app = std::path::Path::new("target/bundle/PodDojo.app");
    let contents = app.join("Contents");
    let _ = std::fs::remove_dir_all(app);
    for dir in [contents.join("MacOS"), contents.join("Resources")] {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    let version = env!("CARGO_PKG_VERSION");
    std::fs::write(contents.join("Info.plist"), INFO_PLIST.replace("VERSION", version)).map_err(|e| e.to_string())?;
    for bin in ["poddojo", "dojo"] {
        std::fs::copy(format!("target/release/{bin}"), contents.join("MacOS").join(bin))
            .map_err(|e| format!("{bin}: {e}"))?;
    }
    // iconutil wants an .iconset folder with these exact names.
    let iconset = std::path::Path::new("target/bundle/PodDojo.iconset");
    let _ = std::fs::remove_dir_all(iconset);
    std::fs::create_dir_all(iconset).map_err(|e| e.to_string())?;
    for (name, size) in [
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ] {
        std::fs::copy(format!("assets/app-icon/poddojo-{size}.png"), iconset.join(name)).map_err(|e| e.to_string())?;
    }
    let icns = contents.join("Resources/PodDojo.icns");
    step("icns", "iconutil", &["-c", "icns", &iconset.display().to_string(), "-o", &icns.display().to_string()])?;
    eprintln!("built {}", app.display());
    if install {
        let home = std::env::var("HOME").map_err(|e| e.to_string())?;
        let dest = std::path::Path::new(&home).join("Applications/PodDojo.app");
        if dest.exists() {
            // Only ever replaces a previous PodDojo.app.
            std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(dest.parent().ok_or("no parent")?).map_err(|e| e.to_string())?;
        step("install", "cp", &["-R", &app.display().to_string(), &dest.display().to_string()])?;
        eprintln!("installed {} (open it from Spotlight or the Dock)", dest.display());
        eprintln!("the CLI is inside it: ln -sf \"{}/Contents/MacOS/dojo\" /usr/local/bin/dojo", dest.display());
    }
    Ok(())
}
