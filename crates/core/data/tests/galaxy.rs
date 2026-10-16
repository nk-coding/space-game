//! The galaxy's kinds (GDR-0093, GDR-0094): galaxies, bands, cluster types, trails, signal sources and lore fragments. The base
//! pack's, and what validation refuses.

use data::{Content, Kind, Pack, Problem, Reward, Severity, SourceForm};

const MOD_MANIFEST: &str =
    r#"(id: "test_mod", name: "Test mod", version: "1.0.0", schema_version: 1, depends: ["base"])"#;

/// Base + a mod made of `files`: every problem `validate` finds.
fn problems_with(files: &[(&str, &str)]) -> Vec<Problem> {
    let mut all = vec![("pack.ron", MOD_MANIFEST)];
    all.extend_from_slice(files);
    let m = Pack::from_files(all).unwrap_or_else(|p| panic!("{p:?}"));
    data::validate(&Content::merge(&[data::base_pack().unwrap(), m]))
}

fn has(problems: &[Problem], id: &str, field: &str, needle: &str) {
    assert!(
        problems.iter().any(|p| p.severity == Severity::Error
            && p.id.as_deref() == Some(id)
            && p.field.as_deref() == Some(field)
            && p.message.contains(needle)),
        "no error on `{id}`.{field} with `{needle}` in {problems:#?}"
    );
}

#[test]
fn the_base_galaxy_is_the_rim_and_four_stub_bands() {
    let c = data::base_content();
    let g = c.galaxies.get("galaxy").unwrap();
    assert_eq!(g.bands, ["rim", "outer", "middle", "inner", "core"]);
    // Niklas, 16 Oct 2026 (GDR-0093): the drive jumps 2.5 S at the start.
    assert_eq!(g.jump_range, 2.5);
    assert!(g.spacing <= 0.9, "the first loop's voyages lie 0.9 S and more apart: they must fit");
    for b in &g.bands[1..] {
        assert!(c.bands.get(b).unwrap().tags.iter().any(|t| t == "stub"), "{b} is a stub");
    }
    let rim = c.bands.get("rim").unwrap();
    assert!(!rim.tags.iter().any(|t| t == "stub"));
    assert_eq!(c.trails.get(&rim.gates.as_ref().unwrap().trail).unwrap().reward, Reward::Gate);
    assert!(c.lore.iter().filter(|l| l.band == "rim").count() >= 3);
    assert!(c.signal_sources.iter().any(|s| s.form == SourceForm::Log));
    assert!(c.signal_sources.iter().any(|s| s.form == SourceForm::Beacon));
    assert_eq!(c.stats.get("jump_range").unwrap().unit, "S");
    let texts = c.texts();
    assert_eq!(texts["trail.rim_gate.hint"], "A gate");
    assert_eq!(texts["signal_source.hauler_log.line"], "a log from a hauler's wreck");
    assert!(texts.contains_key("lore.the_quiet.text"));
    assert_eq!(Kind::from_dir("cluster_types"), Some(Kind::ClusterType));
}

#[test]
fn validation_refuses_broken_galaxies() {
    let p = problems_with(&[(
        "galaxy.ron",
        r#"(id: "broken", name: "Broken", summary: "A galaxy that cannot be.", bands: ["core", "rim", "nowhere"],
            spacing: 2, jump_range: 3, home_neighbours: 3, warp: (3, 2),
            signal_area: (across: (3, 2), offset: 1), trail_step: (1, 2.5), names: (starts: [], ends: ["x"]))"#,
    )]);
    has(&p, "broken", "bands[2]", "unknown band `nowhere`");
    has(&p, "broken", "bands[1]", "must lie inside");
    has(&p, "broken", "bands[0]", "has no gates");
    has(&p, "broken", "bands[1]", "has no next band");
    has(&p, "broken", "jump_range", "at least twice the spacing");
    has(&p, "broken", "signal_area.across", "min ≤ max");
    has(&p, "broken", "signal_area.offset", "below 1");
    has(&p, "broken", "names.starts", "at least one name part");
}

#[test]
fn validation_refuses_broken_bands_types_trails_sources_and_lore() {
    let p = problems_with(&[
        (
            "bands.ron",
            r#"(id: "odd", name: "Odd", summary: "A band.", edge: 0, clusters: [(id: "nothing", weight: 0)],
                gates: (count: 0, trail: "graveyard"), trails: [(trail: "rim_gate", count: 1)])"#,
        ),
        (
            "cluster_types/odd.ron",
            r#"(id: "too_big", name: "Too big", summary: "A cluster.", radius: (3000, 5000), spacing: (7000, 8000),
                words: [], sites: [(encounter: "nobody", count: (2, 1))])"#,
        ),
        (
            "trails.ron",
            r#"(id: "long", name: "Long", summary: "A trail.", reward: Gate, length: (0, 5), target: "derelict_field",
                sources: ["nothing", "nothing"], hint: " ")"#,
        ),
        (
            "signal_sources.ron",
            r#"[(id: "lost_log", name: "Lost log", summary: "A log.", form: Log, line: "a log"),
                (id: "bad_log", name: "Bad log", summary: "A log.", form: Log, line: "a log", site: "raider_pair")]"#,
        ),
        (
            "lore/odd.ron",
            r#"(id: "odd_fragment", name: "Odd", summary: "A fragment.", band: "nowhere", trail: "graveyard", text: "")"#,
        ),
    ]);
    has(&p, "odd", "edge", "must be a finite number > 0");
    has(&p, "odd", "clusters[0].id", "unknown cluster_type `nothing`");
    has(&p, "odd", "clusters[0].weight", "above 0");
    has(&p, "odd", "gates.count", "at least one gate");
    has(&p, "odd", "gates.trail", "leads to Salvage");
    has(&p, "odd", "trails[0].trail", "leads to Gate");
    has(&p, "too_big", "radius", "≤ 4000 m");
    has(&p, "too_big", "spacing", "never fit");
    has(&p, "too_big", "words", "at least one name word");
    has(&p, "too_big", "sites[0].encounter", "unknown encounter `nobody`");
    has(&p, "too_big", "sites[0].count", "min 2 is above max 1");
    has(&p, "long", "length", "1 ≤ min ≤ max ≤ 4");
    has(&p, "long", "target", "no ships for sure");
    has(&p, "long", "sources[0]", "unknown signal_source `nothing`");
    has(&p, "long", "hint", "must not be empty");
    has(&p, "lost_log", "site", "name the encounter it lies in");
    has(&p, "bad_log", "site", "no wrecks");
    has(&p, "odd_fragment", "band", "unknown band `nowhere`");
    has(&p, "odd_fragment", "trail", "leads to Salvage");
    has(&p, "odd_fragment", "text", "must not be empty");
}
