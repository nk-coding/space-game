use std::collections::BTreeSet;
use std::path::Path;

use data::Pack;

use super::*;
use crate::{load_content, pack_dirs};

/// The base pack's content, loaded like the CLI does; it must load without warnings.
fn base() -> Content {
    let (c, warnings) = load_content(&pack_dirs(&[])).expect("base pack loads");
    assert_eq!(warnings, 0, "the base pack has content warnings");
    c
}

/// The repository's wiki directory.
fn wiki_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../wiki")
}

/// The generated files for some content, without hand-written pages (their problems are not looked at).
fn gen_files(c: &Content) -> Site {
    generate(c, &[])
}

/// A generated page by its served path (`parts/pulse_laser.html`, a list `research.html` or `research/index.html`,
/// the overview `index.html`), its blocks flattened for the needles below: headings without their id and anchor,
/// the block wrappers and table bodies left out, no blank lines.
fn page(site: &Site, path: &str) -> String {
    let file = match path.strip_suffix(".html") {
        Some("index") => "reference/index.md".to_string(),
        Some(p) if !p.contains('/') => format!("{p}/index.md"),
        Some(p) => format!("{p}.md"),
        None => path.to_string(),
    };
    let text = std::str::from_utf8(
        site.files
            .get(&file)
            .unwrap_or_else(|| panic!("no page {file}")),
    )
    .unwrap();
    flatten(text)
}

fn flatten(text: &str) -> String {
    let mut s = String::new();
    let mut rest = text;
    // Headings: `<h2 id="x" tabindex="-1" v-pre>Title <a class="header-anchor" …>&#8203;</a></h2>` → `<h2>Title</h2>`.
    while let Some(k) = rest.find("<h") {
        let level = rest[k + 2..].chars().next().filter(char::is_ascii_digit);
        let (Some(_), Some(open_end)) = (level, rest[k..].find('>')) else {
            s.push_str(&rest[..k + 2]);
            rest = &rest[k + 2..];
            continue;
        };
        s.push_str(&rest[..k + 3]);
        s.push('>');
        rest = &rest[k + open_end + 1..];
        if let Some(a) = rest.find(" <a class=\"header-anchor\"")
            && let Some(e) = rest[a..].find("</a>")
            && !rest[..a].contains('\n')
        {
            s.push_str(&rest[..a]);
            rest = &rest[a + e + 4..];
        }
    }
    s.push_str(rest);
    s.replace("<ScrollBox x>\n", "")
        .replace("</ScrollBox>\n", "")
        .replace("<div class=\"sg-section\" v-pre>\n", "")
        .replace("<tbody>", "")
        .replace("</tbody>", "")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
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

/// The generated file a served path is: `parts/x.html` → `parts/x.md`.
fn served(site: &Site, path: &str) -> Option<String> {
    let md = format!("{}.md", path.strip_suffix(".html")?);
    site.files.contains_key(&md).then_some(md)
}

/// Values of `name="..."` attributes in a text.
fn values<'a>(text: &'a str, open: &str, close: char) -> Vec<&'a str> {
    text.match_indices(open)
        .map(|(k, _)| {
            let v = &text[k + open.len()..];
            &v[..v.find(close).unwrap_or(v.len())]
        })
        .collect()
}

/// Every link of every generated page, and of the HTML in `refs.json` and `ui.json`, points at a generated page (and
/// an element with that id); `#icon-…` references at the sprite's symbols, arrows at the theme's markers; `url(...)`
/// in `tokens.css` at a generated file. Nothing points outside the site.
fn assert_links_resolve(site: &Site) {
    let sprite = std::str::from_utf8(&site.files["sprite.svg"]).unwrap();
    let symbols: BTreeSet<&str> = values(sprite, " id=\"", '"').into_iter().collect();
    // The theme defines these markers once for every page (`wiki/.vitepress/theme/Layout.vue`).
    let markers = ["arrow", "arrow-teal", "arrow-gold", "arrow-coral", "arrow-violet"];
    let check = |from: &str, dir: &str, text: &str| {
        let ids: BTreeSet<&str> = values(text, " id=\"", '"').into_iter().collect();
        for t in values(text, "href=\"", '"') {
            assert!(!t.contains("://") && !t.starts_with("//"), "{from}: link out of the site: {t}");
            if let Some(id) = t.strip_prefix('#') {
                assert!(
                    ids.contains(id) || symbols.contains(id),
                    "{from}: no element #{id}"
                );
                continue;
            }
            let rel = t.strip_prefix(ROOT).unwrap_or(t);
            let (file, frag) = rel.split_once('#').unwrap_or((rel, ""));
            let base = if t.starts_with(ROOT) { "" } else { dir };
            let target = served(site, &resolve(base, file)).unwrap_or_else(|| panic!("{from}: broken link {t}"));
            if !frag.is_empty() {
                let text = std::str::from_utf8(&site.files[&target]).unwrap();
                assert!(text.contains(&format!(" id=\"{frag}\"")), "{from}: no element #{frag} for {t}");
            }
        }
        for m in values(text, "url(#", ')') {
            assert!(markers.contains(&m) || symbols.contains(m), "{from}: no marker or filter #{m}");
        }
    };
    for (path, bytes) in &site.files {
        let text = std::str::from_utf8(bytes).unwrap_or("");
        let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
        if path.ends_with(".md") {
            check(path, dir, text);
        } else if path.ends_with(".json") {
            // JSON escapes quotes: read it back and check every string.
            fn strings<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
                match v {
                    Value::String(s) => out.push(s),
                    Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
                    Value::Object(o) => o.values().for_each(|x| strings(x, out)),
                    _ => {}
                }
            }
            let v: Value = serde_json::from_str(text).unwrap();
            let mut all = Vec::new();
            strings(&v, &mut all);
            for s in all {
                assert!(!s.contains("href=\"../"), "{path}: a relative link in shared HTML: {s}");
                check(path, "", s);
            }
        } else if path.ends_with(".css") {
            for u in values(text, "url(\"", '"') {
                let file = resolve(dir, u);
                assert!(site.files.contains_key(&file), "{path}: no file {u}");
            }
        }
    }
}

