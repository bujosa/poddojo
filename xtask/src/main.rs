//! Repository tasks: `cargo xtask ci` (every gate), `cargo xtask icon` (render the app icon),
//! `cargo xtask bundle [--install]` (build and sign PodDojo.app for macOS; `--install` copies it to
//! ~/Applications so it opens from the Dock and Spotlight) and `cargo xtask release` (gates, bundle,
//! zip and checksums in `target/release-assets`, ready to upload).
//!
//! Signing: ad-hoc by default. With `PODDOJO_SIGN_IDENTITY="Developer ID Application: …"` the app is
//! signed with the hardened runtime, and with `PODDOJO_NOTARY_PROFILE=<notarytool keychain profile>`
//! it is also notarised and stapled, so it opens without any Gatekeeper warning.

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("ci") => ci(),
        Some("icon") => icon(),
        Some("bundle") => bundle(args.iter().any(|a| a == "--install")),
        Some("release") => release(),
        _ => Err("usage: cargo xtask <ci|icon|bundle [--install]|release>".to_string()),
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
    sign(app)?;
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

/// Sign the bundle: inner binaries first, then the app. Ad-hoc unless a Developer ID is configured.
fn sign(app: &std::path::Path) -> Result<(), String> {
    let identity = std::env::var("PODDOJO_SIGN_IDENTITY").ok().filter(|s| !s.trim().is_empty());
    let id = identity.as_deref().unwrap_or("-");
    let mut base = vec!["--force", "--sign", id];
    if identity.is_some() {
        base.extend(["--options", "runtime", "--timestamp"]);
    }
    for bin in ["dojo", "poddojo"] {
        let path = app.join("Contents/MacOS").join(bin).display().to_string();
        let mut args = base.clone();
        args.push(&path);
        step(&format!("sign {bin}"), "codesign", &args)?;
    }
    let path = app.display().to_string();
    let mut args = base.clone();
    args.push(&path);
    step("sign app", "codesign", &args)?;
    step("verify signature", "codesign", &["--verify", "--strict", "--deep", &path])?;
    if identity.is_none() {
        eprintln!("signed ad-hoc (set PODDOJO_SIGN_IDENTITY for a Developer ID signature)");
    }
    Ok(())
}

/// Gates, a signed bundle, the zip and its checksum: everything a GitHub release needs.
fn release() -> Result<(), String> {
    ci()?;
    bundle(false)?;
    let version = env!("CARGO_PKG_VERSION");
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        other => other,
    };
    let out = std::path::Path::new("target/release-assets");
    let _ = std::fs::remove_dir_all(out);
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let name = format!("PodDojo-v{version}-macos-{arch}.zip");
    let zip = out.join(&name).display().to_string();
    step("zip", "ditto", &["-c", "-k", "--keepParent", "target/bundle/PodDojo.app", &zip])?;
    if let Ok(profile) = std::env::var("PODDOJO_NOTARY_PROFILE")
        && !profile.trim().is_empty()
    {
        step("notarise", "xcrun", &["notarytool", "submit", &zip, "--keychain-profile", &profile, "--wait"])?;
        step("staple", "xcrun", &["stapler", "staple", "target/bundle/PodDojo.app"])?;
        // Re-zip so the download carries the stapled ticket.
        step("zip (stapled)", "ditto", &["-c", "-k", "--keepParent", "target/bundle/PodDojo.app", &zip])?;
    }
    let sums = Command::new("shasum")
        .args(["-a", "256", &name])
        .current_dir(out)
        .output()
        .map_err(|e| format!("shasum: {e}"))?;
    if !sums.status.success() {
        return Err("shasum failed".into());
    }
    std::fs::write(out.join("SHA256SUMS.txt"), &sums.stdout).map_err(|e| e.to_string())?;
    eprintln!("── release assets in {}:", out.display());
    eprint!("{}", String::from_utf8_lossy(&sums.stdout));
    eprintln!(
        "publish: gh release create v{version} --target main --title \"PodDojo v{version}\" --notes-file <notes> \\\n  {zip} {}/SHA256SUMS.txt",
        out.display()
    );
    Ok(())
}
