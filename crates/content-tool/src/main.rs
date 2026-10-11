//! `content-tool`: validate content packs, export the resolved content as JSON, list entries.
//!
//! ```text
//! content-tool validate [--icons <dir> | --no-icons] [pack_dir...]
//! content-tool export-json <out.json | -> [pack_dir...]
//! content-tool list <kind> [pack_dir...]
//! ```
//!
//! Pack directories are loaded in the order given (later packs override earlier ones by id). Without any,
//! `content/base` is used (relative to the current directory, else to this crate's repo).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use data::geometry::hull_loops;
use data::{Content, Entry, Kind, Pack, Problem, Severity, ValidateOptions, text_key};
use serde_json::{Value, json};

const USAGE: &str = "usage:
  content-tool validate [--icons <dir> | --no-icons] [pack_dir...]
  content-tool export-json <out.json | -> [pack_dir...]
  content-tool list <kind> [pack_dir...]     kind: stat, category, faction, material, part, ship, page

Pack dirs load in order; later packs override earlier ones by id. Default: content/base.
validate checks icon keys against <first pack>/../../art/icons when that directory exists.";

/// Version of the JSON export layout (bump on breaking changes for external readers).
const EXPORT_FORMAT_VERSION: u32 = 1;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("validate") => validate(&args[1..]),
        Some("export-json") => export_json(&args[1..]),
        Some("list") => list(&args[1..]),
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("{msg}");
            }
            ExitCode::FAILURE
        }
    }
}

fn pack_dirs(args: &[String]) -> Vec<PathBuf> {
    if !args.is_empty() {
        return args.iter().map(PathBuf::from).collect();
    }
    let local = PathBuf::from("content/base");
    if local.is_dir() {
        return vec![local];
    }
    vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/base")]
}

/// Loaded packs plus a map pack id → directory, for printing real file paths.
struct Loaded {
    packs: Vec<Pack>,
    dirs: BTreeMap<String, PathBuf>,
}

fn load_packs(dirs: &[PathBuf]) -> Result<Loaded, Vec<(Problem, Option<PathBuf>)>> {
    let mut packs = Vec::new();
    let mut map = BTreeMap::new();
    let mut problems = Vec::new();
    for dir in dirs {
        match data::fs::load_pack_dir(dir) {
            Ok(p) => {
                map.insert(p.manifest.id.clone(), dir.clone());
                packs.push(p);
            }
            Err(ps) => problems.extend(ps.into_iter().map(|p| (p, Some(dir.clone())))),
        }
    }
    if problems.is_empty() {
        Ok(Loaded { packs, dirs: map })
    } else {
        Err(problems)
    }
}

/// `error: content/base/parts/power.ron:3:5: part `x`, field `y`: message`
fn format_problem(p: &Problem, dir: Option<&Path>) -> String {
    let mut s = String::from(match p.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    });
    match (&p.source, dir) {
        (Some(src), Some(dir)) => s += &format!(": {}", dir.join(&src.file).display()),
        (Some(src), None) => s += &format!(": {src}"),
        (None, Some(dir)) => s += &format!(": {}", dir.display()),
        (None, None) => {}
    }
    if let Some((l, c)) = p.line {
        s += &format!(":{l}:{c}");
    }
    if let (Some(k), Some(id)) = (p.kind, &p.id) {
        s += &format!(": {k} `{id}`");
    }
    if let Some(f) = &p.field {
        s += &format!(", field `{f}`");
    }
    s + ": " + &p.message
}

fn report(problems: &[(Problem, Option<PathBuf>)]) -> (usize, usize) {
    for (p, dir) in problems {
        eprintln!("{}", format_problem(p, dir.as_deref()));
    }
    let errors = problems.iter().filter(|(p, _)| p.is_error()).count();
    (errors, problems.len() - errors)
}

/// Loads, merges and validates; prints every problem. `Err` if there are errors.
fn load_content(dirs: &[PathBuf], opts: &ValidateOptions) -> Result<(Content, usize), String> {
    let loaded = match load_packs(dirs) {
        Ok(l) => l,
        Err(problems) => {
            let (e, w) = report(&problems);
            return Err(format!("{e} error(s), {w} warning(s)"));
        }
    };
    let content = Content::merge(&loaded.packs);
    let problems: Vec<(Problem, Option<PathBuf>)> = data::validate_with(&content, opts)
        .into_iter()
        .map(|p| {
            let dir = p
                .source
                .as_ref()
                .and_then(|s| loaded.dirs.get(&s.pack))
                .cloned();
            (p, dir)
        })
        .collect();
    let (errors, warnings) = report(&problems);
    if errors > 0 {
        return Err(format!("{errors} error(s), {warnings} warning(s)"));
    }
    Ok((content, warnings))
}