#[test]
fn every_kind_has_a_section() {
    for kind in Kind::ALL {
        assert!(KINDS.iter().any(|(k, _)| *k == kind), "kind {kind} has no section in the wiki");
    }
    assert_eq!(KINDS.len(), Kind::ALL.len());
}

/// The completeness check of the whole wiki: the repository's hand-written pages (`wiki/`) against the base content.
/// Every reference resolves, every rule example checks, every page is in the navigation, nothing is raster; and the
/// base content generates without warnings.
#[test]
fn the_wiki_is_complete_and_consistent() {
    let hand = hand::read(&wiki_dir()).expect("the wiki directory reads");
    assert!(hand.iter().any(|f| f.path == "nav.json"), "no wiki/nav.json");
    let site = generate(&base(), &hand);
    assert_eq!(site.warnings, Vec::<String>::new());
    assert_eq!(site.errors, Vec::<String>::new(), "run `content-tool wiki-data wiki` for the details");
    assert!(site.files.keys().filter(|p| p.ends_with(".md")).count() > 100);
}

#[test]
fn every_content_item_has_a_page() {
    let c = base();
    let site = gen_files(&c);
    let mut want: Vec<(Kind, String)> = Vec::new();
    want.extend(c.stats.iter().map(|e| (Kind::Stat, e.id.clone())));
    want.extend(c.categories.iter().map(|e| (Kind::Category, e.id.clone())));
    want.extend(c.factions.iter().map(|e| (Kind::Faction, e.id.clone())));
    want.extend(c.materials.iter().map(|e| (Kind::Material, e.id.clone())));
    want.extend(c.parts.iter().map(|e| (Kind::Part, e.id.clone())));
    want.extend(c.frames.iter().map(|e| (Kind::Frame, e.id.clone())));
    want.extend(c.ships.iter().map(|e| (Kind::Ship, e.id.clone())));
    want.extend(c.behavior_nodes.iter().map(|e| (Kind::BehaviorNode, e.id.clone())));
    want.extend(c.behaviors.iter().map(|e| (Kind::Behavior, e.id.clone())));
    want.extend(c.families.iter().map(|e| (Kind::Family, e.id.clone())));
    want.extend(c.research.iter().map(|e| (Kind::Research, e.id.clone())));
    want.extend(c.starts.iter().map(|e| (Kind::Start, e.id.clone())));
    want.extend(c.controls.iter().map(|e| (Kind::Control, e.id.clone())));
    want.extend(c.voyages.iter().map(|e| (Kind::Voyage, e.id.clone())));
    want.extend(c.encounters.iter().map(|e| (Kind::Encounter, e.id.clone())));
    want.extend(
        c.wiki
            .values()
            .filter(|p| p.kind == Kind::Page)
            .map(|p| (Kind::Page, p.id.clone())),
    );
    let sidebar = std::str::from_utf8(&site.files["sidebar.json"]).unwrap();
    let refs: Value = serde_json::from_slice(&site.files["refs.json"]).unwrap();
    let mut pages: Vec<String> = Vec::new();
    for (kind, id) in &want {
        let path = format!("{}/{id}.md", kind_dir(*kind));
        assert!(site.files.contains_key(&path), "missing page {path}");
        assert_ne!(id, "index", "an entry named `index` would be its kind's list");
        // In the sidebar, and something hand-written pages can reference.
        assert!(
            sidebar.contains(&format!("\"/{}/{id}\"", kind_dir(*kind))),
            "the sidebar lacks {path}"
        );
        assert!(
            refs["entries"].get(format!("{}:{id}", kind.key())).is_some(),
            "refs.json lacks {kind}:{id}"
        );
        pages.push(path);
    }
    // Every kind has a list page, in the sidebar and on the reference overview.
    for (kind, _) in KINDS {
        pages.push(format!("{}/index.md", kind_dir(kind)));
        assert!(sidebar.contains(&format!("\"/{}/\"", kind_dir(kind))));
        assert!(page(&site, "index.html").contains(&format!("href=\"../{}\"", list_path(kind))));
    }
    pages.push("reference/index.md".into());
    // Nothing else but the data files, the sprite, the tokens and the fonts.
    let data = ["refs.json", "ui.json", "sidebar.json", "sprite.svg", "tokens.css"];
    let extra: Vec<&String> = site
        .files
        .keys()
        .filter(|p| !pages.contains(p) && !data.contains(&p.as_str()) && !p.starts_with("fonts/"))
        .collect();
    assert!(extra.is_empty(), "unexpected files {extra:?}");
}

#[test]
fn every_internal_link_resolves() {
    assert_links_resolve(&gen_files(&base()));
}

#[test]
fn output_is_byte_identical_on_two_runs() {
    let hand = hand::read(&wiki_dir()).unwrap();
    let a = generate(&base(), &hand);
    let b = generate(&base(), &hand);
    assert!(a.files == b.files, "two runs differ");
    for path in [
        "families/logic.md",
        "research/mind_expansion_1.md",
        "starts/rim_awakening.md",
        "voyages/yard_approach.md",
        "encounters/the_warden.md",
        "controls/thrust_forward.md",
    ] {
        assert!(a.files.contains_key(path), "no {path}");
    }
}

