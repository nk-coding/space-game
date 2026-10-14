//! With the `embed` feature, generates `$OUT_DIR/base_pack.rs`: a list of `(path, contents)` pairs for every
//! `.ron` and `.svg` file of `content/base` except `wiki/` (wiki prose is for tools, never loaded by the game).
//!
//! Staleness: `include_str!` makes rustc track the contents of every listed file, but the *list* only changes
//! when this script reruns, and cargo decides that by mtimes. A moved file keeps its old mtime, so the script
//! names every directory and every embedded file in `rerun-if-changed`: a path that no longer exists always
//! reruns it (moves, renames, deletions), and a directory's own mtime changes whenever an entry is added,
//! removed or renamed in it. The test `embedded_pack_matches_disk` (tests/base_pack.rs) is the safety net.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("base_pack.rs");
    let mut code = String::from("pub static BASE_PACK_FILES: &[(&str, &str)] = &[\n");
    if std::env::var_os("CARGO_FEATURE_EMBED").is_some() {
        let root =
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../../content/base");
        let root = root
            .canonicalize()
            .expect("content/base must exist next to crates/");
        let mut files = Vec::new();
        let mut dirs = vec![root.clone()];
        collect(&root, &root, &mut files, &mut dirs);
        files.sort();
        for dir in &dirs {
            println!("cargo::rerun-if-changed={}", dir.display());
        }
        for (rel, abs) in files {
            println!("cargo::rerun-if-changed={}", abs.display());
            writeln!(
                code,
                "    ({rel:?}, include_str!({:?})),",
                abs.display().to_string()
            )
            .unwrap();
        }
    }
    code.push_str("];\n");
    // Rewriting an unchanged list would only recompile the crate.
    if std::fs::read_to_string(&out).ok().as_deref() != Some(code.as_str()) {
        std::fs::write(out, code).unwrap();
    }
}

/// Embedded files `(path relative to the pack, absolute path)` and every directory walked (wiki included, so
/// a file moved between `wiki/` and the rest is noticed too).
fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>, dirs: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        let rel: Vec<String> = path
            .strip_prefix(root)
            .unwrap()
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        if rel.iter().any(|c| c.starts_with('.')) {
            continue;
        }
        let wiki = rel.first().map(String::as_str) == Some("wiki");
        if path.is_dir() {
            dirs.push(path.clone());
            collect(root, &path, out, dirs);
        } else if !wiki && path.extension().is_some_and(|e| e == "ron" || e == "svg") {
            out.push((rel.join("/"), path));
        }
    }
}
