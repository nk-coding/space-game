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
    want.extend(c.families.iter().map(|e| item_path(Kind::Family, &e.id)));
    want.extend(c.research.iter().map(|e| item_path(Kind::Research, &e.id)));
    want.extend(c.starts.iter().map(|e| item_path(Kind::Start, &e.id)));
    want.extend(c.voyages.iter().map(|e| item_path(Kind::Voyage, &e.id)));
    want.extend(
        c.encounters
            .iter()
            .map(|e| item_path(Kind::Encounter, &e.id)),
    );
    // Every kind with entries has a list page and an index count (pages come from `wiki/page/`).
    for kind in Kind::ALL {
        assert!(
            KINDS.iter().any(|(k, _)| *k == kind),
            "kind {kind} has no list page"
        );
        assert!(
            page(&site, "index.html").contains(&format!("href=\"{}\"", list_path(kind))),
            "index lacks {kind}"
        );
    }
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
    for path in [
        "family/logic.html",
        "research/mind_expansion_1.html",
        "start/rim_awakening.html",
        "voyage/yard_approach.html",
        "encounter/the_warden.html",
    ] {
        assert!(a.files.contains_key(path), "no {path}");
    }
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
fn career_pages_link_both_ways() {
    let site = generate(&base());
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
            "family/logic.html",
            &format!("href=\"../research/{node}.html\""),
        );
        has(
            &format!("research/{node}.html"),
            "href=\"../family/logic.html\"",
        );
    }
    has("family/logic.html", "href=\"../part/logic_core.html\"");
    has(
        "family/logic.html",
        "href=\"../part/automaton_core.html\">Automaton core</a> <span class=\"badge locked\"",
    );
    has("family/hull.html", "href=\"../material/steel.html\"");
    has(
        "family/hull.html",
        "href=\"../research/cruiser_drydock.html\"",
    );
    has(
        "family/shields.html",
        "<h2>Research</h2>\n<p class=\"none\">",
    );
    // A node: family, kind, cost in data and materials, what it needs, what needs it, what it unlocks.
    let node = "research/mind_expansion_1.html";
    has(node, "<dt>Kind</dt><dd><span title=");
    has(node, ">Refinement</span>");
    has(
        node,
        "10 <a href=\"../family/logic.html\">Logic and sensors</a> data<br>2 <a href=\"../material/programmable_matter.html\">",
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
        "href=\"../part/automaton_core.html\">Automaton core</a> <span class=\"badge locked\"",
    );
    has(
        "research/composite_plating.html",
        "href=\"../material/composite.html\"",
    );
    has(
        "research/cruiser_drydock.html",
        "href=\"../frame/cruiser.html\">Cruiser</a> <span class=\"badge locked\"",
    );
    // A node's texts link to entries, and those list it as mentioning them.
    has(
        "part/automaton_core.html",
        "href=\"../research/tamed_core.html\"",
    );
    // A start: its ship drawn like a ship page and linked, the mind capacity with the research that adds to
    // it, the open voyages (linked), and what a new game has.
    let start = "start/rim_awakening.html";
    has(start, "<figure class=\"hero\"");
    has(start, "<svg class=\"ship\"");
    has(start, "aria-label=\"Rim skiff\"");
    has(
        start,
        "<dt>Ship</dt><dd><a href=\"../ship/rim_skiff.html\">",
    );
    has(start, "<dt>Mind capacity</dt><dd>2 logic cores</dd>");
    has(start, "href=\"../research/mind_expansion_2.html\"");
    has(start, "(<strong>4</strong> in all)");
    has(
        start,
        "<h2>Open voyages</h2>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../voyage/yard_approach.html\">Shipyard approach</a>",
    );
    lacks(start, "halvex_drift.html");
    has(start, "href=\"../part/pulse_laser.html\"");
    lacks(start, "href=\"../part/beam_laser.html\"");
    // List pages: research grouped by family, families with counts, starts as cards.
    has("research.html", "href=\"family/logic.html\"");
    has("research.html", "href=\"research/cruiser_drydock.html\"");
    lacks("research.html", "href=\"family/shields.html\"");
    has("families.html", "href=\"family/utility.html\"");
    has("families.html", "3 research nodes");
    has(
        "starts.html",
        "<a class=\"card\" href=\"start/rim_awakening.html\">",
    );
}