/// Generated pages are Markdown made of HTML blocks: front matter with the title, then blocks that Vue leaves
/// alone and that hold no blank line (which would end the block).
#[test]
fn pages_are_html_blocks_the_site_leaves_alone() {
    let site = gen_files(&base());
    for (path, bytes) in site.files.iter().filter(|(p, _)| p.ends_with(".md")) {
        let text = std::str::from_utf8(bytes).unwrap();
        let body = text
            .strip_prefix("---\ntitle: \"")
            .and_then(|t| t.split_once("\n---\n\n"))
            .map(|(_, b)| b)
            .unwrap_or_else(|| panic!("{path}: no front matter with a title"));
        for block in body.split("\n\n").filter(|b| !b.trim().is_empty()) {
            let first = block.lines().next().unwrap();
            let component = first.starts_with('<') && first[1..].starts_with(|c: char| c.is_ascii_uppercase());
            assert!(
                first.contains(" v-pre>") || component,
                "{path}: a block Vue would read as a template: {first}"
            );
        }
    }
}

#[test]
fn pages_link_both_ways() {
    let site = gen_files(&base());
    let has = |path: &str, needle: &str| {
        assert!(page(&site, path).contains(needle), "{path} lacks {needle}")
    };
    // A part lists the ships that use it, a ship its parts.
    has(
        "parts/pulse_laser.html",
        "href=\"../ships/halvex_runner.html\"",
    );
    has(
        "ships/halvex_runner.html",
        "href=\"../parts/pulse_laser.html\"",
    );
    // A material lists the parts made from it and the ships using it; a hull part its materials.
    has(
        "materials/composite.html",
        "href=\"../parts/armor_block.html\"",
    );
    has(
        "materials/composite.html",
        "href=\"../ships/halvex_runner.html\"",
    );
    has(
        "parts/armor_block.html",
        "href=\"../materials/composite.html\"",
    );
    // A behavior lists its nodes, a node the behaviors using it; ships and factions their behavior.
    has(
        "behaviors/raider_basic.html",
        "href=\"../behavior_nodes/orbit.html\"",
    );
    has(
        "behavior_nodes/orbit.html",
        "href=\"../behaviors/raider_basic.html\"",
    );
    has(
        "behaviors/skirmisher.html",
        "href=\"../behavior_nodes/closing_in.html\"",
    );
    has(
        "factions/automatons.html",
        "href=\"../behaviors/raider_basic.html\"",
    );
    has(
        "behaviors/raider_basic.html",
        "href=\"../ships/automaton_raider.html\"",
    );
    // Stats link to their page and back; bonuses name their targets; text links and prose become hyperlinks.
    has(
        "parts/fission_reactor.html",
        "href=\"../stats/power_output.html\"",
    );
    has(
        "stats/power_output.html",
        "href=\"../parts/fission_reactor.html\"",
    );
    has("parts/capacitor.html", "href=\"../parts/beam_laser.html\"");
    has("parts/beam_laser.html", "href=\"../parts/capacitor.html\"");
    has("parts/fission_reactor.html", "<h2>Keeping it cool</h2>");
    has("parts/radiator.html", "Mentioned on");
    has("pages/drydock_rules.html", "<h1>Drydock rules</h1>");
}

#[test]
fn career_pages_link_both_ways() {
    let site = gen_files(&base());
    let has = |path: &str, needle: &str| {
        assert!(page(&site, path).contains(needle), "{path} lacks {needle}")
    };
    let lacks = |path: &str, needle: &str| {
        assert!(!page(&site, path).contains(needle), "{path} has {needle}")
    };
    // A family lists its nodes and what feeds it (parts and armour materials by their salvage); the nodes, parts
    // and materials link back.
    for node in ["tamed_core", "mind_expansion_1", "mind_expansion_2"] {
        has(
            "families/logic.html",
            &format!("href=\"../research/{node}.html\""),
        );
        has(
            &format!("research/{node}.html"),
            "href=\"../families/logic.html\"",
        );
    }
    has("families/logic.html", "href=\"../parts/logic_core.html\"");
    has(
        "families/logic.html",
        "href=\"../parts/automaton_core.html\">Automaton core</a> <span class=\"badge locked\"",
    );
    has("families/hull.html", "href=\"../materials/steel.html\"");
    has(
        "families/hull.html",
        "href=\"../research/cruiser_drydock.html\"",
    );
    has(
        "families/shields.html",
        "<h2>Research</h2>\n<p class=\"none\">",
    );
    // A node: family, kind, cost in data and materials, what it needs, what needs it, what it unlocks.
    let node = "research/mind_expansion_1.html";
    has(node, "<dt>Kind</dt><dd><span title=");
    has(node, ">Refinement</span>");
    has(
        node,
        "10 <a href=\"../families/logic.html\">Logic and sensors</a> data<br>3 <a href=\"../materials/programmable_matter.html\">",
    );
    has(
        node,
        "<h2>Requires</h2>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../research/tamed_core.html\">",
    );
    has(
        node,
        "<h2>Required by</h2>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../research/mind_expansion_2.html\">",
    );
    has(node, "Mind capacity +1");
    has(node, "The ship-mind runs 1 more logic core at once.");
    has(
        "research/tamed_core.html",
        "<h2>Requires</h2>\n<p class=\"none\">",
    );
    has(
        "research/tamed_core.html",
        "href=\"../parts/automaton_core.html\">Automaton core</a> <span class=\"badge locked\"",
    );
    has(
        "research/composite_plating.html",
        "href=\"../materials/composite.html\"",
    );
    has(
        "research/cruiser_drydock.html",
        "href=\"../frames/cruiser.html\">Cruiser</a> <span class=\"badge locked\"",
    );
    // A node's texts link to entries, and those list it as mentioning them.
    has(
        "parts/automaton_core.html",
        "href=\"../research/tamed_core.html\"",
    );
    // A start: its ship drawn like a ship page and linked, the mind capacity with the research that adds to
    // it, the open voyages (linked), and what a new game has.
    let start = "starts/rim_awakening.html";
    has(start, "<figure class=\"hero\"");
    has(start, "<svg class=\"ship\"");
    has(start, "aria-label=\"Rim skiff\"");
    has(
        start,
        "<dt>Ship</dt><dd><a href=\"../ships/rim_skiff.html\">",
    );
    has(start, "<dt>Mind capacity</dt><dd>2 logic cores</dd>");
    has(start, "href=\"../research/mind_expansion_2.html\"");
    has(start, "(<strong>4</strong> in all)");
    has(
        start,
        "<h2>Open voyages</h2>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../voyages/yard_approach.html\">Shipyard approach</a>",
    );
    lacks(start, "halvex_drift.html");
    has(start, "href=\"../parts/pulse_laser.html\"");
    lacks(start, "href=\"../parts/beam_laser.html\"");
    // List pages: research grouped by family, families with counts, starts as cards.
    has("research.html", "href=\"../families/logic.html\"");
    has("research.html", "href=\"../research/cruiser_drydock.html\"");
    lacks("research.html", "href=\"../families/shields.html\"");
    has("families.html", "href=\"../families/utility.html\"");
    has("families.html", "3 research nodes");
    has(
        "starts.html",
        "<a class=\"card\" href=\"../starts/rim_awakening.html\">",
    );
}

