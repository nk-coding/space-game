//! `content-tool`: validate content packs, export the resolved content as JSON, list entries, generate the wiki's
//! data.
//!
//! ```text
//! content-tool validate [pack_dir...]
//! content-tool export-json <out.json | -> [pack_dir...]
//! content-tool list <kind> [pack_dir...]
//! content-tool wiki-data <wiki_dir> [pack_dir...]
//! content-tool galaxy [--seed N] [--svg out.svg] [pack_dir...]
//! content-tool galaxy --seeds N [pack_dir...]
//! ```
//!
//! Pack directories are loaded in the order given (later packs override earlier ones by id). Without any,
//! `content/base` is used (relative to the current directory, else to this crate's repo).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use data::design::geometry::hull_loops;
use data::{
    Content, Encounter, Entry, Kind, Material, Pack, Part, Problem, ResearchNode, Severity, Start,
    Unlock, Voyage, text_key,
};
use serde_json::{Value, json};

mod survey;
mod wiki;

const USAGE: &str = "usage:
  content-tool validate [pack_dir...]
  content-tool export-json <out.json | -> [pack_dir...]
  content-tool list <kind> [pack_dir...]     kind: stat, category, faction, material, part, frame,
                                             ship, behavior_node, behavior, family, research, start,
                                             voyage, encounter, galaxy, band, cluster_type,
                                             trail, signal_source, lore, page
  content-tool wiki-data <wiki_dir> [pack_dir...]
                                             the wiki's generated half into <wiki_dir>/gen, checking its
                                             hand-written pages (docs/wiki.md)
  content-tool galaxy [--seed N] [--svg out.svg] [pack_dir...]
                                             generates the galaxy for a seed (default 1, the `galaxy`
                                             crate): its bands, home, gates and trails, and its map as SVG
  content-tool galaxy --seeds N [pack_dir...]
                                             seeds 1 to N: home's free clusters in range, and how many
                                             voyages finding a signal takes

Pack dirs load in order; later packs override earlier ones by id (icons by key). Default: content/base.";

/// Version of the JSON export layout (bump on breaking changes for external readers).
const EXPORT_FORMAT_VERSION: u32 = 1;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("validate") => validate(&args[1..]),
        Some("export-json") => export_json(&args[1..]),
        Some("list") => list(&args[1..]),
        Some("wiki-data") => wiki_data(&args[1..]),
        Some("galaxy") => galaxy_cmd(&args[1..]),
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
    vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../content/base")]
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
    s + ": " + p.message.as_str()
}

fn report(problems: &[(Problem, Option<PathBuf>)]) -> (usize, usize) {
    for (p, dir) in problems {
        eprintln!("{}", format_problem(p, dir.as_deref()));
    }
    let errors = problems.iter().filter(|(p, _)| p.is_error()).count();
    (errors, problems.len() - errors)
}

/// Loads, merges and validates; prints every problem. `Err` if there are errors.
fn load_content(dirs: &[PathBuf]) -> Result<(Content, usize), String> {
    let loaded = match load_packs(dirs) {
        Ok(l) => l,
        Err(problems) => {
            let (e, w) = report(&problems);
            return Err(format!("{e} error(s), {w} warning(s)"));
        }
    };
    let content = Content::merge(&loaded.packs);
    let mut found = data::validate(&content);
    // Behavior nodes must match what the sim implements; Rhai conditions must compile. Only meaningful once
    // the content itself is valid.
    if !found.iter().any(Problem::is_error) {
        found.extend(sim::behavior::check_vocabulary(&content));
    }
    let problems: Vec<(Problem, Option<PathBuf>)> = found
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
    let (content, warnings) = load_content(&pack_dirs(args))?;
    let counts = format!(
        "{} stats, {} categories, {} factions, {} materials, {} parts, {} frames, {} ships, {} behavior nodes, {} behaviors, {} families, {} research nodes, {} starts, {} voyages, {} encounters, {} galaxies, {} bands, {} cluster types, {} trails, {} signal sources, {} lore fragments, {} controls, {} icons, {} wiki pages",
        content.stats.len(),
        content.categories.len(),
        content.factions.len(),
        content.materials.len(),
        content.parts.len(),
        content.frames.len(),
        content.ships.len(),
        content.behavior_nodes.len(),
        content.behaviors.len(),
        content.families.len(),
        content.research.len(),
        content.starts.len(),
        content.voyages.len(),
        content.encounters.len(),
        content.galaxies.len(),
        content.bands.len(),
        content.cluster_types.len(),
        content.trails.len(),
        content.signal_sources.len(),
        content.lore.len(),
        content.controls.len(),
        content.icons.len(),
        content.wiki.len()
    );
    let packs: Vec<&str> = content.packs.iter().map(|p| p.id.as_str()).collect();
    println!(
        "ok: packs [{}]: {counts}; {warnings} warning(s)",
        packs.join(", ")
    );
    Ok(())
}

