//! What the wiki's hand-written pages may reference, so they never type a name or a number the data holds
//! (`docs/wiki.md` "Writing pages"):
//!
//! * `[[kind:id]]` (and `[[kind:id|label]]`): a link to an entry's page, labelled with its name;
//! * `[[kind:id.field]]`: a value of the entry, formatted as the game shows it: a part's, material's or ship's stat
//!   (`[[part:thruster.thrust]]`, `[[ship:rim_skiff.top_speed]]`; a hull part in a material:
//!   `[[part:block_1x1@steel.hp]]`), a control's default keys (`.keys`), a node's signature and parameter defaults
//!   (`.signature`, `.default.radius`), and a few more (see [`Refs::new`]);
//! * `[[sim:name]]`: a constant of the simulation, from the sim's own constants and default config ([`sim_values`]);
//! * `[[lab:list.id]]`: an entry of `lab.ron` (a goal, trial, gene, pilot, size, ...), a link to its row on the lab's
//!   page with its name, and `[[lab:list.id.field]]` one of its values (`[[lab:size.standard.population]]`);
//!   `[[lab:spread.design]]`, `[[lab:budget.layer_open]]` and `[[lab:palette.<faction>]]` are values too.
//!
//! `refs.json` carries all of them for the site's Markdown plugin; [`super::hand`] checks the pages against them.

use super::*;
use pages::{default_inputs, minutes};

/// The references and values a hand-written page may use.
pub struct Refs {
    /// `kind:id` → (name, page path).
    pub entries: BTreeMap<String, (String, String)>,
    /// `kind:id.field` (or `kind:id@material.field`) → HTML (links through [`ROOT`]).
    pub values: BTreeMap<String, String>,
    /// `sim:name` → text.
    pub sim: BTreeMap<String, String>,
}

