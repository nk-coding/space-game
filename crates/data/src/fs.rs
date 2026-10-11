//! Loading packs from directories (feature `fs`, native tools only).

use std::path::Path;

use crate::load::Pack;
use crate::problem::Problem;

/// Reads every `.ron`, `.md` and `.svg` file below `dir` as `(relative path with '/', contents)`, sorted by
/// path.
/// Hidden files and directories (starting with '.') are skipped.
pub fn read_pack_files(dir: &Path) -> std::io::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            walk(root, &path, out)?;
        } else if path
            .extension()
            .is_some_and(|e| e == "ron" || e == "md" || e == "svg")
        {
            let rel: Vec<String> = path
                .strip_prefix(root)
                .unwrap()
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            out.push((rel.join("/"), std::fs::read_to_string(&path)?));
        }
    }
    Ok(())
}

/// Reads and parses a pack directory (including `wiki/`).
pub fn load_pack_dir(dir: &Path) -> Result<Pack, Vec<Problem>> {
    let files = read_pack_files(dir).map_err(|e| {
        vec![Problem::error(format!(
            "cannot read pack directory {}: {e}",
            dir.display()
        ))]
    })?;
    Pack::from_files(files)
}