fn list(args: &[String]) -> Result<(), String> {
    let kind = args
        .first()
        .and_then(|k| Kind::from_key(k))
        .ok_or_else(|| USAGE.to_string())?;
    let (c, _) = load_content(&pack_dirs(&args[1..]))?;
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
        Kind::Frame => rows(c.frames.as_slice()),
        Kind::Ship => rows(c.ships.as_slice()),
        Kind::BehaviorNode => rows(c.behavior_nodes.as_slice()),
        Kind::Behavior => rows(c.behaviors.as_slice()),
        Kind::Family => rows(c.families.as_slice()),
        Kind::Research => rows(c.research.as_slice()),
        Kind::Facility => rows(c.facilities.as_slice()),
        Kind::Start => rows(c.starts.as_slice()),
        Kind::Voyage => rows(c.voyages.as_slice()),
        Kind::Encounter => rows(c.encounters.as_slice()),
        Kind::Galaxy => rows(c.galaxies.as_slice()),
        Kind::Band => rows(c.bands.as_slice()),
        Kind::ClusterType => rows(c.cluster_types.as_slice()),
        Kind::Trail => rows(c.trails.as_slice()),
        Kind::SignalSource => rows(c.signal_sources.as_slice()),
        Kind::Lore => rows(c.lore.as_slice()),
        Kind::Control => rows(c.controls.as_slice()),
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

/// Writes the wiki's generated half (`<wiki_dir>/gen`) and checks its hand-written pages against the content: a
/// problem there fails the command, after the data is written (so the site still builds while it is fixed).
fn wiki_data(args: &[String]) -> Result<(), String> {
    let dir = PathBuf::from(args.first().ok_or_else(|| USAGE.to_string())?);
    let (c, content_warnings) = load_content(&pack_dirs(&args[1..]))?;
    let hand = wiki::hand::read(&dir)?;
    let site = wiki::generate(&c, &hand);
    for w in &site.warnings {
        eprintln!("warning: wiki: {w}");
    }
    let out = dir.join("gen");
    wiki::write(&site, &out)?;
    let pages = site.files.keys().filter(|p| p.ends_with(".md")).count();
    println!(
        "wrote {pages} pages and the wiki data to {}; {} warning(s)",
        out.display(),
        content_warnings + site.warnings.len()
    );
    for e in &site.errors {
        eprintln!("error: wiki: {e}");
    }
    if site.errors.is_empty() {
        Ok(())
    } else {
        Err(format!("{} error(s) in the wiki's hand-written pages", site.errors.len()))
    }
}

/// Generates a galaxy of the content (the first `galaxy.ron` entry) for a seed and prints what it holds; `--svg`
/// also writes its map.
fn galaxy_cmd(args: &[String]) -> Result<(), String> {
    let (mut seed, mut svg, mut seeds, mut dirs) = (1u64, None, None, Vec::new());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|v| v.parse().ok()).ok_or("--seed needs an integer")?,
            "--seeds" => seeds = Some(it.next().and_then(|v| v.parse().ok()).ok_or("--seeds needs an integer")?),
            "--svg" => svg = Some(it.next().ok_or("--svg needs a file")?.clone()),
            _ => dirs.push(a.clone()),
        }
    }
    let (c, _) = load_content(&pack_dirs(&dirs))?;
    if let Some(n) = seeds {
        return survey::survey(&c, n);
    }
    let g = c.galaxies.iter().next().ok_or("the content has no galaxy (`galaxy.ron`)")?;
    let s = galaxy::generate(&c, &g.id, seed).map_err(|e| e.to_string())?;
    let name = |k: u32| s.clusters[k as usize].name.as_str();
    let counts: Vec<String> = g.bands.iter().map(|b| format!("{b} {}", s.band(b).count())).collect();
    println!("galaxy `{}`, seed {seed}, generator {}: {} clusters ({}), hash {:016x}", g.id, s.version, s.clusters.len(), counts.join(", "), s.hash());
    let home = s.clusters[s.home as usize].at;
    let piece = |k: u32| s.pieces.values().any(|&p| p == k);
    let near = s.band(&g.bands[0]).filter(|&k| k != s.home && !piece(k) && s.clusters[k as usize].at.distance_s(home) <= g.jump_range as f64).count();
    println!("home: {} at ({:.2}, {:.2}) S, {near} free clusters of its band within {} S", name(s.home), home.x as f64 / 100.0, home.y as f64 / 100.0, g.jump_range);
    for (v, &k) in &s.pieces {
        let gate = if s.clusters[k as usize].gate { ", a gate" } else { "" };
        println!("set piece {v}: {} at {:.1} S from home{gate}", name(k), s.clusters[k as usize].at.distance_s(home));
    }
    for b in &g.bands {
        let gates: Vec<&str> = s.band(b).filter(|&k| s.clusters[k as usize].gate).map(name).collect();
        if !gates.is_empty() {
            println!("gates of {b}: {}", gates.join(", "));
        }
    }
    let mut nearest: Vec<f64> = s
        .clusters
        .iter()
        .enumerate()
        .map(|(k, a)| s.clusters.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, b)| a.at.distance_s(b.at)).fold(f64::MAX, f64::min))
        .collect();
    nearest.sort_by(f64::total_cmp);
    println!("nearest neighbour: {:.2} / {:.2} / {:.2} S (min / median / max)", nearest[0], nearest[nearest.len() / 2], nearest[nearest.len() - 1]);
    for t in &s.trails {
        let steps: Vec<String> = t
            .places
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let what = match (t.signals.get(k), &p.encounter) {
                    (Some(sig), _) => sig.source.clone(),
                    (None, Some(e)) => e.clone(),
                    (None, None) => "?".into(),
                };
                let hop = k.checked_sub(1).map_or(String::new(), |j| {
                    format!("{:.1} S → ", s.clusters[t.places[j].cluster as usize].at.distance_s(s.clusters[p.cluster as usize].at))
                });
                format!("{hop}{} [{what}]", name(p.cluster))
            })
            .collect();
        let lore = t.lore.as_ref().map_or(String::new(), |l| format!(" ({l})"));
        println!("trail {}{lore} in {}, {} signals: {}", t.trail, t.band, t.signals.len(), steps.join(" "));
    }
    if let Some(path) = svg {
        let map = wiki::map_svg(&c, &s);
        std::fs::write(&path, map).map_err(|e| format!("cannot write {path}: {e}"))?;
        eprintln!("wrote {path}");
    }
    Ok(())
}

