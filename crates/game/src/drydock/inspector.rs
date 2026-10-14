//! The SELECTED PART block of the right column ([`Inspector`]): the selected placed part with its numbers from
//! the design report, else the part picked in the palette.
//!
//! * **A placed part** (the select tool): icon, name, `2×2 · CELL 5,2 · INSTALLED` (or `IDLE` when it is over
//!   the control capacity, `FALLS OFF` when it is not on the main piece), its stat lines after adjacency bonuses
//!   (the content's stat lines with the report's values, boosted ones in gold; build points, control cost and
//!   minimum power stay in the palette), its peak heat at full use (`T / limit`, teal up to the limit,
//!   gold up to 120 % (GDR-0052), coral above) and its power level at full use, and one gold line per bonus it
//!   receives or gives (`◆ +15% FIRE RATE FROM CAPACITOR ×2`, `◆ +15% FIRE RATE TO BEAM LASER`: names and
//!   labels from content, the bonuses that apply from the report). Below: the armour pills of a hull part, the
//!   power tier pills and weight stepper of a part that draws power (GDR-0014, through `set_power`; the
//!   part's defaults are no override), and rotate, flip and delete.
//! * **A palette part**: icon, name, footprint, category (and the armour of hull parts), its summary, its stat
//!   lines from content and the bonuses it gives.
//!
//! The widgets are the panel's (`panels.rs` shows the texts and icon from [`Inspector`]); the controls below
//! are spawned and kept here.

use bevy::prelude::*;
use data::design::stats::{format_number, with_unit};
use data::{Content, Effect, PowerTier, StatDef};
use sim::{DesignReport, Stat};

use crate::flight_hud::HudFonts;
use crate::sim_bridge::GameContent;
use crate::style::{Style, ui_color};
use crate::ui_icons::{Glyph, UiIcon};
use crate::ui_kit::{Pill, Tip, Tone, label, number, ring_glyph, ring_node, set_text, spawn_pill};

use super::PlayerDesign;
use super::actions::{ActionButton, DrydockAction};
use super::report::DrydockReport;
use super::state::{DrydockUi, armor_materials};
use super::stats::heat_tone;
use super::tools::{Selection, power_setting};

/// Stat lines a placed part leaves to the palette and the stats panel (its heat limit is in PEAK HEAT, its
/// minimum power behind its POWER LEVEL).
const DESIGN_TIME: [&str; 4] = ["build_points", "control_cost", "heat_limit", "power_min_level"];

/// One stat cell of the block.
#[derive(Clone, Debug, PartialEq)]
pub struct SelLine {
    pub label: String,
    pub value: String,
    pub tone: Tone,
}

/// What the SELECTED PART block shows.
#[derive(Resource, Clone, Debug, PartialEq, Default)]
pub struct Inspector {
    /// The icon's part and armour material (`""`: none).
    pub part: String,
    pub material: Option<String>,
    pub name: String,
    pub name_tone: Tone,
    pub sub: String,
    pub sub_tone: Tone,
    pub summary: String,
    pub stats: Vec<SelLine>,
    /// Gold bonus lines (uppercase, without the diamond).
    pub bonuses: Vec<String>,
    /// The controls of a placed part; `None` for a palette part.
    pub installed: Option<Installed>,
}

/// The selected placed part's settings, for its controls.
#[derive(Clone, Debug, PartialEq)]
pub struct Installed {
    /// A hull part's armour material.
    pub material: Option<String>,
    /// A consumer's tier and weight (its override, else the part's defaults).
    pub power: Option<(PowerTier, f32)>,
}

/// Rows of controls under the block.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum ControlRow {
    Materials,
    Power,
    Actions,
}

#[derive(Component)]
pub(super) struct WeightText;