#[test]
fn voyage_pages_link_both_ways() {
    let c = base();
    let site = gen_files(&c);
    let has = |path: &str, needle: &str| {
        assert!(page(&site, path).contains(needle), "{path} lacks {needle}")
    };
    let lacks = |path: &str, needle: &str| {
        assert!(!page(&site, path).contains(needle), "{path} has {needle}")
    };
    let goal = "<span class=\"badge goal\"";
    // A voyage: its numbers with units, its sites (encounters with what they hold and how many, the goal
    // marked), the start that opens it, the cleared line with its links, and the voyages it opens.
    let first = "voyages/yard_approach.html";
    has(first, "<dt>Band</dt><dd>rim</dd>");
    has(first, ">0.28, 0.66</span></dd>");
    has(first, "<dt>Radius</dt><dd>600 m</dd>");
    has(first, "<dt>Site spacing</dt><dd>400–700 m</dd>");
    has(first, "<dt>Expected length</dt><dd>6–8 min</dd>");
    has(first, "<dt>Sites</dt><dd>4</dd>");
    has(
        first,
        "<dt>Goal</dt><dd><a href=\"../encounters/raider_pair.html\">Raider pair</a></dd>",
    );
    has(
        first,
        "<dt>Open at the start of</dt><dd><a href=\"../starts/rim_awakening.html\">",
    );
    has(
        first,
        "<tr><td class=\"name\"><a href=\"../encounters/raider_pair.html\">Raider pair</a></td><td><a href=\"../ships/automaton_raider.html\">Automaton raider</a>&nbsp;×2</td><td>—</td><td class=\"num\">×2</td></tr>",
    );
    has(
        first,
        "<a href=\"../ships/sample_early.html\">Sample early</a>&nbsp;×1, 50% of parts gone",
    );
    has(
        first,
        &format!("href=\"../encounters/raider_pair.html\">Raider pair</a> {goal}"),
    );
    lacks(first, "href=\"../encounters/raider_pack.html\"");
    has(
        first,
        "<blockquote><p>The lanes around the shipyard are quiet again. Further out, the <a href=\"../voyages/halvex_drift.html\">Halvex Drift</a> is waiting.</p></blockquote>",
    );
    has(
        first,
        "<h2>Opens after</h2>\n<p class=\"none\">None: it is open at the start.</p>",
    );
    has(
        first,
        "<h2>Opens</h2>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../voyages/halvex_drift.html\">",
    );
    // The next voyage names it back (and the cleared line counts as a mention); the last opens nothing.
    has(
        "voyages/halvex_drift.html",
        "<h2>Opens after</h2>\n<p class=\"note\">It opens once the goal of each of these has been cleared.</p>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../voyages/yard_approach.html\">",
    );
    has(
        "voyages/halvex_drift.html",
        "<h2>Mentioned on</h2>\n<p><a href=\"../voyages/yard_approach.html\">",
    );
    lacks("voyages/halvex_drift.html", "Open at the start of");
    has(
        "voyages/wardens_gate.html",
        "<h2>Opens</h2>\n<p class=\"none\">None.</p>",
    );
    // An encounter: wake radius, its ships and wrecks drawn and linked with their counts, the wrecks' damage,
    // and the voyages that place it (how many sites, the goal marked).
    let warden = "encounters/the_warden.html";
    has(warden, "<dt>Wake radius</dt><dd>300 m</dd>");
    has(warden, "<dt>Ships</dt><dd>2–3</dd>");
    lacks(warden, "<dt>Wrecks</dt>");
    has(warden, "aria-label=\"Automaton warden\"");
    has(
        warden,
        "href=\"../ships/automaton_raider.html\">Automaton raider</a></td><td>",
    );
    has(warden, "<td class=\"num\">×1–2</td>");
    has(warden, "within 300 m of the site");
    has(warden, "<h2>Wrecks</h2>\n<p class=\"none\">None.</p>");
    has(
        warden,
        &format!(
            "<tr><td class=\"name\"><a href=\"../voyages/wardens_gate.html\">The Warden&#39;s gate</a> {goal}"
        ),
    );
    has(warden, "<td class=\"num\">1 site</td>");
    has(warden, "the shielded heart holds");
    let graveyard = "encounters/core_graveyard.html";
    has(graveyard, "<dt>Wrecks</dt><dd>4–6</dd>");
    has(
        graveyard,
        "<td>30% of parts gone</td><td class=\"num\">×4–6</td>",
    );
    has(
        "encounters/raider_pair.html",
        &format!(
            "<tr><td class=\"name\"><a href=\"../voyages/yard_approach.html\">Shipyard approach</a> {goal}"
        ),
    );
    has(
        "encounters/raider_pair.html",
        "<td class=\"num\">3 sites</td>",
    );
    has(
        "encounters/raider_pack.html",
        "href=\"../voyages/wardens_gate.html\"",
    );
    lacks(
        "encounters/raider_pack.html",
        "href=\"../voyages/yard_approach.html\"",
    );
    // A ship names the encounters it waits in, fighting or as a wreck; the encounters name it back.
    has(
        "ships/automaton_warden.html",
        "<h2>In encounters</h2>\n<ul>\n<li><a href=\"../encounters/the_warden.html\">The Warden</a>: ×1 fighting</li>",
    );
    has(
        "ships/automaton_raider.html",
        "<a href=\"../encounters/derelict_field.html\">Derelict field</a>: ×0–1 fighting; ×2–3 wrecked, 50% of parts gone",
    );
    has(
        "ships/sample_mid.html",
        "<a href=\"../encounters/precursor_wrecks.html\">Precursor wrecks</a>: ×1–2 wrecked, 40% of parts gone",
    );
    has(
        "encounters/precursor_wrecks.html",
        "href=\"../ships/sample_mid.html\"",
    );
    lacks("ships/rim_skiff.html", "In encounters");
    // List pages: voyages in load order with band, sites and length; encounters as a table.
    let list = page(&site, "voyages.html");
    let at: Vec<usize> = c
        .voyages
        .iter()
        .map(|v| {
            list.find(&format!("href=\"../voyages/{}.html\"", v.id))
                .unwrap_or_else(|| panic!("voyages.html lacks {}", v.id))
        })
        .collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "voyages out of order");
    has("voyages.html", "<span class=\"note\">rim band</span>");
    has(
        "voyages.html",
        "<td class=\"num\">4 sites</td><td class=\"num\">6–8 min</td>",
    );
    has(
        "encounters.html",
        "<th>Ships</th><th>Wrecks</th><th class=\"num\">Wake radius</th>",
    );
    has(
        "encounters.html",
        "<a href=\"../ships/automaton_hunter.html\">Automaton hunter</a>&nbsp;×2–3</td><td>—</td><td class=\"num\">300 m</td>",
    );
    has(
        "encounters.html",
        "<a href=\"../ships/automaton_raider.html\">Automaton raider</a>&nbsp;×4–6, 30% of parts gone",
    );
}