fn validate(args: &[String]) -> Result<(), String> {
    let mut icons: Option<Option<PathBuf>> = None;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--icons" => {
                icons = Some(Some(PathBuf::from(
                    it.next().ok_or("--icons needs a directory")?,
                )))
            }
            "--no-icons" => icons = Some(None),
            _ => rest.push(a.clone()),
        }
    }
    let dirs = pack_dirs(&rest);
    let icon_dir = match icons {
        Some(explicit) => explicit,
        None => dirs[0]
            .join("../../art/icons")
            .canonicalize()
            .ok()
            .filter(|d| d.is_dir()),
    };
    let mut opts = ValidateOptions::default();
    if let Some(dir) = &icon_dir {
        let keys: BTreeSet<String> = std::fs::read_dir(dir)
            .map_err(|e| format!("cannot read icon directory {}: {e}", dir.display()))?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                e.file_name()
                    .to_string_lossy()
                    .split('.')
                    .next()
                    .map(str::to_string)
            })
            .collect();
        opts.icon_keys = Some(keys);
    }
    let (content, warnings) = load_content(&dirs, &opts)?;
    let counts = format!(
        "{} stats, {} categories, {} factions, {} materials, {} parts, {} ships, {} wiki pages",
        content.stats.len(),
        content.categories.len(),
        content.factions.len(),
        content.materials.len(),
        content.parts.len(),
        content.ships.len(),
        content.wiki.len()
    );
    let packs: Vec<&str> = content.packs.iter().map(|p| p.id.as_str()).collect();
    let icons_note = match &icon_dir {
        Some(d) => format!("icons checked against {}", d.display()),
        None => "icons not checked".into(),
    };
    println!(
        "ok: packs [{}]: {counts}; {warnings} warning(s); {icons_note}",
        packs.join(", ")
    );
    Ok(())
}

fn list(args: &[String]) -> Result<(), String> {
    let kind = args
        .first()
        .and_then(|k| Kind::from_key(k))
        .ok_or_else(|| USAGE.to_string())?;
    let (c, _) = load_content(&pack_dirs(&args[1..]), &ValidateOptions::default())?;
    fn rows<T: Entry>(items: &[T]) -> Vec<(String, String, String)> {
        items
            .iter()
            .map(|e| {
                (
                    e.id().to_string(),
                    e.name().to_string(),
                    e.source().to_string(),
                )
            })
            .collect()
    }
    let rows = match kind {
        Kind::Stat => rows(c.stats.as_slice()),
        Kind::Category => rows(c.categories.as_slice()),
        Kind::Faction => rows(c.factions.as_slice()),
        Kind::Material => rows(c.materials.as_slice()),
        Kind::Part => rows(c.parts.as_slice()),
        Kind::Ship => rows(c.ships.as_slice()),
        Kind::Page => c
            .wiki
            .values()
            .filter(|w| w.kind == Kind::Page)
            .map(|w| {
                (
                    w.id.clone(),
                    c.name_of(Kind::Page, &w.id).unwrap_or_default().to_string(),
                    w.source.to_string(),
                )
            })
            .collect(),
    };
    let w = rows.iter().map(|r| r.0.len()).max().unwrap_or(0);
    let n = rows.iter().map(|r| r.1.chars().count()).max().unwrap_or(0);
    for (id, name, src) in rows {
        println!("{id:w$}  {name:n$}  {src}");
    }
    Ok(())
}

fn export_json(args: &[String]) -> Result<(), String> {
    let out = args.first().ok_or_else(|| USAGE.to_string())?;
    let (c, _) = load_content(&pack_dirs(&args[1..]), &ValidateOptions::default())?;
    let text = serde_json::to_string_pretty(&export(&c)).map_err(|e| e.to_string())?;
    if out == "-" {
        println!("{text}");
    } else {
        std::fs::write(out, text + "\n").map_err(|e| format!("cannot write {out}: {e}"))?;
        eprintln!("wrote {out}");
    }
    Ok(())
}

/// Serialises an entry and adds `text_keys` (field → key), `links` (targets mentioned in its texts) and
/// `wiki` (long-form markdown or null).
fn entry_json<T: Entry + serde::Serialize>(c: &Content, e: &T) -> Value {
    let mut v = serde_json::to_value(e).unwrap();
    let keys: BTreeMap<&str, String> = e
        .text_fields()
        .into_iter()
        .map(|(f, _)| (f, text_key(T::KIND, e.id(), f)))
        .collect();
    let mut links = BTreeSet::new();
    for (_, text) in e.text_fields() {
        for l in data::parse_links(text).into_iter().flatten() {
            links.insert(format!("{}:{}", l.kind, l.id));
        }
    }
    let obj = v.as_object_mut().unwrap();
    obj.insert("text_keys".into(), json!(keys));
    obj.insert("links".into(), json!(links));
    obj.insert(
        "wiki".into(),
        json!(c.wiki_page(T::KIND, e.id()).map(|w| &w.markdown)),
    );
    v
}