/// Recomputes the block when the selection, the picked part, the design or its report change.
pub(super) fn update(
    design: Res<PlayerDesign>,
    ui: Res<DrydockUi>,
    sel: Res<Selection>,
    report: Res<DrydockReport>,
    content: Res<GameContent>,
    mut out: ResMut<Inspector>,
) {
    if !(design.is_changed() || ui.is_changed() || sel.is_changed() || report.is_changed()) {
        return;
    }
    let content = &content.0;
    // Only a report of this revision: another design's part numbers must not show on this one's parts.
    let report = report.get().filter(|_| report.revision == Some(design.revision()));
    let want = match sel.part(&design) {
        Some(i) => placed(content, &design, report, i),
        None => picked(content, &ui),
    };
    if *out != want {
        *out = want;
    }
}

/// The part picked in the palette, from content.
fn picked(content: &Content, ui: &DrydockUi) -> Inspector {
    let Some(part) = ui.picked.as_deref().and_then(|p| content.parts.get(p)) else {
        return Inspector { name: "Nothing picked".into(), name_tone: Tone::Dim, sub: "PICK A PART IN THE LIST".into(), sub_tone: Tone::Dim, ..default() };
    };
    let material = content.materials.get(&ui.material).filter(|_| part.hull);
    let cat = content.categories.get(&part.category).map_or(String::new(), |c| c.name.to_uppercase());
    let mut sub = format!("{}×{}  ·  {cat}", part.size.0, part.size.1);
    if let Some(m) = material {
        sub.push_str(&format!("  ·  {}", m.name.to_uppercase()));
    }
    let lines = content.part_stats(part, material);
    let stats = lines
        .iter()
        .filter(|l| l.bonus.is_none())
        .map(|l| SelLine { label: l.label.to_uppercase(), value: l.text.clone(), tone: Tone::Number })
        .collect();
    let bonuses = lines
        .iter()
        .filter_map(|l| {
            let b = l.bonus.as_ref()?;
            let mut s = format!("{} {} TO ADJACENT {}", l.text, l.label, b.target_label).to_uppercase();
            if let Some(n) = b.max_stacks {
                s.push_str(&format!("  ·  MAX {n}"));
            }
            Some(s)
        })
        .collect();
    Inspector {
        part: part.id.clone(),
        material: material.map(|m| m.id.clone()),
        name: part.name.clone(),
        name_tone: Tone::Number,
        sub,
        sub_tone: Tone::Dim,
        summary: part.summary.clone(),
        stats,
        bonuses,
        installed: None,
    }
}

/// Placed part `i` of the design, with the report's numbers (content values while the design doesn't compile).
fn placed(content: &Content, design: &PlayerDesign, report: Option<&DesignReport>, i: usize) -> Inspector {
    let p = &design.dock().parts()[i];
    let Some(part) = content.parts.get(&p.part) else { return Inspector::default() };
    let pr = report.and_then(|r| r.parts.get(i));
    let idle = pr.is_some_and(|r| r.idle);
    let falls_off = report.is_some_and(|r| r.falls_off.contains(&(i as u32)));
    let (w, h) = part.footprint().dims(p.rot);
    let status = if idle { "IDLE" } else if falls_off { "FALLS OFF" } else { "INSTALLED" };
    let material = content.placement_material(p);

    // The live numbers first (heat and power at full use), then the stat lines after bonuses.
    let mut stats = Vec::new();
    if let Some(r) = pr {
        let limit = r.stats.get(Stat::HeatLimit);
        if let (Some(def), true) = (content.stats.get("peak_heat"), limit.is_finite() && part.heat.is_some()) {
            let d = content.stats.get("heat_limit").map_or(1, |d| d.decimals);
            let tone = heat_tone(r.heat_ratio);
            stats.push(SelLine { label: def.name.to_uppercase(), value: format!("{} / {}", format_number(r.heat, d), format_number(limit, d)), tone });
        }
        if part.power.as_ref().is_some_and(|s| s.draw > 0.0) {
            let (value, tone) = match r.power {
                _ if idle => ("IDLE".to_string(), Tone::Coral),
                x if x >= 0.999 => ("100 %".to_string(), Tone::Teal),
                x if x > 0.0 => (format!("{:.0} %", x * 100.0), Tone::Gold),
                _ => ("OFF".to_string(), Tone::Coral),
            };
            stats.push(SelLine { label: "POWER LEVEL".into(), value, tone });
        }
    }
    for l in content.part_stats(part, material) {
        if l.bonus.is_some() || DESIGN_TIME.contains(&l.stat.as_str()) {
            continue;
        }
        let now = pr.and_then(|r| after_bonuses(&l.stat, l.value, &r.stats));
        let (value, tone) = match now.filter(|v| (v - l.value).abs() > 1e-4 * l.value.abs().max(1.0)) {
            Some(v) => (content.stat_line(&l.stat, v).map_or(l.text.clone(), |s| s.text), Tone::Gold),
            None => (l.text.clone(), Tone::Number),
        };
        stats.push(SelLine { label: l.label.to_uppercase(), value, tone });
    }

    Inspector {
        part: part.id.clone(),
        material: p.material.clone().filter(|_| part.hull),
        name: part.name.clone(),
        name_tone: Tone::Number,
        sub: format!("{w}×{h}  ·  CELL {},{}  ·  {status}", p.x, p.y),
        sub_tone: if idle || falls_off { Tone::Coral } else { Tone::Dim },
        summary: String::new(),
        stats,
        bonuses: report.map_or_else(Vec::new, |r| bonus_lines(content, design, r, i)),
        installed: Some(Installed { material: p.material.clone().filter(|_| part.hull), power: power_setting(content, p) }),
    }
}