#[test]
fn locked_content_is_marked() {
    let site = gen_files(&base());
    for path in [
        "parts/automaton_cannon.html",
        "parts/automaton_core.html",
        "materials/automaton_plate.html",
    ] {
        assert!(
            page(&site, path).contains("class=\"badge locked\""),
            "{path} is not marked locked"
        );
    }
    let summary = |path: &str| {
        let p = page(&site, path);
        p.lines().find(|l| l.starts_with("<p class=\"summary\">")).unwrap_or_default().to_string()
    };
    assert!(!summary("parts/pulse_laser.html").contains("class=\"badge locked\""));
    assert!(
        page(&site, "parts.html").contains("Automaton cannon</a> <span class=\"badge locked\"")
    );
    // Frames too (GDR-0063): on their page and in the list.
    assert!(summary("frames/cruiser.html").contains("class=\"badge locked\""));
    assert!(!page(&site, "frames/frigate.html").contains("class=\"badge locked\""));
    assert!(page(&site, "frames.html").contains("Capital</a> <span class=\"badge locked\""));
}

#[test]
fn parts_materials_and_frames_show_salvage_and_unlocks() {
    let site = gen_files(&base());
    let has = |path: &str, needle: &str| {
        assert!(page(&site, path).contains(needle), "{path} lacks {needle}")
    };
    let lacks = |path: &str, needle: &str| {
        assert!(!page(&site, path).contains(needle), "{path} has {needle}")
    };
    let pm = "<a href=\"../materials/programmable_matter.html\">Programmable matter</a>";
    // A part's salvage block: data for its family and materials when intact, the fragment when destroyed.
    let core = "parts/automaton_core.html";
    has(
        core,
        &format!(
            "<tr><th>Intact part</th><td>4 <a href=\"../families/logic.html\">Logic and sensors</a> data, 1 {pm}</td></tr>"
        ),
    );
    has(
        core,
        &format!("<tr><th>Destroyed part</th><td>leaves a fragment of 1 {pm}</td></tr>"),
    );
    has(
        "parts/pulse_laser.html",
        "<td>3 <a href=\"../families/energy.html\">Energy weapons</a> data</td></tr>\n<tr><th>Destroyed part</th><td>leaves nothing</td>",
    );
    // Hull parts give data by their material, per cell of hull area (a 1×3 slope is 1.5 cells).
    let slope = "parts/slope_1x3.html";
    has(slope, "this part is 1.5 cells.");
    has(
        slope,
        "<tr><th><a href=\"../materials/composite.html\">Composite</a> <span class=\"badge locked\"",
    );
    has(
        slope,
        "<td>2.25 <a href=\"../families/hull.html\">Hull and armour</a> data</td>",
    );
    // "Unlocked by": the nodes that unlock a locked part, material or frame, or a note that none does yet;
    // entries a new game has show none.
    has(
        core,
        "<h2>Unlocked by</h2>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../research/tamed_core.html\">",
    );
    has(
        "parts/automaton_cannon.html",
        "href=\"../research/automaton_cannon.html\"",
    );
    has(
        "parts/beam_laser.html",
        "href=\"../research/beam_laser.html\"",
    );
    lacks("parts/pulse_laser.html", "<h2>Unlocked by</h2>");
    has(
        "materials/composite.html",
        "<h2>Unlocked by</h2>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../research/composite_plating.html\">",
    );
    has(
        "materials/reactive.html",
        "<h2>Unlocked by</h2>\n<p class=\"none\">No research unlocks it yet.</p>",
    );
    lacks("materials/steel.html", "<h2>Unlocked by</h2>");
    has(
        "frames/cruiser.html",
        "<h2>Unlocked by</h2>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../research/cruiser_drydock.html\">",
    );
    has("frames/capital.html", "No research unlocks it yet.");
    lacks("frames/frigate.html", "<h2>Unlocked by</h2>");
    // An armour material's salvage per cell; programmable matter: the parts that give it and the research
    // that costs it.
    has(
        "materials/composite.html",
        "<tr><th>Per cell of hull area</th><td>1.5 <a href=\"../families/hull.html\">Hull and armour</a> data</td></tr>",
    );
    let matter = "materials/programmable_matter.html";
    lacks(matter, "<h2>Salvage</h2>");
    has(matter, "<h2>From salvage</h2>");
    has(
        matter,
        "href=\"../parts/logic_core.html\">Logic core</a></td><td>1 per intact part, 1 per fragment of a destroyed one</td>",
    );
    has(matter, "href=\"../parts/automaton_core.html\"");
    has(matter, "<h2>Research that costs it</h2>");
    for node in ["tamed_core", "mind_expansion_1", "mind_expansion_2"] {
        has(matter, &format!("href=\"../research/{node}.html\""));
    }
    lacks(matter, "href=\"../research/beam_laser.html\"");
    // The start's ship names the start.
    has(
        "ships/rim_skiff.html",
        "<dt>Start ship of</dt><dd><a href=\"../starts/rim_awakening.html\">",
    );
}