fn export_json(args: &[String]) -> Result<(), String> {
    let out = args.first().ok_or_else(|| USAGE.to_string())?;
    let (c, _) = load_content(&pack_dirs(&args[1..]))?;
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

// Career lookups (export-json and the wiki), each in load order.

/// The research nodes that unlock what `unlock` names.
fn unlocked_by<'a>(c: &'a Content, unlock: &Unlock) -> Vec<&'a ResearchNode> {
    c.research
        .iter()
        .filter(|n| n.unlocks.contains(unlock))
        .collect()
}

/// The research nodes that need node `id` first.
fn required_by<'a>(c: &'a Content, id: &str) -> Vec<&'a ResearchNode> {
    c.research
        .iter()
        .filter(|n| n.requires.iter().any(|r| r == id))
        .collect()
}

/// The research nodes in a family's tree.
fn family_research<'a>(c: &'a Content, family: &str) -> Vec<&'a ResearchNode> {
    c.research.iter().filter(|n| n.family == family).collect()
}

/// What feeds a family: the parts and the armour materials whose salvage gives its data.
fn salvaged_from<'a>(c: &'a Content, family: &str) -> (Vec<&'a Part>, Vec<&'a Material>) {
    let parts = c
        .parts
        .iter()
        .filter(|p| p.salvage.as_ref().is_some_and(|s| s.family == family))
        .collect();
    let materials = c
        .materials
        .iter()
        .filter(|m| m.salvage.as_ref().is_some_and(|s| s.family == family))
        .collect();
    (parts, materials)
}