impl Refs {
    pub(super) fn new(w: &Wiki) -> Refs {
        let c = w.c;
        let mut entries = BTreeMap::new();
        let mut add = |kind: Kind, id: &str, name: &str| {
            entries.insert(
                format!("{}:{id}", kind.key()),
                (name.to_string(), item_path(kind, id)),
            );
        };
        for e in &c.stats {
            add(Kind::Stat, &e.id, &e.name);
        }
        for e in &c.categories {
            add(Kind::Category, &e.id, &e.name);
        }
        for e in &c.factions {
            add(Kind::Faction, &e.id, &e.name);
        }
        for e in &c.materials {
            add(Kind::Material, &e.id, &e.name);
        }
        for e in &c.parts {
            add(Kind::Part, &e.id, &e.name);
        }
        for e in &c.frames {
            add(Kind::Frame, &e.id, &e.name);
        }
        for e in &c.ships {
            add(Kind::Ship, &e.id, &e.name);
        }
        for e in &c.behavior_nodes {
            add(Kind::BehaviorNode, &e.id, &e.name);
        }
        for e in &c.behaviors {
            add(Kind::Behavior, &e.id, &e.name);
        }
        for e in &c.families {
            add(Kind::Family, &e.id, &e.name);
        }
        for e in &c.research {
            add(Kind::Research, &e.id, &e.name);
        }
        for e in &c.facilities {
            add(Kind::Facility, &e.id, &e.name);
        }
        for e in &c.starts {
            add(Kind::Start, &e.id, &e.name);
        }
        for e in &c.voyages {
            add(Kind::Voyage, &e.id, &e.name);
        }
        for e in &c.encounters {
            add(Kind::Encounter, &e.id, &e.name);
        }
        for e in &c.controls {
            add(Kind::Control, &e.id, &e.name);
        }
        for e in &c.galaxies {
            add(Kind::Galaxy, &e.id, &e.name);
        }
        for e in &c.bands {
            add(Kind::Band, &e.id, &e.name);
        }
        for e in &c.cluster_types {
            add(Kind::ClusterType, &e.id, &e.name);
        }
        for e in &c.trails {
            add(Kind::Trail, &e.id, &e.name);
        }
        for e in &c.signal_sources {
            add(Kind::SignalSource, &e.id, &e.name);
        }
        for e in &c.lore {
            add(Kind::Lore, &e.id, &e.name);
        }
        for p in c.wiki.values().filter(|p| p.kind == Kind::Page) {
            add(Kind::Page, &p.id, c.name_of(Kind::Page, &p.id).unwrap_or(&p.id));
        }

        let mut values = BTreeMap::new();
        let mut put = |key: String, html: String| {
            values.insert(key, html);
        };
        for p in &c.parts {
            if p.hull {
                for m in c.materials.iter().filter(|m| m.armor.is_some()) {
                    for (stat, text) in Wiki::plain_lines(&c.part_stats(p, Some(m))) {
                        put(format!("part:{}@{}.{stat}", p.id, m.id), esc(&text));
                    }
                }
            } else {
                for (stat, text) in Wiki::plain_lines(&c.part_stats(p, None)) {
                    put(format!("part:{}.{stat}", p.id), esc(&text));
                }
            }
        }
        for m in &c.materials {
            for (stat, text) in Wiki::plain_lines(&c.material_stats(m)) {
                put(format!("material:{}.{stat}", m.id), esc(&text));
            }
        }
        for (s, r) in c.ships.iter().zip(&w.reports) {
            if let Some(r) = r {
                for (stat, text) in w.ship_stats(r) {
                    put(format!("ship:{}.{stat}", s.id), esc(&text));
                }
            }
        }
        for e in &c.controls {
            put(format!("control:{}.keys", e.id), default_inputs(e));
        }
        for n in &c.behavior_nodes {
            put(
                format!("behavior_node:{}.signature", n.id),
                format!("<code>{}</code>", esc(&pages::signature(n))),
            );
            for p in &n.params {
                if let Some(d) = &p.default {
                    put(
                        format!("behavior_node:{}.default.{}", n.id, p.id),
                        format!("<code>{}</code>", esc(d)),
                    );
                }
            }
        }
        for b in &c.behaviors {
            put(format!("behavior:{}.rules", b.id), b.rules.len().to_string());
        }
        for f in &c.frames {
            put(
                format!("frame:{}.grid", f.id),
                format!("{} × {} cells", f.size.0, f.size.1),
            );
        }
        for n in &c.research {
            let cost = w.cost_items(ROOT, &n.cost);
            put(
                format!("research:{}.cost", n.id),
                if cost.is_empty() {
                    "free".into()
                } else {
                    cost.join(", ")
                },
            );
        }
        for n in &c.research {
            let requires: Vec<String> = n.requires.iter().map(|r| w.a(ROOT, Kind::Research, r)).collect();
            put(
                format!("research:{}.requires", n.id),
                if requires.is_empty() { "—".into() } else { requires.join(", ") },
            );
            let unlocks: Vec<String> = n.unlocks.iter().map(|u| w.unlock_short(ROOT, u)).collect();
            put(format!("research:{}.unlocks", n.id), unlocks.join(", "));
        }
        for p in &c.parts {
            if let Some(pw) = p.power.as_ref().filter(|pw| pw.draw > 0.0) {
                put(format!("part:{}.power_tier", p.id), format!("{:?}", pw.tier));
            }
            if let Some(sh) = &p.shield {
                for (family, x) in &sh.absorb {
                    put(
                        format!("part:{}.absorb.{}", p.id, format!("{family:?}").to_lowercase()),
                        format!("{}%", format_number(x * 100.0, 0)),
                    );
                }
            }
            for l in c.part_stats(p, None).iter().filter(|l| l.bonus.is_some()) {
                put(format!("part:{}.bonus.{}", p.id, l.stat), esc(&l.text));
                if let Some(n) = l.bonus.as_ref().and_then(|b| b.max_stacks) {
                    put(format!("part:{}.bonus.{}.max_stacks", p.id, l.stat), n.to_string());
                }
            }
            if let Some(sv) = &p.salvage {
                put(format!("part:{}.salvage_data", p.id), w.data_amount(ROOT, &sv.family, sv.data));
                let items = w.material_items(ROOT, &sv.materials);
                put(
                    format!("part:{}.salvage_materials", p.id),
                    if items.is_empty() { "none".into() } else { items.join(", ") },
                );
                let fragment = w.material_items(ROOT, &sv.fragment);
                put(
                    format!("part:{}.fragment", p.id),
                    if fragment.is_empty() { "nothing".into() } else { fragment.join(", ") },
                );
            }
        }
        for m in &c.materials {
            if let Some(sv) = &m.salvage {
                put(format!("material:{}.salvage_data", m.id), w.data_amount(ROOT, &sv.family, sv.data));
            }
        }
        for p in c.parts.iter().filter(|p| !p.hull) {
            let html = match &p.salvage {
                Some(sv) => {
                    let mut items = vec![w.data_amount(ROOT, &sv.family, sv.data)];
                    items.extend(w.material_items(ROOT, &sv.materials));
                    items.join(", ")
                }
                None => "nothing".into(),
            };
            put(format!("part:{}.salvage", p.id), html);
        }
        for m in c.materials.iter().filter(|m| m.armor.is_some()) {
            let html = match &m.salvage {
                Some(sv) => format!("{} per cell of hull", w.data_amount(ROOT, &sv.family, sv.data)),
                None => "nothing".into(),
            };
            put(format!("material:{}.salvage", m.id), html);
        }
        for v in &c.voyages {
            let after: Vec<String> = v.opens_after.iter().map(|a| w.a(ROOT, Kind::Voyage, a)).collect();
            put(
                format!("voyage:{}.opens_after", v.id),
                if after.is_empty() { "—".into() } else { after.join(", ") },
            );
        }
        for s in &c.starts {
            put(
                format!("start:{}.mind_capacity", s.id),
                s.mind_capacity.to_string(),
            );
        }
        for v in &c.voyages {
            put(format!("voyage:{}.sites", v.id), v.site_count().to_string());
            put(format!("voyage:{}.length", v.id), esc(&minutes(v.minutes)));
            put(format!("voyage:{}.band", v.id), esc(&v.band));
        }
        for e in &c.encounters {
            put(
                format!("encounter:{}.wake_radius", e.id),
                esc(&pages::metres(e.wake_radius)),
            );
        }
        for g in &c.galaxies {
            put(format!("galaxy:{}.spacing", g.id), esc(&w.value("galaxy_distance", g.spacing)));
            put(format!("galaxy:{}.jump_range", g.id), esc(&w.value("jump_range", g.jump_range)));
            put(format!("galaxy:{}.home_neighbours", g.id), g.home_neighbours.to_string());
            put(format!("galaxy:{}.warp", g.id), format!("{} s + {} s per S", format_number(g.warp.0, 1), format_number(g.warp.1, 1)));
            put(format!("galaxy:{}.signal_area", g.id), esc(&w.s_range(g.signal_area.across)));
            put(format!("galaxy:{}.signal_narrow", g.id), format!("{}%", format_number(100.0 * g.signal_area.narrow, 0)));
            put(format!("galaxy:{}.trail_step", g.id), esc(&w.s_range(g.trail_step)));
            put(format!("galaxy:{}.bands", g.id), g.bands.len().to_string());
        }
        for b in &c.bands {
            put(format!("band:{}.edge", b.id), esc(&w.value("galaxy_distance", b.edge)));
            put(format!("band:{}.gates", b.id), b.gates.as_ref().map_or(0, |g| g.count).to_string());
        }
        for t in &c.cluster_types {
            put(format!("cluster_type:{}.radius", t.id), esc(&pages::cluster_radius(t)));
        }
        for t in &c.trails {
            let (lo, hi) = t.length;
            put(format!("trail:{}.length", t.id), if lo == hi { lo.to_string() } else { format!("{lo}–{hi}") });
            put(format!("trail:{}.hint", t.id), w.text(ROOT, &t.hint));
        }
        for x in &c.signal_sources {
            put(format!("signal_source:{}.line", x.id), w.text(ROOT, &x.line));
        }
        for st in &c.stats {
            put(
                format!("stat:{}.unit", st.id),
                if st.unit.is_empty() {
                    "none".into()
                } else {
                    esc(&st.unit)
                },
            );
        }
        for e in w.lab_entries(ROOT) {
            for (field, html) in &e.values {
                values.insert(format!("lab:{}.{field}", e.key), html.clone());
            }
            entries.insert(format!("lab:{}", e.key), (e.name.clone(), format!("{}.html#{}", pages::LAB_PAGE, e.anchor())));
        }
        for (key, html) in w.lab_values(ROOT) {
            values.insert(format!("lab:{key}"), html);
        }
        Refs {
            entries,
            values,
            sim: sim_values()
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        }
    }