#[test]
fn drawings_are_vector_with_the_pack_icons() {
    let site = gen_files(&base());
    let sprite = std::str::from_utf8(&site.files["sprite.svg"]).unwrap();
    // The icons are the sprite's symbols with prefixed ids; pages draw them with `<use>`.
    assert!(sprite.contains("<symbol id=\"icon-pulse-laser\" viewBox=\"0 0 20 20\">"));
    assert!(sprite.contains("filter=\"url(#icon-pulse-laser-gl)\""));
    let ship = page(&site, "ships/halvex_runner.html");
    assert!(ship.contains("<use href=\"#icon-torch-drive\""));
    // Ships are drawn as in flight (GDR-0069): machines as their housings, the ship drawings, on hull-toned cells;
    // an engine's drawing reaches past its stern (the torch drive's half a cell: 2 × 2 cells, 2.5 drawn).
    assert!(sprite.contains("<symbol id=\"ship-torch-drive\" viewBox="));
    assert!(ship.contains("<use href=\"#ship-torch-drive\" x=\"-1\" y=\"-1\" width=\"2\" height=\"2.5\""));
    // The hull glow uses the faction's colour: the player's teal, the enemies' coral.
    let glow = |key: &str| {
        format!(
            "<g fill=\"none\" stroke=\"{}\" stroke-linejoin",
            Style::embedded().glow(key)
        )
    };
    assert!(ship.contains(&glow("player")));
    assert!(page(&site, "ships/automaton_raider.html").contains(&glow("enemy")));
    // A part's technical drawing marks what it does: an engine its thrust and plume, a gun its line of fire, a
    // reactor the cells its blast reaches, an emitter its bubble.
    let thruster = page(&site, "parts/thruster.html");
    assert!(thruster.contains("<svg class=\"blueprint\"") && thruster.contains("class=\"bp-thrust\""));
    assert!(thruster.contains("class=\"bp-plume\"") && thruster.contains("PLUME LENGTH 3 m"));
    assert!(page(&site, "parts/pulse_laser.html").contains("class=\"bp-fire\""));
    assert!(page(&site, "parts/fission_reactor.html").contains("class=\"bp-blast\""));
    assert!(page(&site, "parts/shield_emitter.html").contains("class=\"bp-shield\""));
    assert!(page(&site, "parts/rcs_pod.html").contains("class=\"bp-nozzle\""));
    // Rule lists are drawn as the GRAPH view draws them (each chip naming its node), their text linked.
    let raider = page(&site, "behaviors/raider_basic.html");
    assert!(raider.contains("<svg class=\"rule-graph\"") && raider.contains("<g><title>orbit</title>"));
    assert!(raider.contains("<a class=\"act\" href=\"../behavior_nodes/orbit.html\">orbit</a>"));
    // No links inside drawings: the site cannot prefetch an SVG link.
    for (path, bytes) in &site.files {
        let text = std::str::from_utf8(bytes).unwrap_or("");
        for svg in text.split("<svg").skip(1) {
            let svg = &svg[..svg.find("</svg>").unwrap_or(svg.len())];
            assert!(!svg.contains("<a "), "{path}: a link inside an SVG");
        }
    }
    // No raster images and no scripts anywhere.
    for (path, bytes) in &site.files {
        if let Ok(text) = std::str::from_utf8(bytes) {
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
                category: "power", size: (1, 1), build_points: 1, mass: 4, hp: 1, power: (output: 1),
                salvage: (family: "tricky_family", data: 2, materials: {"programmable_matter": 1}))"#,
        ),
        (
            "ships/s.ron",
            r#"(id: "tricky_ship", name: "Ship <&>", summary: "A <ship>.", faction: "ship_mind", size: (2, 1), parts: [
                (part: "logic_core", x: 0, y: 0), (part: "tricky_part", x: 1, y: 0),
            ])"#,
        ),
        ("wiki/page/tricky_page.md", "# Title <script>alert(1)</script>\n\nText & <b>x</b>, see [[ship:tricky_ship]].\n"),
        ("wiki/part/tricky_part.md", "Prose with <div> & [[stat:mass]].\n"),
        (
            "families.ron",
            r#"[(id: "tricky_family", name: "Family <f>", summary: "Fed by <odd> & [[part:tricky_part]].", color: "power")]"#,
        ),
        (
            "research/r.ron",
            r#"[(id: "tricky_node", name: "Node <n> & \"q\"", summary: "Costs <much>.", family: "tricky_family",
                 kind: Variant, cost: (data: {"tricky_family": 1.5, "logic": 2}, materials: {"programmable_matter": 3}),
                 requires: ["tamed_core"], unlocks: [Part("automaton_cannon"), MindCapacity(2)])]"#,
        ),
        (
            "starts.ron",
            r#"[(id: "tricky_start", name: "Start <s>", summary: "Begin & <go>.", ship: "tricky_ship", mind_capacity: 1,
                 voyages: ["first_hop", "second_hop"])]"#,
        ),
        ("wiki/research/tricky_node.md", "Research prose <marquee> & [[family:tricky_family]].\n"),
        (
            "encounters/e.ron",
            r#"[(id: "tricky_encounter", name: "Encounter <e> & \"x\"", summary: "Waits <here>.",
                 lore: "Old <wreck> & [[ship:tricky_ship]].", ships: [(ship: "automaton_raider", count: (1, 2))],
                 wrecks: [(ship: "tricky_ship", count: (0, 1), damage: 0.25)], wake_radius: 150)]"#,
        ),
        (
            "voyages/v.ron",
            r#"[(id: "first_hop", name: "Hop <1>", summary: "First & <hop>.", band: "rim", map: (0.1, 0.2), radius: 500,
                 spacing: (300, 600), minutes: (5, 5), cleared: "Cleared <done> & on to [[voyage:second_hop|the <next>]].",
                 sites: [(encounter: "tricky_encounter", goal: true), (encounter: "raider_pack", count: 2)]),
                (id: "second_hop", name: "Hop <2>", summary: "Second <hop>.", band: "outer", map: (0.3, 0.4),
                 opens_after: ["first_hop"], radius: 500, spacing: (300, 600), minutes: (5, 9),
                 sites: [(encounter: "tricky_encounter", goal: true)])]"#,
        ),
        ("wiki/encounter/tricky_encounter.md", "Encounter prose <blink> & [[voyage:first_hop]].\n"),
    ])
    .unwrap();
    Content::load(&[base, m]).unwrap()
}