// Voyage lookups (export-json and the wiki), each in load order.

/// The voyages that open after voyage `id`: they name it in `opens_after`.
fn opens<'a>(c: &'a Content, id: &str) -> Vec<&'a Voyage> {
    c.voyages
        .iter()
        .filter(|v| v.opens_after.iter().any(|a| a == id))
        .collect()
}

/// The starts that open voyage `id` (list it in `voyages`).
fn starts_opening<'a>(c: &'a Content, id: &str) -> Vec<&'a Start> {
    c.starts
        .iter()
        .filter(|s| s.voyages.iter().any(|v| v == id))
        .collect()
}

/// The voyages with sites of encounter `id`, with how many such sites each places.
fn voyages_using<'a>(c: &'a Content, id: &str) -> Vec<(&'a Voyage, u32)> {
    c.voyages
        .iter()
        .filter_map(|v| {
            let sites = v.sites.iter().filter(|s| s.encounter == id);
            let n = sites.fold(0, |n: u32, s| n.saturating_add(s.count));
            (n > 0).then_some((v, n))
        })
        .collect()
}

/// The encounters ship `id` appears in, as a fighting ship or as a wreck.
fn encounters_with<'a>(c: &'a Content, id: &str) -> Vec<&'a Encounter> {
    c.encounters
        .iter()
        .filter(|e| e.ships.iter().any(|s| s.ship == id) || e.wrecks.iter().any(|w| w.ship == id))
        .collect()
}

