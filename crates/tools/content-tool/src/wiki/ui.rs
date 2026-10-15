//! `ui.json`: what the site's components show (`wiki/.vitepress/theme/components/`): the kinds with their counts,
//! parts and ships with their drawings and stats, the controls, the behavior nodes. Texts are HTML whose links go
//! through [`ROOT`]; drawings are SVG that use the site's sprite.

use super::*;
use pages::{default_inputs, role_class, signature};

/// Pixels per cell of the drawings on cards (parts fit a square of [`CARD_BOX`]).
const CARD_BOX: f32 = 64.0;

impl Wiki<'_> {
    /// Stat lines as `{stat, text, value}`, without the bonuses a part grants.
    fn stat_rows(lines: &[StatLine]) -> Vec<Value> {
        lines
            .iter()
            .filter(|l| l.bonus.is_none())
            .map(|l| json!({ "stat": l.stat, "text": l.text, "value": l.value }))
            .collect()
    }

    pub(super) fn ui(&self) -> Value {
        let c = self.c;
        let url = |kind: Kind, id: &str| item_path(kind, id);
        let kinds: Vec<Value> = KINDS
            .iter()
            .map(|&(kind, title)| {
                json!({
                    "kind": kind.key(),
                    "title": title,
                    "link": format!("/{}/", kind_dir(kind)),
                    "count": self.count(kind),
                })
            })
            .collect();
        let categories: Vec<Value> = c
            .categories
            .iter()
            .map(|k| {
                json!({
                    "id": k.id,
                    "name": k.name,
                    "url": url(Kind::Category, &k.id),
                    "color": k.color.as_deref().map(|key| self.style.get(&format!("ship.system_colors.{key}"))),
                })
            })
            .collect();
        let stats: BTreeMap<&str, Value> = c
            .stats
            .iter()
            .map(|d| {
                (
                    d.id.as_str(),
                    json!({
                        "name": d.name,
                        "url": url(Kind::Stat, &d.id),
                        "unit": d.unit,
                        "better": match d.better { Better::Higher => "higher", Better::Lower => "lower", Better::Neutral => "neutral" },
                        "summary": c.render_plain(&d.summary),
                    }),
                )
            })
            .collect();
        let parts: Vec<Value> = self
            .parts_by_category()
            .into_iter()
            .map(|p| {
                let material = if p.hull { self.default_armor() } else { None };
                json!({
                    "id": p.id,
                    "name": p.name,
                    "url": url(Kind::Part, &p.id),
                    "category": p.category,
                    "locked": p.locked,
                    "hull": p.hull,
                    "material": material.map(|m| m.name.as_str()),
                    "size": [p.size.0, p.size.1],
                    "summary": self.text(ROOT, &p.summary),
                    "svg": self.part_svg(p, material, CARD_BOX / p.size.0.max(p.size.1).max(1) as f32, "card-tile"),
                    "stats": Self::stat_rows(&c.part_stats(p, material)),
                })
            })
            .collect();
        let ships: Vec<Value> = c
            .ships
            .iter()
            .zip(&self.reports)
            .map(|(s, r)| {
                let stats: Vec<Value> = r
                    .as_ref()
                    .map(|r| {
                        self.ship_stats(r)
                            .into_iter()
                            .map(|(stat, text)| json!({ "stat": stat, "text": text }))
                            .collect()
                    })
                    .unwrap_or_default();
                json!({
                    "id": s.id,
                    "name": s.name,
                    "url": url(Kind::Ship, &s.id),
                    "faction": c.factions.get(&s.faction).map_or(s.faction.as_str(), |f| f.name.as_str()),
                    "summary": self.text(ROOT, &s.summary),
                    "svg": self.ship_svg(s, PX_CARD * 1.6, "card-ship"),
                    "stats": stats,
                })
            })
            .collect();
        let controls: Vec<Value> = c
            .controls
            .iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "name": e.name,
                    "url": url(Kind::Control, &e.id),
                    "section": hand::section_key(e.section),
                    "summary": self.text(ROOT, &e.summary),
                    "keys": default_inputs(e),
                })
            })
            .collect();
        let sections: Vec<Value> = data::input::Section::ALL
            .iter()
            .map(|&s| json!({ "id": hand::section_key(s), "title": s.title() }))
            .collect();
        let nodes: Vec<Value> = c
            .behavior_nodes
            .iter()
            .map(|n| {
                json!({
                    "id": n.id,
                    "name": n.name,
                    "url": url(Kind::BehaviorNode, &n.id),
                    "role": role_class(n.role),
                    "pilot": n.pilot,
                    "rhai": n.script.is_some(),
                    "tags": n.tags,
                    "summary": self.text(ROOT, &n.summary),
                    "signature": signature(n),
                })
            })
            .collect();
        json!({
            "root": ROOT,
            "site": c.packs.first().map(|p| p.name.as_str()),
            "kinds": kinds,
            "categories": categories,
            "stats": stats,
            "parts": parts,
            "ships": ships,
            "controls": controls,
            "sections": sections,
            "nodes": nodes,
        })
    }
}