#[test]
fn html_is_escaped() {
    let site = gen_files(&tricky_content());
    assert_eq!(site.warnings, Vec::<String>::new());
    assert_links_resolve(&site);
    // Where text is shown as HTML: page bodies (the front matter is data, which the site escapes), the sprite, and the
    // HTML fields of the data files.
    let mut shown: Vec<(String, String)> = Vec::new();
    for (path, bytes) in site.files.iter().filter(|(p, _)| !p.starts_with("fonts/")) {
        let text = std::str::from_utf8(bytes).unwrap();
        if path.ends_with(".md") {
            shown.push((path.clone(), text.split_once("\n---\n\n").map_or(text, |(_, b)| b).to_string()));
        } else if path.ends_with(".json") {
            fn html<'a>(v: &'a Value, key: &str, out: &mut Vec<&'a str>) {
                match v {
                    Value::String(s) if ["summary", "svg", "keys", "html", "value"].contains(&key) => out.push(s),
                    Value::Array(a) => a.iter().for_each(|x| html(x, key, out)),
                    Value::Object(o) => o.iter().for_each(|(k, x)| html(x, if key == "values" { "value" } else { k }, out)),
                    _ => {}
                }
            }
            let v: Value = serde_json::from_str(text).unwrap();
            let mut out = Vec::new();
            html(&v, "", &mut out);
            shown.extend(out.into_iter().map(|s| (path.clone(), s.to_string())));
        } else {
            shown.push((path.clone(), text.to_string()));
        }
    }
    for (path, text) in &shown {
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
            "<f>",
            "<odd>",
            "<n>",
            "<much>",
            "<s>",
            "<go>",
            "<marquee>",
            "<e>",
            "<here>",
            "<wreck>",
            "<1>",
            "<2>",
            "<hop>",
            "<done>",
            "<next>",
            "<blink>",
        ] {
            assert!(!text.contains(raw), "{path} contains raw `{raw}`");
        }
    }
    let part = page(&site, "parts/tricky_part.html");
    assert!(part.contains("<h1>&lt;Tricky &amp; &quot;Part&quot;&gt;</h1>"));
    assert!(std::str::from_utf8(&site.files["parts/tricky_part.md"])
        .unwrap()
        .starts_with("---\ntitle: \"<Tricky & \\\"Part\\\">\"\n"));
    assert!(part.contains("Uses &lt;b&gt;bold&lt;/b&gt; &amp; <a href=\"../parts/radiator.html\">&lt;i&gt;rad&lt;/i&gt;</a>."));
    assert!(part.contains("<code>&lt;code&gt;</code> and <strong>&lt;strong&gt;</strong>"));
    assert!(part.contains("&#39;quoted&#39; &lt;lore&gt;"));
    assert!(part.contains(
        "Prose with &lt;div&gt; &amp; <a href=\"../stats/mass.html\">Heft &lt;t&gt;</a>."
    ));
    // Stat labels and units come from stats.ron, so a mod's relabelled stat shows everywhere.
    assert!(part.contains(">Heft &lt;t&gt;</a></th><td>4 kg&lt;</td>"));
    assert!(
        page(&site, "parts/fission_reactor.html")
            .contains(">Heft &lt;t&gt;</a></th><td>4 kg&lt;</td>")
    );
    assert!(
        page(&site, "pages/tricky_page.html")
            .contains("<h1>Title &lt;script&gt;alert(1)&lt;/script&gt;</h1>")
    );
    assert!(page(&site, "index.html").contains("Tricky &lt;mod&gt; <code>tricky</code></td><td class=\"num\">1 &amp; 2</td>"));
    assert!(page(&site, "ships.html").contains("aria-label=\"Ship &lt;&amp;&gt;\""));
    // The career kinds of a mod: escaped, linked, and the mod's family in a base family's page (the node costs
    // logic data too).
    let node = page(&site, "research/tricky_node.html");
    assert!(node.contains("<h1>Node &lt;n&gt; &amp; &quot;q&quot;</h1>"));
    assert!(node.contains(
        "Research prose &lt;marquee&gt; &amp; <a href=\"../families/tricky_family.html\">Family &lt;f&gt;</a>."
    ));
    assert!(node.contains("<td class=\"name\">Mind capacity +2</td>"));
    assert!(page(&site, "families/logic.html").contains("<h2>Also pays for</h2>"));
    assert!(
        page(&site, "families/tricky_family.html").contains(
            "href=\"../parts/tricky_part.html\">&lt;Tricky &amp; &quot;Part&quot;&gt;</a>"
        )
    );
    let start = page(&site, "starts/tricky_start.html");
    assert!(start.contains("<h1>Start &lt;s&gt;</h1>"));
    assert!(start.contains("href=\"../voyages/first_hop.html\">Hop &lt;1&gt;</a>"));
    assert!(start.contains("href=\"../voyages/second_hop.html\">Hop &lt;2&gt;</a>"));
    assert!(
        page(&site, "starts.html").contains("<span class=\"sub\">Begin &amp; &lt;go&gt;.</span>")
    );
    // A mod's voyages and encounters: names, summaries, the cleared line with a labelled link, lore, prose.
    let name = "Encounter &lt;e&gt; &amp; &quot;x&quot;";
    let hop = page(&site, "voyages/first_hop.html");
    assert!(hop.contains("<h1>Hop &lt;1&gt;</h1>"));
    assert!(hop.contains(
        "Cleared &lt;done&gt; &amp; on to <a href=\"../voyages/second_hop.html\">the &lt;next&gt;</a>."
    ));
    assert!(hop.contains(&format!(
        "href=\"../encounters/tricky_encounter.html\">{name}</a> <span class=\"badge goal\""
    )));
    assert!(page(&site, "voyages/second_hop.html").contains(
        "<a href=\"../voyages/first_hop.html\">Hop &lt;1&gt;</a><br><span class=\"note\">rim band</span></td><td>First &amp; &lt;hop&gt;.</td>"
    ));
    let enc = page(&site, "encounters/tricky_encounter.html");
    assert!(enc.contains(&format!("<h1>{name}</h1>")));
    assert!(enc.contains(
        "Old &lt;wreck&gt; &amp; <a href=\"../ships/tricky_ship.html\">Ship &lt;&amp;&gt;</a>."
    ));
    assert!(enc.contains(
        "Encounter prose &lt;blink&gt; &amp; <a href=\"../voyages/first_hop.html\">Hop &lt;1&gt;</a>."
    ));
    assert!(enc.contains("<td>25% of parts gone</td><td class=\"num\">×0–1</td>"));
    assert!(enc.contains("<dt>Wake radius</dt><dd>150 m</dd>"));
    assert!(enc.contains("href=\"../voyages/second_hop.html\""));
    assert!(page(&site, "ships/tricky_ship.html").contains(&format!(
        "<a href=\"../encounters/tricky_encounter.html\">{name}</a>: ×0–1 wrecked, 25% of parts gone"
    )));
    assert!(page(&site, "encounters.html").contains("<td>Waits &lt;here&gt;.</td>"));
    assert!(page(&site, "voyages.html").contains("<td class=\"num\">5 min</td>"));
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
        errors: Vec::new(),
    };
    write(&site(&["index.html", "parts/a.html", "parts/b.html"]), &dir).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("parts/b.html")).unwrap(),
        "parts/b.html"
    );
    // A second run removes what it no longer makes (and the emptied directory), and nothing else.
    std::fs::write(dir.join("notes.txt"), "mine").unwrap();
    write(&site(&["index.html", "ships/c.html"]), &dir).unwrap();
    assert!(!dir.join("part").exists());
    assert!(dir.join("ships/c.html").exists() && dir.join("notes.txt").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join(MANIFEST)).unwrap(),
        "index.html\nships/c.html\n"
    );
    // A directory with other files and no manifest is left alone.
    std::fs::remove_file(dir.join(MANIFEST)).unwrap();
    let err = write(&site(&["index.html"]), &dir).unwrap_err();
    assert!(err.contains("not empty"), "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}