fn ids<T: Entry>(entries: &[&T]) -> Vec<String> {
    entries.iter().map(|e| e.id().to_string()).collect()
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
            v["unlocked_by"] = json!(ids(&unlocked_by(c, &Unlock::Material(e.id.clone()))));
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
            v["unlocked_by"] = json!(ids(&unlocked_by(c, &Unlock::Part(p.id.clone()))));
            v
        })
        .collect();
    let frames: Vec<Value> = c
        .frames
        .iter()
        .map(|e| {
            let mut v = entry_json(c, e);
            v["unlocked_by"] = json!(ids(&unlocked_by(c, &Unlock::Frame(e.id.clone()))));
            v
        })
        .collect();
    // Families list what feeds them (parts and armour materials by their salvage) and their research nodes.
    let families: Vec<Value> = c
        .families
        .iter()
        .map(|e| {
            let mut v = entry_json(c, e);
            let (parts, materials) = salvaged_from(c, &e.id);
            v["salvaged_from_parts"] = json!(ids(&parts));
            v["salvaged_from_materials"] = json!(ids(&materials));
            v["research"] = json!(ids(&family_research(c, &e.id)));
            v
        })
        .collect();
    let research: Vec<Value> = c
        .research
        .iter()
        .map(|n| {
            let mut v = entry_json(c, n);
            v["required_by"] = json!(ids(&required_by(c, &n.id)));
            v
        })
        .collect();
    let starts: Vec<Value> = c.starts.iter().map(|e| entry_json(c, e)).collect();
    let facilities: Vec<Value> = c
        .facilities
        .iter()
        .map(|e| {
            let mut v = entry_json(c, e);
            v["unlocked_by"] = json!(ids(&unlocked_by(c, &Unlock::Facility(e.id.clone()))));
            v
        })
        .collect();
    // Voyages list what opens them and what they open; encounters the voyages that place them.
    let voyages: Vec<Value> = c
        .voyages
        .iter()
        .map(|e| {
            let mut v = entry_json(c, e);
            v["site_count"] = json!(e.site_count());
            v["opens"] = json!(ids(&opens(c, &e.id)));
            v["opened_by_starts"] = json!(ids(&starts_opening(c, &e.id)));
            v
        })
        .collect();
    let encounters: Vec<Value> = c
        .encounters
        .iter()
        .map(|e| {
            let mut v = entry_json(c, e);
            let voyages: Vec<&str> = voyages_using(c, &e.id)
                .into_iter()
                .map(|(x, _)| x.id.as_str())
                .collect();
            v["used_by_voyages"] = json!(voyages);
            v
        })
        .collect();
    let ships: Vec<Value> = c.ships.iter().map(|s| ship_json(c, s)).collect();
    let behavior_nodes: Vec<Value> = c
        .behavior_nodes
        .iter()
        .map(|n| {
            let mut v = entry_json(c, n);
            let keys: BTreeMap<String, String> = n
                .param_texts()
                .into_iter()
                .map(|(f, _)| (f.clone(), text_key(Kind::BehaviorNode, &n.id, &f)))
                .collect();
            v["param_text_keys"] = json!(keys);
            v["builtin"] = json!(n.script.is_none());
            let used_by: Vec<&str> = c
                .behaviors
                .iter()
                .filter(|b| {
                    b.program().is_ok_and(|p| {
                        p.target.iter().any(|t| t.call.name == n.id)
                            || p.rules.iter().any(|r| {
                                r.action.name == n.id || r.when.iter().any(|w| w.call.name == n.id)
                            })
                    })
                })
                .map(|b| b.id.as_str())
                .collect();
            v["used_by_behaviors"] = json!(used_by);
            v
        })
        .collect();
    let behaviors: Vec<Value> = c
        .behaviors
        .iter()
        .map(|b| {
            let mut v = entry_json(c, b);
            let program = b.program().expect("validated");
            v["text"] = json!(program.to_text());
            v["program"] = json!(program);
            v["rule_count"] = json!(program.rules.len());
            let factions: Vec<&str> = c
                .factions
                .iter()
                .filter(|f| f.behavior.as_deref() == Some(b.id.as_str()))
                .map(|f| f.id.as_str())
                .collect();
            let ships: Vec<&str> = c
                .ships
                .iter()
                .filter(|s| c.ship_behavior(s).is_some_and(|x| x.id == b.id))
                .map(|s| s.id.as_str())
                .collect();
            v["default_of_factions"] = json!(factions);
            v["flown_by_ships"] = json!(ships);
            v
        })
        .collect();
    // Icons as SVG text (the wiki can inline them or write them out as files), with what uses them.
    let icons: Vec<Value> = c
        .icons
        .values()
        .map(|i| {
            let users = |keys: Vec<(&str, Option<&str>)>| -> Vec<String> {
                keys.into_iter()
                    .filter(|(_, k)| *k == Some(i.key.as_str()))
                    .map(|(id, _)| id.to_string())
                    .collect()
            };
            json!({
                "key": i.key,
                "svg": i.svg,
                "source": i.source,
                "used_by": {
                    "parts": users(c.parts.iter().map(|p| (p.id.as_str(), p.icon.as_deref())).collect()),
                    "materials": users(c.materials.iter().map(|m| (m.id.as_str(), m.icon.as_deref())).collect()),
                    "categories": users(c.categories.iter().map(|k| (k.id.as_str(), k.icon.as_deref())).collect()),
                },
            })
        })
        .collect();
    let pages: Vec<Value> = c
        .wiki
        .values()
        .filter(|w| w.kind == Kind::Page)
        .map(|w| json!({ "id": w.id, "title": c.name_of(Kind::Page, &w.id), "markdown": w.markdown, "source": w.source }))
        .collect();
    let mut out = json!({
        "format": "space-game-content",
        "format_version": EXPORT_FORMAT_VERSION,
        "schema_version": data::SCHEMA_VERSION,
        "packs": c.packs,
        "stats": stats,
        "categories": categories,
        "factions": factions,
        "materials": materials,
        "parts": parts,
        "frames": frames,
        "ships": ships,
        "behavior_nodes": behavior_nodes,
        "behaviors": behaviors,
        "families": families,
        "research": research,
        "facilities": facilities,
        "starts": starts,
        "voyages": voyages,
        "encounters": encounters,
        "galaxies": c.galaxies.iter().map(|e| entry_json(c, e)).collect::<Vec<_>>(),
        "bands": c.bands.iter().map(|e| entry_json(c, e)).collect::<Vec<_>>(),
        "cluster_types": c.cluster_types.iter().map(|e| entry_json(c, e)).collect::<Vec<_>>(),
        "trails": c.trails.iter().map(|e| entry_json(c, e)).collect::<Vec<_>>(),
        "signal_sources": c.signal_sources.iter().map(|e| entry_json(c, e)).collect::<Vec<_>>(),
        "lore": c.lore.iter().map(|e| entry_json(c, e)).collect::<Vec<_>>(),
        "icons": icons,
        "pages": pages,
        "texts": c.texts(),
    });
    shortest_f32(&mut out);
    out
}