fn export(c: &Content) -> Value {
    let stats: Vec<Value> = c.stats.iter().map(|e| entry_json(c, e)).collect();
    let categories: Vec<Value> = c
        .categories
        .iter()
        .map(|e| {
            let mut v = entry_json(c, e);
            let parts: Vec<&str> = c
                .parts
                .iter()
                .filter(|p| p.category == e.id)
                .map(|p| p.id.as_str())
                .collect();
            v["parts"] = json!(parts);
            v
        })
        .collect();
    let factions: Vec<Value> = c
        .factions
        .iter()
        .map(|e| {
            let mut v = entry_json(c, e);
            let ships: Vec<&str> = c
                .ships
                .iter()
                .filter(|s| s.faction == e.id)
                .map(|s| s.id.as_str())
                .collect();
            v["ships"] = json!(ships);
            v
        })
        .collect();
    let materials: Vec<Value> = c
        .materials
        .iter()
        .map(|e| {
            let mut v = entry_json(c, e);
            v["stat_lines"] = json!(c.material_stats(e));
            v
        })
        .collect();
    let parts: Vec<Value> = c
        .parts
        .iter()
        .map(|p| {
            let mut v = entry_json(c, p);
            v["color_key"] = json!(c.part_color(p));
            v["stat_lines"] = json!(c.part_stats(p, None));
            if p.hull {
                // Stats per armour material, for hull parts.
                let per: BTreeMap<&str, Value> = c
                    .materials
                    .iter()
                    .filter(|m| m.armor.is_some())
                    .map(|m| (m.id.as_str(), json!(c.part_stats(p, Some(m)))))
                    .collect();
                v["stat_lines_by_material"] = json!(per);
            }
            let shape = data::PlacedShape {
                footprint: p.footprint(),
                x: 0,
                y: 0,
                rot: 0,
                flip: false,
            };
            v["geometry"] = json!({
                "area": p.area(),
                "polygon": shape.polygon(),
                "connectable_sides": shape.connectable_sides(),
            });
            let used_by: Vec<&str> = c
                .ships
                .iter()
                .filter(|s| s.parts.iter().any(|pl| pl.part == p.id))
                .map(|s| s.id.as_str())
                .collect();
            v["used_by_ships"] = json!(used_by);
            v
        })
        .collect();
    let ships: Vec<Value> = c.ships.iter().map(|s| ship_json(c, s)).collect();
    let pages: Vec<Value> = c
        .wiki
        .values()
        .filter(|w| w.kind == Kind::Page)
        .map(|w| json!({ "id": w.id, "title": c.name_of(Kind::Page, &w.id), "markdown": w.markdown, "source": w.source }))
        .collect();
    json!({
        "format": "space-game-content",
        "format_version": EXPORT_FORMAT_VERSION,
        "schema_version": data::SCHEMA_VERSION,
        "packs": c.packs,
        "stats": stats,
        "categories": categories,
        "factions": factions,
        "materials": materials,
        "parts": parts,
        "ships": ships,
        "pages": pages,
        "texts": c.texts(),
    })
}

fn ship_json(c: &Content, s: &data::Blueprint) -> Value {
    let mut v = entry_json(c, s);
    let grid = c.ship_grid(s).expect("validated");
    let pieces = grid.pieces();
    let is_core = |i: usize| {
        c.parts
            .get(&s.parts[i].part)
            .is_some_and(|p| p.logic.is_some())
    };
    let main = grid.main_piece(&pieces, is_core);
    let mut totals: BTreeMap<String, f32> = BTreeMap::new();
    let placements: Vec<Value> = s
        .parts
        .iter()
        .enumerate()
        .map(|(i, pl)| {
            let part = c.parts.get(&pl.part).unwrap();
            for line in c.part_stats(part, c.placement_material(pl)) {
                if line.bonus.is_none() {
                    *totals.entry(line.stat).or_default() += line.value;
                }
            }
            let shape = grid.shape(i);
            json!({
                "part": pl.part, "x": pl.x, "y": pl.y, "rot": pl.rot, "flip": pl.flip, "material": pl.material,
                "cells": shape.cells(),
                "polygon": shape.polygon(),
                "center": shape.center(),
                "piece": pieces.piece_of_part[i],
            })
        })
        .collect();
    // Sums of per-part stat values that add up meaningfully (sim-level numbers like net power under load,
    // heat or thrust after clearance checks are not computed here).
    let summed = [
        "build_points",
        "mass",
        "hp",
        "power_output",
        "power_draw",
        "thrust",
        "cargo_capacity",
        "rule_slots",
    ];
    // Rounded: f32 sums carry float noise (70.400009) that would leak into the wiki.
    let totals: BTreeMap<&str, f64> = summed
        .iter()
        .filter_map(|k| {
            totals
                .get(*k)
                .map(|v| (*k, (*v as f64 * 1e4).round() / 1e4))
        })
        .collect();
    v["placements"] = json!(placements);
    v["totals"] = json!(totals);
    v["bounds"] = json!(grid.bounds());
    v["pieces"] = json!(pieces.parts);
    v["main_piece"] = json!(main);
    v["hull_loops"] = json!(
        main.map(|m| hull_loops(&grid.piece_hull_edges(&pieces, m)))
            .unwrap_or_default()
    );
    v
}