    /// Does a reference (what stands between `[[` and `]]`) resolve? `Err` says why not.
    pub fn check(&self, r: &str) -> Result<(), String> {
        let (target, label) = match r.split_once('|') {
            Some((t, l)) => (t, Some(l)),
            None => (r, None),
        };
        if let Some(name) = target.strip_prefix("sim:") {
            if label.is_some() {
                return Err("a sim constant takes no label".into());
            }
            return if self.sim.contains_key(name) {
                Ok(())
            } else {
                Err(format!("unknown sim constant `{name}`"))
            };
        }
        if let Some(key) = target.strip_prefix("lab:") {
            return match (self.entries.contains_key(target), self.values.contains_key(target)) {
                (true, _) => Ok(()),
                (false, true) if label.is_some() => Err("a value takes no label".into()),
                (false, true) => Ok(()),
                (false, false) => Err(format!("no lab entry or value `{key}`")),
            };
        }
        let (kind, rest) = target
            .split_once(':')
            .ok_or_else(|| "write [[kind:id]], [[kind:id.field]], [[lab:list.id]] or [[sim:name]]".to_string())?;
        if Kind::from_key(kind).is_none() {
            return Err(format!("unknown kind `{kind}`"));
        }
        if rest.contains(['.', '@']) {
            if label.is_some() {
                return Err("a value takes no label".into());
            }
            return if self.values.contains_key(target) {
                Ok(())
            } else {
                Err(format!("no value `{target}`"))
            };
        }
        if self.entries.contains_key(target) {
            Ok(())
        } else {
            Err(format!("no {kind} `{rest}`"))
        }
    }

