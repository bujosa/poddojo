//! Embeds every file under `content/` so the app ships its curriculum inside the binary.
//! `PODDOJO_CONTENT_DIR` still overrides it at runtime for authoring (docs/content-authoring.md).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if matches!(path.extension().and_then(|e| e.to_str()), Some("md" | "toml")) {
            out.push(path);
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let content = manifest.join("../../content");
    println!("cargo:rerun-if-changed={}", content.display());
    let mut files = Vec::new();
    walk(&content, &mut files);
    files.sort();
    let mut src = String::from("pub static FILES: &[(&str, &str)] = &[\n");
    for f in &files {
        let rel = f.strip_prefix(&content).unwrap_or(f).to_string_lossy().replace('\\', "/");
        let abs = f.canonicalize().unwrap_or_else(|_| f.clone());
        let _ = writeln!(src, "    ({rel:?}, include_str!({:?})),", abs.to_string_lossy());
    }
    src.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("embedded.rs");
    if let Err(e) = std::fs::write(&out, src) {
        println!("cargo:warning=could not write embedded content: {e}");
    }
}