#[test]
fn voyage_pages_link_both_ways() {
    let c = base();
    let site = generate(&c);
    let has = |path: &str, needle: &str| {
        assert!(page(&site, path).contains(needle), "{path} lacks {needle}")
    };
    let lacks = |path: &str, needle: &str| {
        assert!(!page(&site, path).contains(needle), "{path} has {needle}")
    };
    let goal = "<span class=\"badge goal\"";
    // A voyage: its numbers with units, its sites (encounters with what they hold and how many, the goal
    // marked), the start that opens it, the cleared line with its links, and the voyages it opens.
    let first = "voyage/yard_approach.html";
    has(first, "<dt>Band</dt><dd>rim</dd>");
    has(first, ">0.28, 0.66</span></dd>");
    has(first, "<dt>Radius</dt><dd>600 m</dd>");
    has(first, "<dt>Site spacing</dt><dd>400–700 m</dd>");
    has(first, "<dt>Expected length</dt><dd>6–8 min</dd>");
    has(first, "<dt>Sites</dt><dd>4</dd>");
    has(
        first,
        "<dt>Goal</dt><dd><a href=\"../encounter/raider_pack.html\">Raider pack</a></dd>",
    );
    has(
        first,
        "<dt>Open at the start of</dt><dd><a href=\"../start/rim_awakening.html\">",
    );
    has(
        first,
        "<tr><td class=\"name\"><a href=\"../encounter/raider_pair.html\">Raider pair</a></td><td><a href=\"../ship/automaton_raider.html\">Automaton raider</a>&nbsp;×2</td><td>—</td><td class=\"num\">×2</td></tr>",
    );
    has(
        first,
        "<a href=\"../ship/sample_early.html\">Sample early</a>&nbsp;×1, 50% of parts gone",
    );
    has(
        first,
        &format!("href=\"../encounter/raider_pack.html\">Raider pack</a> {goal}"),
    );
    lacks(
        first,
        &format!("href=\"../encounter/raider_pair.html\">Raider pair</a> {goal}"),
    );
    has(
        first,
        "<blockquote><p>The lanes around the shipyard are quiet again. Further out, the <a href=\"../voyage/halvex_drift.html\">Halvex Drift</a> is waiting.</p></blockquote>",
    );
    has(
        first,
        "<h2>Opens after</h2>\n<p class=\"none\">None: it is open at the start.</p>",
    );
    has(
        first,
        "<h2>Opens</h2>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../voyage/halvex_drift.html\">",
    );
    // The next voyage names it back (and the cleared line counts as a mention); the last opens nothing.
    has(
        "voyage/halvex_drift.html",
        "<h2>Opens after</h2>\n<p class=\"note\">It opens once the goal of each of these has been cleared.</p>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../voyage/yard_approach.html\">",
    );
    has(
        "voyage/halvex_drift.html",
        "<h2>Mentioned on</h2>\n<p><a href=\"../voyage/yard_approach.html\">",
    );
    lacks("voyage/halvex_drift.html", "Open at the start of");
    has(
        "voyage/wardens_gate.html",
        "<h2>Opens</h2>\n<p class=\"none\">None.</p>",
    );
    // An encounter: wake radius, its ships and wrecks drawn and linked with their counts, the wrecks' damage,
    // and the voyages that place it (how many sites, the goal marked).
    let warden = "encounter/the_warden.html";
    has(warden, "<dt>Wake radius</dt><dd>300 m</dd>");
    has(warden, "<dt>Ships</dt><dd>3–5</dd>");
    lacks(warden, "<dt>Wrecks</dt>");
    has(warden, "aria-label=\"Automaton warden\"");
    has(
        warden,
        "href=\"../ship/automaton_raider.html\">Automaton raider</a></td><td>",
    );
    has(warden, "<td class=\"num\">×2–4</td>");
    has(warden, "within 300 m of the site");
    has(warden, "<h2>Wrecks</h2>\n<p class=\"none\">None.</p>");
    has(
        warden,
        &format!(
            "<tr><td class=\"name\"><a href=\"../voyage/wardens_gate.html\">The Warden&#39;s gate</a> {goal}"
        ),
    );
    has(warden, "<td class=\"num\">1 site</td>");
    has(warden, "the shielded heart holds");
    let graveyard = "encounter/core_graveyard.html";
    has(graveyard, "<dt>Wrecks</dt><dd>4–6</dd>");
    has(
        graveyard,
        "<td>30% of parts gone</td><td class=\"num\">×4–6</td>",
    );
    has(
        "encounter/raider_pair.html",
        "<tr><td class=\"name\"><a href=\"../voyage/yard_approach.html\">Shipyard approach</a></td><td>",
    );
    has(
        "encounter/raider_pair.html",
        "<td class=\"num\">2 sites</td>",
    );
    for v in &c.voyages {
        has(
            "encounter/raider_pack.html",
            &format!("href=\"../voyage/{}.html\"", v.id),
        );
    }
    // A ship names the encounters it waits in, fighting or as a wreck; the encounters name it back.
    has(
        "ship/automaton_warden.html",
        "<h2>In encounters</h2>\n<ul>\n<li><a href=\"../encounter/the_warden.html\">The Warden</a>: ×1 fighting</li>",
    );
    has(
        "ship/automaton_raider.html",
        "<a href=\"../encounter/derelict_field.html\">Derelict field</a>: ×0–1 fighting; ×2–3 wrecked, 50% of parts gone",
    );
    has(
        "ship/sample_mid.html",
        "<a href=\"../encounter/precursor_wrecks.html\">Precursor wrecks</a>: ×1–2 wrecked, 40% of parts gone",
    );
    has(
        "encounter/precursor_wrecks.html",
        "href=\"../ship/sample_mid.html\"",
    );
    lacks("ship/rim_skiff.html", "In encounters");
    // List pages: voyages in load order with band, sites and length; encounters as a table.
    let list = page(&site, "voyages.html");
    let at: Vec<usize> = c
        .voyages
        .iter()
        .map(|v| {
            list.find(&format!("href=\"voyage/{}.html\"", v.id))
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
        "<a href=\"ship/automaton_hunter.html\">Automaton hunter</a>&nbsp;×2–3<br><a href=\"ship/automaton_raider.html\">Automaton raider</a>&nbsp;×1–2</td><td>—</td><td class=\"num\">300 m</td>",
    );
    has(
        "encounters.html",
        "<a href=\"ship/automaton_raider.html\">Automaton raider</a>&nbsp;×4–6, 30% of parts gone",
    );
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
    // Frames too (GDR-0063): on their page and in the list.
    assert!(page(&site, "frame/cruiser.html").contains("<h1>Cruiser <span class=\"badge locked\""));
    assert!(!page(&site, "frame/frigate.html").contains("class=\"badge locked\""));
    assert!(page(&site, "frames.html").contains("Capital</a> <span class=\"badge locked\""));
}

#[test]
fn parts_materials_and_frames_show_salvage_and_unlocks() {
    let site = generate(&base());
    let has = |path: &str, needle: &str| {
        assert!(page(&site, path).contains(needle), "{path} lacks {needle}")
    };
    let lacks = |path: &str, needle: &str| {
        assert!(!page(&site, path).contains(needle), "{path} has {needle}")
    };
    let pm = "<a href=\"../material/programmable_matter.html\">Programmable matter</a>";
    // A part's salvage block: data for its family and materials when intact, the fragment when destroyed.
    let core = "part/automaton_core.html";
    has(
        core,
        &format!(
            "<tr><th>Intact part</th><td>4 <a href=\"../family/logic.html\">Logic and sensors</a> data, 1 {pm}</td></tr>"
        ),
    );
    has(
        core,
        &format!("<tr><th>Destroyed part</th><td>leaves a fragment of 1 {pm}</td></tr>"),
    );
    has(
        "part/pulse_laser.html",
        "<td>3 <a href=\"../family/energy.html\">Energy weapons</a> data</td></tr>\n<tr><th>Destroyed part</th><td>leaves nothing</td>",
    );
    // Hull parts give data by their material, per cell of hull area (a 1×3 slope is 1.5 cells).
    let slope = "part/slope_1x3.html";
    has(slope, "this part is 1.5 cells.");
    has(
        slope,
        "<tr><th><a href=\"../material/composite.html\">Composite</a> <span class=\"badge locked\"",
    );
    has(
        slope,
        "<td>2.25 <a href=\"../family/hull.html\">Hull and armour</a> data</td>",
    );
    // "Unlocked by": the nodes that unlock a locked part, material or frame, or a note that none does yet;
    // entries a new game has show none.
    has(
        core,
        "<h2>Unlocked by</h2>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../research/tamed_core.html\">",
    );
    has(
        "part/automaton_cannon.html",
        "href=\"../research/automaton_cannon.html\"",
    );
    has(
        "part/beam_laser.html",
        "href=\"../research/beam_laser.html\"",
    );
    lacks("part/pulse_laser.html", "<h2>Unlocked by</h2>");
    has(
        "material/composite.html",
        "<h2>Unlocked by</h2>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../research/composite_plating.html\">",
    );
    has(
        "material/reactive.html",
        "<h2>Unlocked by</h2>\n<p class=\"none\">No research unlocks it yet.</p>",
    );
    lacks("material/steel.html", "<h2>Unlocked by</h2>");
    has(
        "frame/cruiser.html",
        "<h2>Unlocked by</h2>\n<table class=\"list\">\n<tr><td class=\"name\"><a href=\"../research/cruiser_drydock.html\">",
    );
    has("frame/capital.html", "No research unlocks it yet.");
    lacks("frame/frigate.html", "<h2>Unlocked by</h2>");
    // An armour material's salvage per cell; programmable matter: the parts that give it and the research
    // that costs it.
    has(
        "material/composite.html",
        "<tr><th>Per cell of hull area</th><td>1.5 <a href=\"../family/hull.html\">Hull and armour</a> data</td></tr>",
    );
    let matter = "material/programmable_matter.html";
    lacks(matter, "<h2>Salvage</h2>");
    has(matter, "<h2>From salvage</h2>");
    has(
        matter,
        "href=\"../part/logic_core.html\">Logic core</a></td><td>1 per intact part, 1 per fragment of a destroyed one</td>",
    );
    has(matter, "href=\"../part/automaton_core.html\"");
    has(matter, "<h2>Research that costs it</h2>");
    for node in ["tamed_core", "mind_expansion_1", "mind_expansion_2"] {
        has(matter, &format!("href=\"../research/{node}.html\""));
    }
    lacks(matter, "href=\"../research/beam_laser.html\"");
    // The start's ship names the start.
    has(
        "ship/rim_skiff.html",
        "<dt>Start ship of</dt><dd><a href=\"../start/rim_awakening.html\">",
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
    // The career kinds of a mod: escaped, linked, and the mod's family in a base family's page (the node costs
    // logic data too).
    let node = page(&site, "research/tricky_node.html");
    assert!(node.contains("<h1>Node &lt;n&gt; &amp; &quot;q&quot;</h1>"));
    assert!(node.contains(
        "Research prose &lt;marquee&gt; &amp; <a href=\"../family/tricky_family.html\">Family &lt;f&gt;</a>."
    ));
    assert!(node.contains("<td class=\"name\">Mind capacity +2</td>"));
    assert!(page(&site, "family/logic.html").contains("<h2>Also pays for</h2>"));
    assert!(
        page(&site, "family/tricky_family.html").contains(
            "href=\"../part/tricky_part.html\">&lt;Tricky &amp; &quot;Part&quot;&gt;</a>"
        )
    );
    let start = page(&site, "start/tricky_start.html");
    assert!(start.contains("<h1>Start &lt;s&gt;</h1>"));
    assert!(start.contains("href=\"../voyage/first_hop.html\">Hop &lt;1&gt;</a>"));
    assert!(start.contains("href=\"../voyage/second_hop.html\">Hop &lt;2&gt;</a>"));
    assert!(
        page(&site, "starts.html").contains("<span class=\"sub\">Begin &amp; &lt;go&gt;.</span>")
    );
    // A mod's voyages and encounters: names, summaries, the cleared line with a labelled link, lore, prose.
    let name = "Encounter &lt;e&gt; &amp; &quot;x&quot;";
    let hop = page(&site, "voyage/first_hop.html");
    assert!(hop.contains("<h1>Hop &lt;1&gt;</h1>"));
    assert!(hop.contains(
        "Cleared &lt;done&gt; &amp; on to <a href=\"../voyage/second_hop.html\">the &lt;next&gt;</a>."
    ));
    assert!(hop.contains(&format!(
        "href=\"../encounter/tricky_encounter.html\">{name}</a> <span class=\"badge goal\""
    )));
    assert!(page(&site, "voyage/second_hop.html").contains(
        "<a href=\"../voyage/first_hop.html\">Hop &lt;1&gt;</a><br><span class=\"note\">rim band</span></td><td>First &amp; &lt;hop&gt;.</td>"
    ));
    let enc = page(&site, "encounter/tricky_encounter.html");
    assert!(enc.contains(&format!("<h1>{name}</h1>")));
    assert!(enc.contains(
        "Old &lt;wreck&gt; &amp; <a href=\"../ship/tricky_ship.html\">Ship &lt;&amp;&gt;</a>."
    ));
    assert!(enc.contains(
        "Encounter prose &lt;blink&gt; &amp; <a href=\"../voyage/first_hop.html\">Hop &lt;1&gt;</a>."
    ));
    assert!(enc.contains("<td>25% of parts gone</td><td class=\"num\">×0–1</td>"));
    assert!(enc.contains("<dt>Wake radius</dt><dd>150 m</dd>"));
    assert!(enc.contains("href=\"../voyage/second_hop.html\""));
    assert!(page(&site, "ship/tricky_ship.html").contains(&format!(
        "<a href=\"../encounter/tricky_encounter.html\">{name}</a>: ×0–1 wrecked, 25% of parts gone"
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
