//! With the `embed` feature, generates `$OUT_DIR/base_pack.rs`: a list of `(path, contents)` pairs for every
//! `.ron` and `.svg` file of `content/base` except `wiki/` (wiki prose is for tools, never loaded by the game).
//! `include_str!` makes rustc track every file; `rerun-if-changed` on the directory catches added files.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("base_pack.rs");
    let mut code = String::from("pub static BASE_PACK_FILES: &[(&str, &str)] = &[\n");
    if std::env::var_os("CARGO_FEATURE_EMBED").is_some() {
        let root =
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../content/base");
        let root = root
            .canonicalize()
            .expect("content/base must exist next to crates/");
        println!("cargo::rerun-if-changed={}", root.display());
        let mut files = Vec::new();
        collect(&root, &root, &mut files);
        files.sort();
        for (rel, abs) in files {
            writeln!(
                code,
                "    ({rel:?}, include_str!({:?})),",
                abs.display().to_string()
            )
            .unwrap();
        }
    }
    code.push_str("];\n");
    std::fs::write(out, code).unwrap();
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
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
        if rel.first().map(String::as_str) == Some("wiki") || rel.iter().any(|c| c.starts_with('.'))
        {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "ron" || e == "svg") {
            out.push((rel.join("/"), path));
        }
    }
}
