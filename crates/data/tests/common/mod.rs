#![allow(dead_code)]

use std::path::{Path, PathBuf};

use data::{Content, Pack, Problem};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// All `.ron`, `.md` and `.svg` files of a pack directory (tests may touch the filesystem; the crate may not).
pub fn read_dir_files(dir: &Path) -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else if p
                .extension()
                .is_some_and(|x| x == "ron" || x == "md" || x == "svg")
            {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

/// The base pack from disk, including wiki prose.
pub fn base_pack_with_wiki() -> Pack {
    Pack::from_files(read_dir_files(&repo_root().join("content/base")))
        .unwrap_or_else(|p| panic!("{}", show(&p)))
}

pub fn show(problems: &[Problem]) -> String {
    problems
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn content() -> Content {
    data::try_base_content().unwrap_or_else(|p| panic!("{}", show(&p)))
}