/// A content stat's value after bonuses, from the report's part stats; `None` for stats bonuses can't change.
fn after_bonuses(stat: &str, base: f32, stats: &sim::PartStats) -> Option<f32> {
    match stat {
        // Damage per second and shots per second scale with the fire-rate multiplier (as `GunMount::dps`).
        "damage" => Some(stats.get(Stat::Damage) * stats.get(Stat::FireRate)),
        "shot_rate" => Some(base * stats.get(Stat::FireRate)),
        _ => Stat::from_id(stat).map(|s| stats.get(s)),
    }
}

/// One line per bonus part `i` receives (grouped by stat, effect and source part) or gives (by stat, effect
/// and target part): `+15% FIRE RATE FROM CAPACITOR ×2`.
fn bonus_lines(content: &Content, design: &PlayerDesign, report: &DesignReport, i: usize) -> Vec<String> {
    let parts = design.dock().parts();
    let name = |k: u32| parts.get(k as usize).and_then(|p| content.parts.get(&p.part)).map_or("?", |p| p.name.as_str());
    let i = i as u32;
    let mut lines: Vec<(String, usize)> = Vec::new();
    for b in &report.bonuses {
        let (word, other) = match (b.target == i, b.source == i) {
            (true, _) => ("FROM", b.source),
            (false, true) => ("TO", b.target),
            _ => continue,
        };
        let Some(def) = stat_def(content, b.stat) else { continue };
        let line = format!("{} {} {word} {}", effect_text(def, b.effect), def.name, name(other)).to_uppercase();
        match lines.iter_mut().find(|(l, _)| *l == line) {
            Some((_, n)) => *n += 1,
            None => lines.push((line, 1)),
        }
    }
    lines.into_iter().map(|(l, n)| if n > 1 { format!("{l} ×{n}") } else { l }).collect()
}

/// The content definition of a sim stat.
fn stat_def(content: &Content, s: Stat) -> Option<&StatDef> {
    content.stats.iter().find(|d| Stat::from_id(&d.id) == Some(s))
}

/// `+15%`, `+2 MW`.
fn effect_text(def: &StatDef, e: Effect) -> String {
    match e {
        Effect::Percent(x) => format!("+{}%", format_number(x, 1)),
        Effect::Add(x) => with_unit(&format!("+{}", format_number(x, def.decimals)), &def.unit),
    }
}

