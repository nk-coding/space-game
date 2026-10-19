//! The play-test kits (`playtest.ron`, `?bands=`): the base pack's, a mod that replaces one, and the errors a bad file
//! gets.

use data::{Content, Pack, Severity};

const MOD_MANIFEST: &str =
    r#"(id: "test_mod", name: "Test mod", version: "1.0.0", schema_version: 1, depends: ["base"])"#;

fn with_kits(text: &str) -> Result<Content, Vec<data::Problem>> {
    let m = Pack::from_files([("pack.ron", MOD_MANIFEST), ("playtest.ron", text)])?;
    Content::load(&[data::base_pack().unwrap(), m])
}

#[test]
fn the_base_kits_cover_every_research_node_once_and_change_no_fight() {
    let c = data::base_content();
    let bands: Vec<&str> = c.playtest.iter().map(|k| k.band.as_str()).collect();
    assert_eq!(bands, ["outer", "middle", "inner"]);
    let mut all: Vec<&str> = c.playtest.iter().flat_map(|k| k.research.iter().map(String::as_str)).collect();
    all.sort_unstable();
    let mut nodes: Vec<&str> = c.research.iter().map(|n| n.id.as_str()).collect();
    nodes.sort_unstable();
    assert_eq!(all, nodes, "the inner band's kit has all the research there is");
    let mut other = c.clone();
    other.playtest.clear();
    assert_eq!(other.fingerprint(), c.fingerprint());
}

#[test]
fn a_mod_replaces_a_bands_kit() {
    let c = with_kits(r#"[(band: "inner", materials: {"steel": 5})]"#).unwrap();
    let k = c.playtest.iter().position(|k| k.band == "inner").unwrap();
    assert_eq!(k, 2, "in its place");
    assert!(c.playtest[k].research.is_empty());
    assert_eq!(c.playtest[k].source.pack, "test_mod");
}

#[test]
fn a_bad_kit_says_where() {
    let bad = r#"[
        (band: "rim"),
        (band: "nowhere"),
        (band: "inner", research: ["no_such_node", "stingers"], materials: {"steel": 0, "gold": 1}, data: {"swarm": -1}),
    ]"#;
    let errors: Vec<(String, String)> = with_kits(bad)
        .unwrap_err()
        .into_iter()
        .filter(|p| p.severity == Severity::Error)
        .map(|p| (p.field.unwrap_or_default(), p.message))
        .collect();
    let has = |field: &str, words: &str| errors.iter().any(|(f, m)| f == field && m.contains(words));
    assert!(has("playtest[rim].band", "open from the start"), "{errors:?}");
    assert!(has("playtest[nowhere].band", "no band"), "{errors:?}");
    assert!(has("playtest[inner].research[0]", "unknown research"), "{errors:?}");
    assert!(has("playtest[inner].research[1]", "requires chitin_plating"), "{errors:?}");
    assert!(has("playtest[inner].materials", "`steel`: an amount above 0"), "{errors:?}");
    assert!(has("playtest[inner].materials", "unknown material `gold`"), "{errors:?}");
    assert!(has("playtest[inner].data", "`swarm`"), "{errors:?}");
    let twice = Pack::from_files([("pack.ron", MOD_MANIFEST), ("playtest.ron", r#"[(band: "outer"), (band: "outer")]"#)]).unwrap_err();
    assert!(twice.iter().any(|p| p.message.contains("two kits")), "{twice:?}");
}