/// Content numbers are f32; serde widens them to f64, which shows the f32's binary noise (0.28 becomes
/// 0.2800000011920929). Every number that is exactly an f32 is written as that f32's shortest text instead.
fn shortest_f32(v: &mut Value) {
    match v {
        Value::Number(n) if n.is_f64() => {
            if let Some(x) = n.as_f64()
                && f64::from(x as f32) == x
                && let Some(m) = format!("{}", x as f32)
                    .parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
            {
                *n = m;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(shortest_f32),
        Value::Object(o) => o.values_mut().for_each(shortest_f32),
        _ => {}
    }
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
    let placements: Vec<Value> = s
        .parts
        .iter()
        .enumerate()
        .map(|(i, pl)| {
            let shape = grid.shape(i);
            json!({
                "part": pl.part, "x": pl.x, "y": pl.y, "rot": pl.rot, "flip": pl.flip, "material": pl.material,
                "power": pl.power,
                "cells": shape.cells(),
                "polygon": shape.polygon(),
                "center": shape.center(),
                "piece": pieces.piece_of_part[i],
            })
        })
        .collect();
    // Sums of per-part stat values that add up meaningfully (sim-level numbers like net power under load,
    // heat or thrust after clearance checks are not computed here). A key is present when some part has that
    // stat line. Rounded: f32 sums carry float noise (70.400009) that would leak into the wiki.
    let t = c.ship_totals(s);
    let totals: BTreeMap<&str, f64> = [
        ("build_points", t.build_points, true),
        ("mass", t.mass, true),
        ("hp", t.hp, true),
        ("power_output", t.power_output, t.power_output > 0.0),
        ("power_draw", t.power_draw, t.power_draw > 0.0),
        ("thrust", t.thrust, t.thrust > 0.0),
        ("cargo_capacity", t.cargo_capacity, t.cargo_capacity > 0.0),
        ("rule_slots", t.rule_slots as f32, t.logic_cores > 0),
        ("control_cost", t.control_cost as f32, t.control_cost > 0),
        (
            "control_capacity",
            t.control_capacity as f32,
            t.logic_cores > 0,
        ),
    ]
    .into_iter()
    .filter(|(_, _, present)| *present)
    .map(|(k, v, _)| (k, (v as f64 * 1e4).round() / 1e4))
    .collect();
    v["rule_slots"] = json!(c.ship_rule_slots(s));
    let (control_load, control_capacity) = c.ship_control(s);
    v["control_load"] = json!(control_load);
    v["control_capacity"] = json!(control_capacity);
    v["effective_behavior"] = json!(c.ship_behavior(s).map(|b| &b.id));
    v["in_encounters"] = json!(ids(&encounters_with(c, &s.id)));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A ship's `totals` are the sums of its parts' stat lines (what the export shows per part), key for key:
    /// a key appears when some part has that line.
    #[test]
    fn ship_totals_sum_the_part_stat_lines() {
        let base = data::fs::load_pack_dir(&pack_dirs(&[])[0]).unwrap();
        // Next to the base ships: a core without rule slots or capacity, and no power, engine or cargo.
        let m = Pack::from_files([
            (
                "pack.ron",
                r#"(id: "test_mod", name: "T", version: "1", schema_version: 1, depends: ["base"])"#,
            ),
            (
                "parts/p.ron",
                r#"(id: "dummy_core", name: "Dummy core", summary: "S.", category: "logic", size: (1, 1),
                    build_points: 0, mass: 1, hp: 1, logic: (rule_slots: 0, control_capacity: 0))"#,
            ),
            (
                "ships/s.ron",
                r#"(id: "s", name: "S", summary: "S.", faction: "ship_mind", size: (2, 1), parts: [
                    (part: "dummy_core", x: 0, y: 0), (part: "slope_1x1", x: 1, y: 0, material: "composite"),
                ])"#,
            ),
        ])
        .unwrap();
        let c = Content::load(&[base, m]).unwrap();
        let summed = [
            "build_points",
            "mass",
            "hp",
            "power_output",
            "power_draw",
            "thrust",
            "cargo_capacity",
            "rule_slots",
            "control_cost",
            "control_capacity",
        ];
        for s in c.ships.iter() {
            let mut sums: BTreeMap<String, f32> = BTreeMap::new();
            for pl in &s.parts {
                let part = c.parts.get(&pl.part).unwrap();
                for line in c.part_stats(part, c.placement_material(pl)) {
                    if line.bonus.is_none() {
                        *sums.entry(line.stat).or_default() += line.value;
                    }
                }
            }
            let want: BTreeMap<String, f64> = sums
                .into_iter()
                .filter(|(k, _)| summed.contains(&k.as_str()))
                .map(|(k, v)| (k, (v as f64 * 1e4).round() / 1e4))
                .collect();
            assert_eq!(ship_json(&c, s)["totals"], json!(want), "{}", s.id);
        }
        let edge = &ship_json(&c, c.ships.get("s").unwrap())["totals"];
        assert_eq!(edge["rule_slots"], json!(0.0));
        assert!(edge.get("power_output").is_none() && edge.get("thrust").is_none());
    }

    /// Parts and materials carry `locked` (what the player can't build yet), locked or not.
    #[test]
    fn export_marks_locked_parts_and_materials() {
        let base = data::fs::load_pack_dir(&pack_dirs(&[])[0]).unwrap();
        let v = export(&Content::load(&[base]).unwrap());
        let locked = |kind: &str, id: &str| {
            let entries = v[kind].as_array().unwrap();
            entries.iter().find(|e| e["id"] == id).unwrap()["locked"].clone()
        };
        assert_eq!(locked("parts", "automaton_cannon"), json!(true));
        assert_eq!(locked("parts", "pulse_laser"), json!(false));
        assert_eq!(locked("materials", "automaton_plate"), json!(true));
        assert_eq!(locked("materials", "steel"), json!(false));
        assert_eq!(locked("frames", "cruiser"), json!(true));
        assert_eq!(locked("frames", "frigate"), json!(false));
    }

    /// The career's content: families with what feeds them, research nodes with what needs them, starts, the
    /// salvage blocks, and per part, material and frame the nodes that unlock it.
    #[test]
    fn export_holds_the_career_content() {
        let base = data::fs::load_pack_dir(&pack_dirs(&[])[0]).unwrap();
        let c = Content::load(&[base]).unwrap();
        let v = export(&c);
        let find = |kind: &str, id: &str| -> Value {
            let entries = v[kind].as_array().unwrap_or_else(|| panic!("no {kind}"));
            entries.iter().find(|e| e["id"] == id).unwrap().clone()
        };
        let logic = find("families", "logic");
        assert_eq!(logic["color"], json!("logic"));
        assert_eq!(logic["text_keys"]["name"], json!("family.logic.name"));
        assert_eq!(
            logic["salvaged_from_parts"],
            json!(["automaton_core", "logic_core", "scanner"])
        );
        assert_eq!(
            logic["research"],
            json!(["tamed_core", "mind_expansion_1", "mind_expansion_2", "simulation_vault"])
        );
        assert_eq!(
            find("families", "hull")["salvaged_from_materials"],
            json!(["steel", "composite", "reactive", "automaton_plate"])
        );
        let node = find("research", "mind_expansion_1");
        assert_eq!(node["kind"], json!("Refinement"));
        assert_eq!(node["requires"], json!(["tamed_core"]));
        assert_eq!(node["required_by"], json!(["mind_expansion_2"]));
        assert_eq!(node["unlocks"], json!([{"MindCapacity": 1}]));
        assert_eq!(
            node["cost"],
            json!({"data": {"logic": 10.0}, "materials": {"programmable_matter": 3}})
        );
        let start = find("starts", "rim_awakening");
        assert_eq!(
            (&start["ship"], &start["mind_capacity"]),
            (&json!("rim_skiff"), &json!(2))
        );
        let core = find("parts", "automaton_core");
        assert_eq!(core["unlocked_by"], json!(["tamed_core"]));
        assert_eq!(
            core["salvage"],
            json!({"family": "logic", "data": 4.0, "materials": {"programmable_matter": 1},
                   "fragment": {"programmable_matter": 1}})
        );
        assert_eq!(find("parts", "pulse_laser")["unlocked_by"], json!([]));
        assert_eq!(
            find("materials", "composite")["unlocked_by"],
            json!(["composite_plating"])
        );
        assert_eq!(
            find("materials", "steel")["salvage"],
            json!({"family": "hull", "data": 1.0})
        );
        assert_eq!(
            find("frames", "cruiser")["unlocked_by"],
            json!(["cruiser_drydock"])
        );
        assert_eq!(
            v["texts"]["research.cruiser_drydock.name"],
            json!("Cruiser drydock")
        );
    }

    /// Voyages with what opens them and what they open, encounters with the voyages that place them, and per
    /// ship the encounters it appears in (fighting or as a wreck).
    #[test]
    fn export_holds_voyages_and_encounters() {
        let base = data::fs::load_pack_dir(&pack_dirs(&[])[0]).unwrap();
        let c = Content::load(&[base]).unwrap();
        let v = export(&c);
        let find = |kind: &str, id: &str| -> Value {
            let entries = v[kind].as_array().unwrap_or_else(|| panic!("no {kind}"));
            entries.iter().find(|e| e["id"] == id).unwrap().clone()
        };
        let first = find("voyages", "yard_approach");
        assert_eq!(first["band"], json!("rim"));
        assert_eq!(first["site_count"], json!(4));
        assert_eq!(
            first["sites"][2],
            json!({"encounter": "raider_pair", "count": 1, "goal": true})
        );
        assert_eq!(first["opens"], json!(["halvex_drift"]));
        assert_eq!(first["opened_by_starts"], json!(["rim_awakening"]));
        assert_eq!(
            first["text_keys"]["cleared"],
            json!("voyage.yard_approach.cleared")
        );
        assert_eq!(first["links"], json!(["voyage:halvex_drift"]));
        let last = find("voyages", "wardens_gate");
        assert_eq!(last["opens_after"], json!(["corran_reach"]));
        assert_eq!(
            (&last["opens"], &last["opened_by_starts"]),
            (&json!([]), &json!([]))
        );
        assert_eq!(
            find("encounters", "raider_pair")["used_by_voyages"],
            json!(["yard_approach", "halvex_drift"])
        );
        assert_eq!(
            find("encounters", "raider_pack")["used_by_voyages"],
            json!(["wardens_gate"])
        );
        assert_eq!(
            find("encounters", "hunter_den")["used_by_voyages"],
            json!(["corran_reach"])
        );
        let field = find("encounters", "derelict_field");
        assert_eq!(field["wake_radius"], json!(200.0));
        assert_eq!(
            field["wrecks"][0],
            json!({"ship": "sample_early", "count": [1, 1], "damage": 0.5})
        );
        assert_eq!(
            find("ships", "sample_early")["in_encounters"],
            json!(["derelict_field"])
        );
        assert_eq!(
            find("ships", "automaton_warden")["in_encounters"],
            json!(["gate_sentinel", "the_warden"])
        );
        assert_eq!(find("ships", "rim_skiff")["in_encounters"], json!([]));
        // The raider is almost everywhere: fighting in every encounter but the hunters' den, the gate sentinel and the
        // memory wreck, lying as a wreck in some.
        assert_eq!(
            find("ships", "automaton_raider")["in_encounters"]
                .as_array()
                .unwrap()
                .len(),
            c.encounters.len() - 3
        );
        assert_eq!(v["texts"]["encounter.the_warden.name"], json!("The Warden"));
        // f32 numbers export as written, without the noise of widening them to f64.
        assert_eq!(
            find("encounters", "precursor_wrecks")["wrecks"][0]["damage"],
            json!(0.4)
        );
    }
}