/// The controls under the block (hidden until a placed part is selected): armour pills, power tier pills
/// and the weight stepper, rotate / flip / delete.
pub(super) fn spawn_controls(rp: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, content: &Content) {
    let ui = &style.ui;
    let c = |tone: Tone| ui_color(tone.color(ui), 1.0);
    let row = || Node { display: Display::None, flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, column_gap: px(5), row_gap: px(5), margin: UiRect::top(px(12)), flex_shrink: 0.0, ..default() };
    let glyph = |glyph: Glyph| (UiIcon::Glyph { glyph, color: ui.number }, Node { width: px(15), height: px(15), ..default() });
    rp.spawn((ControlRow::Materials, row())).with_children(|r| {
        for m in armor_materials(content) {
            spawn_pill(r, fonts, &m.name.to_uppercase(), 8.5, 24.0, None, Pill::new(Tone::Teal), ActionButton(DrydockAction::PartMaterial(m.id.clone())));
        }
    });
    rp.spawn((ControlRow::Power, Node { flex_direction: FlexDirection::Column, flex_wrap: FlexWrap::NoWrap, align_items: AlignItems::Stretch, row_gap: px(8), ..row() })).with_children(|col| {
        col.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() })
            .with_children(|h| {
                h.spawn(label(fonts, "POWER PRIORITY", 8.0, c(Tone::Label)));
                h.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(7), ..default() }).with_children(|w| {
                    w.spawn(label(fonts, "WEIGHT", 8.0, c(Tone::Label)));
                    for (up, sign) in [(false, Glyph::Minus), (true, Glyph::Plus)] {
                        if up {
                            w.spawn((WeightText, number(fonts, "", 12.0, c(Tone::Number)), Node { min_width: px(24), justify_content: JustifyContent::Center, ..default() }));
                        }
                        w.spawn((ring_node(22.0), ActionButton(DrydockAction::PartWeight(up)))).with_child(ring_glyph(sign, 10.0));
                    }
                });
            });
        col.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(4), ..default() }).with_children(|t| {
            for tier in PowerTier::ALL {
                spawn_pill(t, fonts, tier_name(tier), 8.0, 22.0, None, Pill::new(Tone::Teal), ActionButton(DrydockAction::PartTier(tier)));
            }
        });
    });
    rp.spawn((ControlRow::Actions, row())).with_children(|a| {
        for (g, action, tip) in [(Glyph::Undo, DrydockAction::RotateCcw, Tip::key("ROTATE LEFT", "Q")), (Glyph::Redo, DrydockAction::RotateCw, Tip::key("ROTATE RIGHT", "E"))] {
            a.spawn((ring_node(26.0), ActionButton(action), tip)).with_children(|r| {
                r.spawn(glyph(g));
            });
        }
        spawn_pill(a, fonts, "FLIP", 8.5, 26.0, None, Pill::new(Tone::Teal), (ActionButton(DrydockAction::Flip), Tip::key("FLIP", "F")));
        spawn_pill(a, fonts, "DELETE", 8.5, 26.0, None, Pill::new(Tone::Coral), (ActionButton(DrydockAction::Delete), Tip::key("DELETE", "DEL")));
    });
}

fn tier_name(t: PowerTier) -> &'static str {
    match t {
        PowerTier::Critical => "CRITICAL",
        PowerTier::High => "HIGH",
        PowerTier::Normal => "NORMAL",
        PowerTier::Low => "LOW",
        PowerTier::Off => "OFF",
    }
}