    pub(super) fn to_json(&self, examples: &BTreeMap<String, Value>) -> Value {
        let entries: BTreeMap<&str, Value> = self
            .entries
            .iter()
            .map(|(k, (name, url))| (k.as_str(), json!({ "name": name, "url": url })))
            .collect();
        json!({
            "root": ROOT,
            "entries": entries,
            "values": self.values,
            "sim": self.sim,
            "examples": examples,
        })
    }
}

/// The simulation's constants a player meets, formatted with their units: the sim's own `pub` constants and its
/// default config, so the wiki shows what the game runs.
pub fn sim_values() -> Vec<(&'static str, String)> {
    use sim::behavior::tuning as bt;
    use sim::ship::tuning as st;
    let cfg = sim::SimConfig::default();
    let pct = |x: f32| format!("{}%", format_number(x * 100.0, 0));
    let num = |x: f32, d: u8, unit: &str| with_unit(&format_number(x, d), unit);
    vec![
        ("tick_hz", format!("{} Hz", sim::TICK_HZ)),
        (
            "think_hz",
            format!("{} Hz", u64::from(sim::TICK_HZ) / bt::THINK_PERIOD),
        ),
        ("rated_knee", pct(st::RATED_KNEE)),
        ("fire_range", pct(bt::FIRE_RANGE)),
        ("fire_cone", num(bt::FIRE_CONE.to_degrees(), 1, "°")),
        ("default_reach", num(bt::DEFAULT_REACH, 0, "m")),
        ("target_sticky", pct(bt::STICKY)),
        ("assist_turn_dv", num(bt::ASSIST_TURN_DV, 1, "m/s")),
        ("station_slack", num(bt::STATION_SLACK, 1, "m")),
        ("avoid_ahead", num(bt::AVOID_AHEAD, 0, "s")),
        ("avoid_clearance", num(bt::AVOID_CLEARANCE, 1, "m")),
        ("overheat_resume", pct(st::OVERHEAT_RESUME)),
        ("reactor_idle_heat", pct(st::REACTOR_IDLE_HEAT)),
        ("ram_threshold", num(cfg.physics.ram_threshold, 0, "kN·s")),
        ("salvage_reach", num(cfg.salvage.reach, 1, "m")),
        ("salvage_max_speed", num(cfg.salvage.max_speed, 1, "m/s")),
        (
            "salvage_rate",
            format!(
                "{} parts/s",
                format_number(sim::TICK_HZ as f32 / cfg.salvage.interval.max(1) as f32, 1)
            ),
        ),
        (
            "key_turn_rate",
            num(cfg.flight.key_turn_rate.to_degrees(), 0, "°/s"),
        ),
        ("max_rules", data::behavior::MAX_RULES.to_string()),
        ("max_conditions", data::behavior::MAX_CONDITIONS.to_string()),
        ("max_values", data::behavior::MAX_ARGS.to_string()),
        ("max_line_chars", data::behavior::MAX_LINE_CHARS.to_string()),
        ("max_reach_percent", format!("{}%", format_number(data::behavior::MAX_REACH_PERCENT, 0))),
        ("script_max_ops", bt::SCRIPT_MAX_OPS.to_string()),
        ("undo_steps", data::design::edit::UNDO_STEPS.to_string()),
        ("max_history", data::career::MAX_HISTORY.to_string()),
        ("voyage_spawn_radius", num(cfg.voyage.spawn_radius, 0, "m")),
        ("recall_radius", num(cfg.voyage.recall_radius, 0, "m")),
        ("signal_reveal", num(cfg.voyage.signal_reveal, 0, "m")),
        ("edge_margin", num(cfg.voyage.edge_margin, 0, "m")),
        ("max_edge", num(sim::voyage::MAX_EDGE / 1000.0, 0, "km")),
        ("always_seen", num(sim::ship::sensor_tuning::ALWAYS_SEEN, 0, "m")),
        ("ram_damage", num(cfg.physics.ram_damage, 2, "HP per kN·s")),
        ("shield_restart", pct(sim::combat::weapons::tuning::SHIELD_RESTART)),
        ("heat_loss_open_side", format_number(st::HEAT_LOSS_OPEN_SIDE, 2)),
        ("settle_speed", num(cfg.salvage.settle_speed, 1, "m/s")),
        ("settle_decel", num(cfg.salvage.settle_decel, 1, "m/s²")),
        ("spill_speed", num(cfg.salvage.spill_speed, 1, "m/s")),
        (
            "ricochet_angle",
            num(sim::combat::weapons::tuning::RICOCHET_COS.acos().to_degrees(), 0, "°"),
        ),
        ("ricochet_speed", pct(sim::combat::weapons::tuning::RICOCHET_SPEED)),
        ("ricochet_damage", pct(sim::combat::weapons::tuning::RICOCHET_DAMAGE)),
        ("max_ricochets", sim::combat::weapons::tuning::MAX_RICOCHETS.to_string()),
        ("key_turn_start", pct(sim::flight::KEY_TURN_START)),
        (
            "key_turn_ramp",
            num(
                (1.0 - sim::flight::KEY_TURN_START) / sim::flight::KEY_TURN_GROWTH,
                1,
                "s",
            ),
        ),
        ("nudge_max", num(sim::flight::NUDGE_MAX.to_degrees(), 0, "°")),
        (
            "max_turn_rate",
            num(cfg.flight.max_turn_rate.to_degrees(), 0, "°/s"),
        ),
        (
            "assist_after",
            num(
                cfg.flight.assist_after as f32 / sim::TICK_HZ as f32,
                1,
                "s",
            ),
        ),
        ("keep_heading_fraction", pct(cfg.flight.keep_heading_fraction)),
        // The push angle off the motion up to which an engine keeps full thrust at its rated speed: the falloff
        // weighs the speed along the push, `cos² θ` of it (GDR-0089), full while that stays under the knee.
        (
            "rated_full_angle",
            num(st::RATED_KNEE.sqrt().acos().to_degrees(), 0, "°"),
        ),
    ]
}
