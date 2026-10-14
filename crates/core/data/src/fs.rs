//! Loading packs from directories (feature `fs`, native tools only).

use std::path::Path;

use crate::content::load::Pack;
use crate::content::problem::Problem;

/// Reads every `.ron`, `.md` and `.svg` file below `dir` as `(relative path with '/', contents)`, sorted by
/// path.
/// Hidden files and directories (starting with '.') are skipped, and so are links to directories (a link to a
/// parent would never end).
pub fn read_pack_files(dir: &Path) -> std::io::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        // `file_type` does not follow links: a linked directory is neither `is_dir` nor read below.
        if entry.file_type()?.is_dir() {
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

#[cfg(all(test, unix))]
mod tests {
    /// A mod folder with a link back to itself loads instead of recursing until the stack runs out.
    #[test]
    fn links_to_directories_are_skipped() {
        let dir = std::env::temp_dir().join(format!("data-fs-loop-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("factions")).unwrap();
        std::fs::write(
            dir.join("pack.ron"),
            r#"(id: "m", name: "M", version: "1", schema_version: 1)"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("factions/x.ron"),
            r#"(id: "x", name: "X", summary: "X.", glow: "enemy")"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(&dir, dir.join("factions/again")).unwrap();
        let pack = super::load_pack_dir(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(pack.unwrap().factions.len(), 1);
    }
}