/// Shows the control rows that fit the selected part, its material and tier, its weight.
pub(super) fn sync_controls(
    insp: Res<Inspector>,
    mut rows: Query<(&ControlRow, &mut Node)>,
    mut pills: Query<(&ActionButton, &mut Pill)>,
    mut weight: Single<&mut Text, With<WeightText>>,
) {
    if !insp.is_changed() {
        return;
    }
    let inst = insp.installed.as_ref();
    let power = inst.and_then(|i| i.power);
    for (row, mut n) in &mut rows {
        let show = match row {
            ControlRow::Materials => inst.is_some_and(|i| i.material.is_some()),
            ControlRow::Power => power.is_some(),
            ControlRow::Actions => inst.is_some(),
        };
        let d = if show { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    for (b, mut pill) in &mut pills {
        let active = match &b.0 {
            DrydockAction::PartMaterial(m) => inst.and_then(|i| i.material.as_ref()) == Some(m),
            DrydockAction::PartTier(t) => power.is_some_and(|(tier, _)| tier == *t),
            _ => continue,
        };
        if pill.active != active {
            pill.active = active;
        }
    }
    set_text(&mut weight, &power.map_or(String::new(), |(_, w)| format_number(w, 2)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner_design(content: &Content) -> PlayerDesign {
        let mut d = PlayerDesign::starter(content, "halvex_runner");
        d.edit(|dock| dock.mirror = false);
        d
    }

    fn index_of(d: &PlayerDesign, id: &str) -> usize {
        d.dock().parts().iter().position(|p| p.part == id).unwrap()
    }

    #[test]
    fn a_placed_part_shows_the_reports_numbers_and_bonuses() {
        let content = data::base_content();
        let d = runner_design(&content);
        let report = sim::design_report(&content, d.blueprint(), &sim::SimConfig::default()).unwrap();
        // A beam laser next to capacitors fires faster: its bonuses arrive from them, in gold numbers.
        let beam = index_of(&d, "beam_laser");
        let insp = placed(&content, &d, Some(&report), beam);
        assert_eq!(insp.name, content.parts.get("beam_laser").unwrap().name);
        assert!(insp.sub.contains("INSTALLED") && insp.sub.contains("CELL"), "{}", insp.sub);
        assert!(insp.stats.iter().any(|l| l.label == "PEAK HEAT" && l.value.contains(" / ")), "{:?}", insp.stats);
        assert!(insp.stats.iter().any(|l| l.label == "POWER LEVEL"), "{:?}", insp.stats);
        assert!(!insp.stats.iter().any(|l| l.label == "BUILD POINTS"));
        let received: Vec<_> = report.bonuses.iter().filter(|b| b.target as usize == beam).collect();
        assert_eq!(insp.bonuses.is_empty(), received.is_empty(), "{:?}", insp.bonuses);
        if let Some(b) = received.first() {
            let src = &content.parts.get(&d.dock().parts()[b.source as usize].part).unwrap().name;
            assert!(insp.bonuses.iter().any(|l| l.contains("FROM") && l.contains(&src.to_uppercase())), "{:?}", insp.bonuses);
            // A faster beam does more damage per second: the boosted number is gold.
            assert!(insp.stats.iter().any(|l| l.label == "DAMAGE" && l.tone == Tone::Gold), "{:?}", insp.stats);
        }
        assert!(insp.installed.as_ref().is_some_and(|i| i.power.is_some() && i.material.is_none()));
        // A hull part: its material, no power.
        let block = index_of(&d, "armor_block");
        let insp = placed(&content, &d, Some(&report), block);
        let inst = insp.installed.unwrap();
        assert!(inst.material.is_some() && inst.power.is_none());
        // A source names its targets.
        if let Some(b) = report.bonuses.first() {
            let insp = placed(&content, &d, Some(&report), b.source as usize);
            assert!(insp.bonuses.iter().any(|l| l.contains(" TO ")), "{:?}", insp.bonuses);
        }
    }

    #[test]
    fn a_palette_part_shows_content_numbers() {
        let content = data::base_content();
        let d = runner_design(&content);
        let mut ui = DrydockUi::new(&content, &d, &data::career::Unlocks::all(&content));
        ui.picked = Some("capacitor".into());
        let insp = picked(&content, &ui);
        assert_eq!(insp.installed, None);
        assert!(insp.stats.iter().any(|l| l.label == "BUILD POINTS"));
        assert!(insp.bonuses.iter().any(|l| l.contains("TO ADJACENT")), "{:?}", insp.bonuses);
        ui.picked = None;
        assert_eq!(picked(&content, &ui).name, "Nothing picked");
    }
}
