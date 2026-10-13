use std::collections::BTreeSet;

use data::Pack;

use super::*;
use crate::{load_content, pack_dirs};

/// The base pack's content, loaded like the CLI does; it must load without warnings.
fn base() -> Content {
    let (c, warnings) = load_content(&pack_dirs(&[])).expect("base pack loads");
    assert_eq!(warnings, 0, "the base pack has content warnings");
    c
}

fn page<'a>(site: &'a Site, path: &str) -> &'a str {
    std::str::from_utf8(
        site.files
            .get(path)
            .unwrap_or_else(|| panic!("no page {path}")),
    )
    .unwrap()
}

/// Values of `name="..."` attributes and `url(...)` references in a page.
fn values<'a>(text: &'a str, open: &str, close: char) -> Vec<&'a str> {
    text.match_indices(open)
        .map(|(k, _)| {
            let v = &text[k + open.len()..];
            &v[..v.find(close).unwrap_or(v.len())]
        })
        .collect()
}

fn resolve(dir: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    for seg in rel.split('/') {
        match seg {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// Every `href`, `url(...)` and fragment in every page and the stylesheet points at a generated file (and an
/// element with that id); nothing points outside the site.
fn assert_links_resolve(site: &Site) {
    let ids_of = |text: &str| -> BTreeSet<String> {
        values(text, " id=\"", '"')
            .into_iter()
            .map(String::from)
            .collect()
    };
    for (path, bytes) in &site.files {
        if !(path.ends_with(".html") || path.ends_with(".css")) {
            continue;
        }
        let text = std::str::from_utf8(bytes).unwrap();
        let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
        let ids = ids_of(text);
        let mut targets = values(text, "href=\"", '"');
        targets.extend(
            values(text, "url(", ')')
                .into_iter()
                .map(|t| t.trim_matches('"')),
        );
        assert!(
            !targets.is_empty() || path.ends_with(".css"),
            "{path}: no links at all"
        );
        for t in targets {
            assert!(!t.contains(':'), "{path}: link out of the site: {t}");
            let (file, frag) = t.split_once('#').unwrap_or((t, ""));
            let target = (!file.is_empty()).then(|| {
                let resolved = resolve(dir, file);
                site.files
                    .get(&resolved)
                    .unwrap_or_else(|| panic!("{path}: broken link {t}"))
            });
            if !frag.is_empty() {
                let target_ids =
                    target.map_or_else(|| ids.clone(), |t| ids_of(std::str::from_utf8(t).unwrap()));
                assert!(
                    target_ids.contains(frag),
                    "{path}: no element #{frag} for {t}"
                );
            }
        }
    }
}

#[test]
fn base_wiki_generates_without_warnings() {
    let site = generate(&base());
    assert_eq!(site.warnings, Vec::<String>::new());
    assert!(site.files.contains_key("index.html") && site.files.contains_key("style.css"));
    assert!(site.files.keys().filter(|p| p.ends_with(".html")).count() > 100);
}

#[test]
fn every_content_item_has_a_page() {
    let c = base();
    let site = generate(&c);
    let mut want: Vec<String> = KINDS.iter().map(|(k, _)| list_path(*k)).collect();
    want.extend(c.stats.iter().map(|e| item_path(Kind::Stat, &e.id)));
    want.extend(
        c.categories
            .iter()
            .map(|e| item_path(Kind::Category, &e.id)),
    );
    want.extend(c.factions.iter().map(|e| item_path(Kind::Faction, &e.id)));
    want.extend(c.materials.iter().map(|e| item_path(Kind::Material, &e.id)));
    want.extend(c.parts.iter().map(|e| item_path(Kind::Part, &e.id)));
    want.extend(c.frames.iter().map(|e| item_path(Kind::Frame, &e.id)));
    want.extend(c.ships.iter().map(|e| item_path(Kind::Ship, &e.id)));
    want.extend(
        c.behavior_nodes
            .iter()
            .map(|e| item_path(Kind::BehaviorNode, &e.id)),
    );
    want.extend(c.behaviors.iter().map(|e| item_path(Kind::Behavior, &e.id)));
    want.extend(
        c.wiki
            .values()
            .filter(|p| p.kind == Kind::Page)
            .map(|p| item_path(Kind::Page, &p.id)),
    );
    for path in &want {
        assert!(site.files.contains_key(path), "missing page {path}");
    }
    // Nothing else but the index, the stylesheet and the fonts.
    let extra: Vec<&String> = site
        .files
        .keys()
        .filter(|p| {
            !want.contains(p) && *p != "index.html" && *p != "style.css" && !p.starts_with("fonts/")
        })
        .collect();
    assert!(extra.is_empty(), "unexpected files {extra:?}");
}

#[test]
fn every_internal_link_resolves() {
    assert_links_resolve(&generate(&base()));
}

#[test]
fn output_is_byte_identical_on_two_runs() {
    let a = generate(&base());
    let b = generate(&base());
    assert!(a.files == b.files, "two runs differ");
}

#[test]
fn pages_link_both_ways() {
    let site = generate(&base());
    let has = |path: &str, needle: &str| {
        assert!(page(&site, path).contains(needle), "{path} lacks {needle}")
    };
    // A part lists the ships that use it, a ship its parts.
    has(
        "part/pulse_laser.html",
        "href=\"../ship/halvex_runner.html\"",
    );
    has(
        "ship/halvex_runner.html",
        "href=\"../part/pulse_laser.html\"",
    );
    // A material lists the parts made from it and the ships using it; a hull part its materials.
    has(
        "material/composite.html",
        "href=\"../part/armor_block.html\"",
    );
    has(
        "material/composite.html",
        "href=\"../ship/halvex_runner.html\"",
    );
    has(
        "part/armor_block.html",
        "href=\"../material/composite.html\"",
    );
    // A behavior lists its nodes, a node the behaviors using it; ships and factions their behavior.
    has(
        "behavior/raider_basic.html",
        "href=\"../behavior_node/orbit.html\"",
    );
    has(
        "behavior_node/orbit.html",
        "href=\"../behavior/raider_basic.html\"",
    );
    has(
        "behavior/skirmisher.html",
        "href=\"../behavior_node/closing_in.html\"",
    );
    has(
        "faction/automatons.html",
        "href=\"../behavior/raider_basic.html\"",
    );
    has(
        "behavior/raider_basic.html",
        "href=\"../ship/automaton_raider.html\"",
    );
    // Stats link to their page and back; bonuses name their targets; text links and prose become hyperlinks.
    has(
        "part/fission_reactor.html",
        "href=\"../stat/power_output.html\"",
    );
    has(
        "stat/power_output.html",
        "href=\"../part/fission_reactor.html\"",
    );
    has("part/capacitor.html", "href=\"../part/beam_laser.html\"");
    has("part/beam_laser.html", "href=\"../part/capacitor.html\"");
    has("part/fission_reactor.html", "<h2>Keeping it cool</h2>");
    has("part/radiator.html", "Mentioned on");
    has("page/drydock_rules.html", "<h1>Drydock rules</h1>");
}

#[test]
fn locked_content_is_marked() {
    let site = generate(&base());
    for path in [
        "part/automaton_cannon.html",
        "part/automaton_core.html",
        "material/automaton_plate.html",
    ] {
        assert!(
            page(&site, path).contains("class=\"badge locked\""),
            "{path} is not marked locked"
        );
    }
    assert!(
        !page(&site, "part/pulse_laser.html")
            .contains("<h1>Pulse laser <span class=\"badge locked\"")
    );
    assert!(
        page(&site, "parts.html").contains("Automaton cannon</a> <span class=\"badge locked\"")
    );
}

#[test]
fn drawings_are_vector_with_the_pack_icons() {
    let site = generate(&base());
    let ship = page(&site, "ship/halvex_runner.html");
    // The icons it shows are page symbols with prefixed ids; the hull glow uses the player's teal.
    assert!(ship.contains("<symbol id=\"icon-pulse-laser\" viewBox=\"0 0 20 20\">"));
    assert!(ship.contains("filter=\"url(#icon-pulse-laser-gl)\""));
    assert!(ship.contains("<use href=\"#icon-torch-drive\""));
    let glow = |key: &str| {
        format!(
            "<g fill=\"none\" stroke=\"{}\" stroke-linejoin",
            Style::embedded().glow(key)
        )
    };
    assert!(ship.contains(&glow("player")));
    assert!(page(&site, "ship/automaton_raider.html").contains(&glow("enemy")));
    // Icons the page does not show stay out of it.
    assert!(!page(&site, "part/radiator.html").contains("icon-torch-drive"));
    // No raster images and no scripts anywhere.
    for (path, bytes) in &site.files {
        if path.ends_with(".html") {
            let text = std::str::from_utf8(bytes).unwrap();
            assert!(
                !text.contains("<img") && !text.contains("<script") && !text.contains("<image"),
                "{path}"
            );
        }
    }
}

/// A mod with markup characters in every kind of text, and a stat label it renames.
fn tricky_content() -> Content {
    let base = data::fs::load_pack_dir(&pack_dirs(&[])[0]).unwrap();
    let m = Pack::from_files([
        ("pack.ron", r#"(id: "tricky", name: "Tricky <mod>", version: "1 & 2", schema_version: 1, depends: ["base"])"#),
        (
            "stats.ron",
            r#"[(id: "mass", name: "Heft <t>", summary: "Weight & \"more\".", unit: "kg<", decimals: 2, better: Lower)]"#,
        ),
        (
            "parts/p.ron",
            r#"(id: "tricky_part", name: "<Tricky & \"Part\">", summary: "Uses <b>bold</b> & [[part:radiator|<i>rad</i>]].",
                description: "`<code>` and **<strong>**", lore: "'quoted' <lore>", tags: ["odd"],
                category: "power", size: (1, 1), build_points: 1, mass: 4, hp: 1, power: (output: 1))"#,
        ),
        (
            "ships/s.ron",
            r#"(id: "tricky_ship", name: "Ship <&>", summary: "A <ship>.", faction: "ship_mind", size: (2, 1), parts: [
                (part: "logic_core", x: 0, y: 0), (part: "tricky_part", x: 1, y: 0),
            ])"#,
        ),
        ("wiki/page/tricky_page.md", "# Title <script>alert(1)</script>\n\nText & <b>x</b>, see [[ship:tricky_ship]].\n"),
        ("wiki/part/tricky_part.md", "Prose with <div> & [[stat:mass]].\n"),
    ])
    .unwrap();
    Content::load(&[base, m]).unwrap()
}

#[test]
fn html_is_escaped() {
    let site = generate(&tricky_content());
    assert_eq!(site.warnings, Vec::<String>::new());
    assert_links_resolve(&site);
    for (path, bytes) in site.files.iter().filter(|(p, _)| !p.starts_with("fonts/")) {
        let text = std::str::from_utf8(bytes).unwrap();
        for raw in [
            "<Tricky",
            "<b>bold",
            "<i>rad",
            "<code>`",
            "<strong>**",
            "<lore>",
            "<script",
            "<ship>",
            "<div>",
            "<t>",
            "<mod>",
            "<&>",
            "kg<",
        ] {
            assert!(!text.contains(raw), "{path} contains raw `{raw}`");
        }
    }
    let part = page(&site, "part/tricky_part.html");
    assert!(part.contains("<h1>&lt;Tricky &amp; &quot;Part&quot;&gt;</h1>"));
    assert!(
        part.contains("<title>&lt;Tricky &amp; &quot;Part&quot;&gt; · Space Game wiki</title>")
    );
    assert!(part.contains("Uses &lt;b&gt;bold&lt;/b&gt; &amp; <a href=\"../part/radiator.html\">&lt;i&gt;rad&lt;/i&gt;</a>."));
    assert!(part.contains("<code>&lt;code&gt;</code> and <strong>&lt;strong&gt;</strong>"));
    assert!(part.contains("&#39;quoted&#39; &lt;lore&gt;"));
    assert!(part.contains(
        "Prose with &lt;div&gt; &amp; <a href=\"../stat/mass.html\">Heft &lt;t&gt;</a>."
    ));
    // Stat labels and units come from stats.ron, so a mod's relabelled stat shows everywhere.
    assert!(part.contains(">Heft &lt;t&gt;</a></th><td>4 kg&lt;</td>"));
    assert!(
        page(&site, "part/fission_reactor.html")
            .contains(">Heft &lt;t&gt;</a></th><td>4 kg&lt;</td>")
    );
    assert!(
        page(&site, "page/tricky_page.html")
            .contains("<h1>Title &lt;script&gt;alert(1)&lt;/script&gt;</h1>")
    );
    assert!(page(&site, "index.html").contains("Tricky &lt;mod&gt; <code>tricky</code> 1 &amp; 2"));
    assert!(page(&site, "ships.html").contains("aria-label=\"Ship &lt;&amp;&gt;\""));
}

#[test]
fn write_replaces_an_earlier_wiki_and_refuses_other_directories() {
    let dir = std::env::temp_dir().join(format!("content-tool-wiki-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let site = |paths: &[&str]| Site {
        files: paths
            .iter()
            .map(|p| (p.to_string(), p.as_bytes().to_vec()))
            .collect(),
        warnings: Vec::new(),
    };
    write(&site(&["index.html", "part/a.html", "part/b.html"]), &dir).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("part/b.html")).unwrap(),
        "part/b.html"
    );
    // A second run removes what it no longer makes (and the emptied directory), and nothing else.
    std::fs::write(dir.join("notes.txt"), "mine").unwrap();
    write(&site(&["index.html", "ship/c.html"]), &dir).unwrap();
    assert!(!dir.join("part").exists());
    assert!(dir.join("ship/c.html").exists() && dir.join("notes.txt").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join(MANIFEST)).unwrap(),
        "index.html\nship/c.html\n"
    );
    // A directory with other files and no manifest is left alone.
    std::fs::remove_file(dir.join(MANIFEST)).unwrap();
    let err = write(&site(&["index.html"]), &dir).unwrap_err();
    assert!(err.contains("not empty"), "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}
