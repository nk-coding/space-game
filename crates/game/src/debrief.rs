//! The debrief (GDR-0065, after `docs/plans/mockups/home-b.png`): after a voyage, a full panel over home's right
//! side until it is dismissed (DISMISS, Esc or Enter), then a one-line summary top left. A layer of its own over
//! [`AppMode::Home`] (the shipyard), independent of home's layout; hidden while FLEET is open.
//!
//! ```text
//! VOYAGE HOME · HAUL UNLOADED
//! Shipyard approach
//! [RIM] [GOAL CLEARED] [FIRST CLEAR]
//! TIME 7:42   SITES 4 / 4   KILLS 9   SHIPS LOST 0
//! 14 PARTS DISMANTLED
//!   Automaton cannon ×6              KINETIC WEAPONS +18
//! RESEARCH DATA
//!   • KINETIC WEAPONS                  +18   IN STORE 18
//! ◆ PROGRAMMABLE MATTER +1
//! RESEARCH READY · 1
//!   Reverse-engineered cannon
//!   ✓ HALVEX DRIFT IS NOW OPEN
//!                                            [DISMISS ESC]
//! ```
//!
//! * [`DebriefData`] is built from the career before and after the voyage's end (`voyage.rs`), the voyage's haul
//!   and what the flight counted ([`DebriefStats`]: sites, kills, ships lost). Kills and ships lost are not in the
//!   save, so a game loaded with a `last_haul` shows only the one-line summary of it.
//! * Dismissed is not saved: the next start shows the summary.
//! * A voyage it opened adds VOYAGE MAP beside DISMISS: it dismisses the panel and flies to the gate
//!   (`map::OpenMap`), where the map selects the newly open voyage.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use bevy::prelude::*;
use data::career::{Haul, HaulItem, LastHaul, Outcome, unload};
use data::{Career, Content};

use crate::career::PlayerCareer;
use crate::fleet::{self, Fleet};
use crate::flight_hud::HudFonts;
use crate::mode::AppMode;
use crate::sim_bridge::GameContent;
use crate::style::{Style, ui_color};
use crate::ui_kit::{Hot, Pill, Tone, UiBlock, UiClick, UiInputSet, UiPointer, UiStyleSet, label, number, prose, spawn_pill, tracked};
use crate::voyage::clock;

/// Rows of parts shown (the rest are counted in one line).
const MAX_ROWS: usize = 6;
/// The panel's width (logical px).
const WIDTH: f32 = 420.0;

/// What the flight counted, beside the career's last haul.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DebriefStats {
    /// Sites done and all (`SITES 4 / 4`).
    pub sites: Option<(u32, u32)>,
    pub kills: Option<u32>,
    pub ships_lost: Option<u32>,
    /// Items left in the wrecks (a lost voyage).
    pub left: u32,
    /// Why the voyage was a practice run (not saved).
    pub practice: Option<String>,
}

/// One kind of item that came home.
#[derive(Clone, Debug, PartialEq)]
pub struct PartRow {
    pub name: String,
    pub count: u32,
    /// What it gave: `KINETIC +18  ·  PROGRAMMABLE MATTER +1`.
    pub gives: String,
    /// The family colour key of its research data (none: neutral).
    pub color: Option<String>,
}

/// One family's research data.
#[derive(Clone, Debug, PartialEq)]
pub struct FamilyLine {
    pub name: String,
    pub gained: f32,
    pub store: f32,
    pub color: Option<String>,
}

/// A voyage's debrief.
#[derive(Clone, Debug, PartialEq)]
pub struct DebriefData {
    pub voyage: String,
    pub name: String,
    pub band: String,
    pub outcome: Outcome,
    pub ticks: u64,
    pub stats: DebriefStats,
    /// Items dismantled (parts and fragments).
    pub items: u32,
    /// The goal was cleared for the first time, and the voyage's words for it.
    pub first_clear: bool,
    pub cleared_text: Option<String>,
    pub rows: Vec<PartRow>,
    pub families: Vec<FamilyLine>,
    /// Materials gained: name, amount.
    pub materials: Vec<(String, u32)>,
    /// Research nodes that can be paid now (names, content order).
    pub ready: Vec<String>,
    /// Voyages this one opened (names).
    pub opened: Vec<String>,
    pub practice: Option<String>,
}

/// Research data as the debrief writes it: whole numbers plain, else one decimal.
pub fn amount(x: f32) -> String {
    if (x - x.round()).abs() < 0.05 { format!("{:.0}", x.round()) } else { format!("{x:.1}") }
}

impl DebriefData {
    /// The debrief of `last` (the career's last haul), from the career `before` and `after` the end, the `haul`
    /// that came home (its items, for the rows) and what the flight counted.
    pub fn build(content: &Content, before: &Career, after: &Career, last: &LastHaul, haul: &Haul, stats: DebriefStats) -> DebriefData {
        let voyage = content.voyages.get(&last.voyage);
        let was_cleared = before.voyage(&last.voyage).is_some_and(|r| r.cleared);
        let first_clear = !was_cleared && after.voyage(&last.voyage).is_some_and(|r| r.cleared);
        let family = |id: &str| content.families.get(id);
        let upper = |s: &str| s.to_uppercase();

        // Rows: per part (fragments apart), what its items gave, most first.
        let mut rows: BTreeMap<(bool, String), (u32, BTreeMap<String, f32>, BTreeMap<String, u32>)> = BTreeMap::new();
        for (item, &n) in &haul.items {
            let one = unload(content, &[(item.clone(), n)].into_iter().collect());
            let key = match item {
                HaulItem::Part { part, .. } => (false, part.clone()),
                HaulItem::Fragment { part } => (true, part.clone()),
            };
            let row = rows.entry(key).or_default();
            row.0 = row.0.saturating_add(n);
            for (f, x) in one.data {
                *row.1.entry(f).or_default() += x;
            }
            for (m, x) in one.materials {
                *row.2.entry(m).or_default() += x;
            }
        }
        let mut rows: Vec<PartRow> = rows
            .into_iter()
            .map(|((fragment, part), (count, data, materials))| {
                let base = content.parts.get(&part).map_or(part.clone(), |p| p.name.clone());
                let name = if fragment { format!("{base} fragment") } else { base };
                let mut gives: Vec<String> = data.iter().map(|(f, x)| format!("{} +{}", upper(family(f).map_or(f, |f| &f.name)), amount(*x))).collect();
                gives.extend(materials.iter().map(|(m, x)| format!("{} +{x}", upper(content.materials.get(m).map_or(m, |m| &m.name)))));
                let color = data.keys().next().and_then(|f| family(f)).and_then(|f| f.color.clone());
                PartRow { name, count, gives: gives.join("  ·  "), color }
            })
            .collect();
        rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));

        let families = last
            .report
            .data
            .iter()
            .map(|(f, &gained)| FamilyLine {
                name: upper(family(f).map_or(f, |f| &f.name)),
                gained,
                store: after.data().get(f).copied().unwrap_or(0.0),
                color: family(f).and_then(|f| f.color.clone()),
            })
            .collect();
        let materials = last.report.materials.iter().map(|(m, &n)| (content.materials.get(m).map_or(m.clone(), |m| m.name.clone()), n)).collect();
        let ready = content.research.iter().filter(|n| after.can_pay(content, &n.id).is_ok()).map(|n| n.name.clone()).collect();
        let open_before = before.open_voyages(content);
        let opened = after
            .open_voyages(content)
            .into_iter()
            .filter(|v| !open_before.contains(v))
            .map(|v| content.voyages.get(v).map_or(v.to_string(), |v| v.name.clone()))
            .collect();
        DebriefData {
            voyage: last.voyage.clone(),
            name: voyage.map_or(last.voyage.clone(), |v| v.name.clone()),
            band: voyage.map_or(String::new(), |v| upper(&v.band.replace('_', " "))),
            outcome: last.outcome,
            ticks: last.ticks,
            practice: stats.practice.clone(),
            stats,
            items: last.report.items(),
            first_clear,
            cleared_text: first_clear.then(|| voyage.and_then(|v| v.cleared.as_deref()).map(|t| content.render_plain(t))).flatten(),
            rows,
            families,
            materials,
            ready,
            opened,
        }
    }

    /// The panel's head line.
    pub fn head(&self) -> (&'static str, Tone) {
        match (&self.practice, self.outcome) {
            (Some(_), _) => ("PRACTICE RUN  ·  NOT SAVED", Tone::Gold),
            (None, Outcome::Lost) => ("SHIP LOST  ·  RECALLED HOME", Tone::Coral),
            (None, _) => ("VOYAGE HOME  ·  HAUL UNLOADED", Tone::Label),
        }
    }

    /// The outcome chip.
    pub fn outcome_chip(&self) -> (&'static str, Tone) {
        match self.outcome {
            Outcome::Cleared => ("GOAL CLEARED", Tone::Teal),
            Outcome::Recalled => ("RECALLED", Tone::Label),
            Outcome::Lost => ("LOST", Tone::Coral),
        }
    }

    /// The one line home shows once the panel is dismissed: `LAST VOYAGE · SHIPYARD APPROACH · GOAL CLEARED · 7:42
    /// · 14 PARTS → KINETIC +18 · PROPULSION +9`.
    pub fn summary(&self) -> String {
        let mut s = String::from(if self.practice.is_some() { "PRACTICE RUN" } else { "LAST VOYAGE" });
        let _ = write!(s, "  ·  {}  ·  {}  ·  {}", self.name.to_uppercase(), self.outcome_chip().0, clock(self.ticks));
        if self.outcome == Outcome::Lost {
            s.push_str("  ·  THE HAUL STAYED BEHIND");
            return s;
        }
        let _ = write!(s, "  ·  {} {}", self.items, if self.items == 1 { "PART" } else { "PARTS" });
        let mut first = true;
        for f in &self.families {
            let _ = write!(s, "{}{} +{}", if first { " → " } else { "  ·  " }, f.name, amount(f.gained));
            first = false;
        }
        for (m, n) in &self.materials {
            let _ = write!(s, "{}{} +{n}", if first { " → " } else { "  ·  " }, m.to_uppercase());
            first = false;
        }
        s
    }
}

/// The last debrief and whether its panel is open (a resource).
#[derive(Resource, Default, Debug)]
pub struct Debrief {
    pub data: Option<DebriefData>,
    pub open: bool,
}

impl Debrief {
    /// A voyage ended: its panel opens at home.
    pub fn show(&mut self, data: DebriefData) {
        self.data = Some(data);
        self.open = true;
    }
}

/// The panel's root (its content is rebuilt whenever the debrief changes).
#[derive(Component)]
struct PanelRoot;

/// The summary line's text.
#[derive(Component)]
struct SummaryText;

#[derive(Component)]
struct DismissButton;

/// VOYAGE MAP (after a voyage that opened another).
#[derive(Component)]
struct MapButton;

pub struct DebriefPlugin;

impl Plugin for DebriefPlugin {
    fn build(&self, app: &mut App) {
        // A game loaded with a last haul starts with its summary (the panel was seen when it came home).
        let debrief = {
            let world = app.world();
            let content = &world.resource::<GameContent>().0;
            let career = world.resource::<PlayerCareer>().career();
            let data = career.last_haul().map(|last| DebriefData::build(content, career, career, last, &Haul::default(), DebriefStats::default()));
            Debrief { data, open: false }
        };
        app.insert_resource(debrief)
            .add_systems(Startup, spawn.after(crate::flight_hud::spawn_hud))
            .add_systems(Update, (dismiss, rebuild, show).chain().after(UiInputSet).before(UiStyleSet));
    }
}

fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, clear: Res<ClearColor>) {
    commands.spawn((
        PanelRoot,
        UiBlock,
        // Hot, so a click on the panel reaches nothing under it.
        Hot::default(),
        GlobalZIndex(10),
        Visibility::Hidden,
        BackgroundColor(ui_color(clear.0, 0.94)),
        BorderColor::all(ui_color(style.ui.dim, 0.5)),
        Node {
            position_type: PositionType::Absolute,
            right: px(0),
            top: px(0),
            bottom: px(0),
            width: px(WIDTH),
            border: UiRect::left(px(1)),
            flex_direction: FlexDirection::Column,
            padding: UiRect::new(px(30), px(30), px(26), px(24)),
            row_gap: px(14),
            overflow: Overflow::clip(),
            ..default()
        },
    ));
    commands.spawn((
        SummaryText,
        Visibility::Hidden,
        tracked(&fonts, "", 10.5, ui_color(style.ui.label, 1.0), 0.18),
        // Top left under the shipyard's name (home's notes are bottom right).
        Node { position_type: PositionType::Absolute, left: px(28), top: px(76), max_width: px(640), ..default() },
    ));
}

/// DISMISS, Esc or Enter close the panel.
#[allow(clippy::too_many_arguments)]
fn dismiss(
    mode: Res<State<AppMode>>,
    fleet: Res<Fleet>,
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Res<UiPointer>,
    mut clicks: MessageReader<UiClick>,
    buttons: Query<(), With<DismissButton>>,
    map_buttons: Query<(), With<MapButton>>,
    mut debrief: ResMut<Debrief>,
    mut open_map: MessageWriter<crate::map::OpenMap>,
) {
    let (mut clicked, mut to_map) = (false, false);
    for c in clicks.read() {
        clicked |= buttons.contains(c.0);
        to_map |= map_buttons.contains(c.0);
    }
    if !debrief.open || *mode.get() != AppMode::Home || fleet.open {
        return;
    }
    let key = !pointer.typing && keys.any_just_pressed([KeyCode::Escape, KeyCode::Enter, KeyCode::NumpadEnter]);
    if to_map {
        open_map.write(crate::map::OpenMap);
    }
    if clicked || key || to_map {
        debrief.open = false;
    }
}

/// Shown at home only (not while FLEET is open): the panel while open, else the summary.
fn show(
    mode: Res<State<AppMode>>,
    fleet: Res<Fleet>,
    debrief: Res<Debrief>,
    mut panel: Single<&mut Visibility, (With<PanelRoot>, Without<SummaryText>)>,
    mut summary: Single<&mut Visibility, (With<SummaryText>, Without<PanelRoot>)>,
) {
    let home = *mode.get() == AppMode::Home && !fleet::is_open(fleet);
    let has = debrief.data.is_some();
    let vis = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
    let (p, s) = (vis(home && has && debrief.open), vis(home && has && !debrief.open));
    if **panel != p {
        **panel = p;
    }
    if **summary != s {
        **summary = s;
    }
}

/// Rebuilds the panel and the summary when the debrief changes.
fn rebuild(
    mut commands: Commands,
    debrief: Res<Debrief>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    panel: Single<Entity, With<PanelRoot>>,
    mut summary: Single<&mut Text, With<SummaryText>>,
) {
    if !debrief.is_changed() {
        return;
    }
    let Some(d) = &debrief.data else { return };
    crate::ui_kit::set_text(&mut summary, &d.summary());
    commands.entity(*panel).despawn_children();
    commands.entity(*panel).with_children(|p| fill(p, &fonts, &style, d));
}

/// The panel's content.
fn fill(p: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, d: &DebriefData) {
    let ui = style.ui;
    let c = |tone: Tone| ui_color(tone.color(&ui), 1.0);
    let fam = |key: &Option<String>| key.as_deref().map_or(ui_color(ui.label, 1.0), |k| ui_color(style.system_color(k), 1.0));
    let row = |gap: f32| Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(gap), ..default() };
    let rule = |p: &mut ChildSpawnerCommands| {
        p.spawn((Node { height: px(1), flex_shrink: 0.0, ..default() }, BackgroundColor(ui_color(ui.dim, 0.35))));
    };
    let (head, tone) = d.head();
    p.spawn(label(fonts, head, 10.5, c(tone)));
    p.spawn(prose(fonts, &d.name, 26.0, c(Tone::Number)));
    // Chips: band, outcome, first clear.
    p.spawn(row(8.0)).with_children(|r| {
        let mut chip = |text: &str, tone: Tone| {
            r.spawn((
                Node { padding: UiRect::axes(px(10), px(4)), border: UiRect::all(px(1)), border_radius: BorderRadius::MAX, ..default() },
                BorderColor::all(ui_color(tone.color(&ui), 0.7)),
            ))
            .with_children(|ch| {
                ch.spawn(label(fonts, text, 9.0, c(tone)));
            });
        };
        if !d.band.is_empty() {
            chip(&d.band, Tone::Label);
        }
        let (o, t) = d.outcome_chip();
        chip(o, t);
        if d.first_clear {
            chip("FIRST CLEAR", Tone::Gold);
        }
    });
    // Time, sites, kills, ships lost.
    p.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(10.0) }).with_children(|r| {
        let opt = |v: Option<u32>| v.map_or("–".to_string(), |v| v.to_string());
        let sites = d.stats.sites.map_or("–".to_string(), |(a, b)| format!("{a} / {b}"));
        for (head, value, tone) in [
            ("TIME", clock(d.ticks), Tone::Number),
            ("SITES", sites, Tone::Teal),
            ("KILLS", opt(d.stats.kills), Tone::Number),
            ("SHIPS LOST", opt(d.stats.ships_lost), if d.stats.ships_lost.unwrap_or(0) > 0 { Tone::Coral } else { Tone::Number }),
        ] {
            r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|col| {
                col.spawn(label(fonts, head, 9.0, c(Tone::Label)));
                col.spawn(number(fonts, &value, 20.0, c(tone)));
            });
        }
    });
    rule(p);
    if d.outcome == Outcome::Lost {
        let left = if d.stats.left > 0 { format!("{} PARTS STAYED IN THE WRECK", d.stats.left) } else { "NOTHING CAME HOME".into() };
        p.spawn(label(fonts, &left, 9.5, c(Tone::Coral)));
        p.spawn(prose(fonts, "The haul of a lost voyage stays behind. Your ships re-formed at home, whole.", 13.0, c(Tone::Label)));
    } else {
        let parts = format!("{} {} DISMANTLED", d.items, if d.items == 1 { "PART" } else { "PARTS" });
        p.spawn(label(fonts, &parts, 9.5, c(Tone::Label)));
        if d.rows.is_empty() {
            p.spawn(prose(fonts, "The holds came home empty.", 13.0, c(Tone::Dim)));
        }
        for r in d.rows.iter().take(MAX_ROWS) {
            p.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(10.0) }).with_children(|line| {
                line.spawn(row(8.0)).with_children(|l| {
                    l.spawn((Node { width: px(7), height: px(7), border_radius: BorderRadius::MAX, ..default() }, BackgroundColor(fam(&r.color))));
                    l.spawn(prose(fonts, &format!("{} ×{}", r.name, r.count), 14.0, c(Tone::Number)));
                });
                line.spawn(label(fonts, &r.gives, 8.5, fam(&r.color)));
            });
        }
        if d.rows.len() > MAX_ROWS {
            let more: u32 = d.rows[MAX_ROWS..].iter().map(|r| r.count).sum();
            p.spawn(label(fonts, &format!("+ {more} MORE"), 9.0, c(Tone::Dim)));
        }
        if !d.families.is_empty() || !d.materials.is_empty() {
            rule(p);
            p.spawn(label(fonts, "RESEARCH DATA", 9.5, c(Tone::Label)));
            for f in &d.families {
                p.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(10.0) }).with_children(|line| {
                    line.spawn(row(8.0)).with_children(|l| {
                        l.spawn((Node { width: px(7), height: px(7), border_radius: BorderRadius::MAX, ..default() }, BackgroundColor(fam(&f.color))));
                        l.spawn(label(fonts, &f.name, 10.0, c(Tone::Number)));
                    });
                    line.spawn(row(14.0)).with_children(|l| {
                        l.spawn(number(fonts, &format!("+{}", amount(f.gained)), 15.0, c(Tone::Number)));
                        l.spawn(label(fonts, &format!("IN STORE {}", amount(f.store)), 8.5, c(Tone::Dim)));
                    });
                });
            }
            for (m, n) in &d.materials {
                p.spawn(label(fonts, &format!("◆ {} +{n}", m.to_uppercase()), 9.5, c(Tone::Gold)));
            }
        }
    }
    if !d.ready.is_empty() || !d.opened.is_empty() || d.cleared_text.is_some() {
        rule(p);
    }
    if !d.ready.is_empty() {
        p.spawn(label(fonts, &format!("RESEARCH READY  ·  {}", d.ready.len()), 9.5, c(Tone::Teal)));
        for name in d.ready.iter().take(3) {
            p.spawn(prose(fonts, name, 15.0, c(Tone::Number)));
        }
    }
    for v in &d.opened {
        p.spawn(label(fonts, &format!("✓ {} IS NOW OPEN", v.to_uppercase()), 9.5, c(Tone::Teal)));
    }
    if let Some(t) = &d.cleared_text {
        p.spawn(prose(fonts, t, 13.0, c(Tone::Label)));
    }
    if let Some(why) = &d.practice {
        p.spawn(label(fonts, why, 9.0, c(Tone::Gold)));
    }
    // DISMISS at the bottom.
    p.spawn(Node { flex_grow: 1.0, ..default() });
    p.spawn(Node { justify_content: JustifyContent::FlexEnd, ..row(10.0) }).with_children(|r| {
        if !d.opened.is_empty() {
            spawn_pill(r, fonts, "VOYAGE MAP", 10.0, 36.0, None, Pill::new(Tone::Teal), MapButton);
        }
        spawn_pill(r, fonts, "DISMISS  ESC", 10.0, 36.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, DismissButton);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::career::VoyageEnd;

    fn career(content: &Content) -> Career {
        Career::new(content, "rim_awakening", 4207).unwrap()
    }

    /// A first clear: rows by part with what they gave, data per family with the store, research ready, the next
    /// voyage open, the voyage's words; then the one-line summary.
    #[test]
    fn a_cleared_voyage_debriefs_its_haul() {
        let content = data::base_content();
        let before = {
            let mut c = career(&content);
            c.voyage_launched(&content, "yard_approach").unwrap();
            c
        };
        let haul: Haul = [
            (HaulItem::Part { part: "automaton_cannon".into(), material: None }, 6),
            (HaulItem::Part { part: "thruster".into(), material: None }, 3),
            (HaulItem::Fragment { part: "automaton_core".into() }, 1),
        ]
        .into_iter()
        .collect();
        let mut after = before.clone();
        let last = after.voyage_ended(&content, "yard_approach", VoyageEnd::Recalled { haul: haul.clone(), cleared: true, ticks: 30 * 462 }).unwrap().clone();
        let stats = DebriefStats { sites: Some((4, 4)), kills: Some(9), ships_lost: Some(0), ..default() };
        let d = DebriefData::build(&content, &before, &after, &last, &haul, stats);
        assert_eq!((d.name.as_str(), d.band.as_str(), d.items, d.first_clear), ("Shipyard approach", "RIM", 10, true));
        assert_eq!(d.head(), ("VOYAGE HOME  ·  HAUL UNLOADED", Tone::Label));
        assert_eq!(d.outcome_chip().0, "GOAL CLEARED");
        let cannon = &d.rows[0];
        assert_eq!((cannon.name.as_str(), cannon.count), ("Automaton cannon", 6));
        assert!(cannon.gives.starts_with("KINETIC WEAPONS +"), "{}", cannon.gives);
        let fragment = d.rows.iter().find(|r| r.name.ends_with("fragment")).unwrap();
        assert!(fragment.gives.contains("PROGRAMMABLE MATTER +1"), "{}", fragment.gives);
        let kinetic = d.families.iter().find(|f| f.name == "KINETIC WEAPONS").unwrap();
        assert_eq!(kinetic.gained, kinetic.store);
        assert_eq!(d.opened, ["Halvex Drift"]);
        assert!(d.cleared_text.as_deref().is_some_and(|t| t.contains("Halvex Drift") && !t.contains("[[")), "{:?}", d.cleared_text);
        assert!(!d.ready.is_empty(), "the cannon data pays for the cannon node");
        let line = d.summary();
        assert!(line.starts_with("LAST VOYAGE  ·  SHIPYARD APPROACH  ·  GOAL CLEARED  ·  7:42  ·  10 PARTS → KINETIC WEAPONS +"), "{line}");
        // From the save alone (no rows, no counts): the same summary.
        let loaded = DebriefData::build(&content, &after, &after, &last, &Haul::default(), DebriefStats::default());
        assert!(loaded.rows.is_empty() && !loaded.first_clear && loaded.stats.kills.is_none());
        assert_eq!(loaded.summary(), line);
    }

    #[test]
    fn a_lost_voyage_debriefs_what_stayed_behind() {
        let content = data::base_content();
        let mut before = career(&content);
        before.voyage_launched(&content, "yard_approach").unwrap();
        let mut after = before.clone();
        let last = after.voyage_ended(&content, "yard_approach", VoyageEnd::Lost { ticks: 30 * 61 }).unwrap().clone();
        let d = DebriefData::build(&content, &before, &after, &last, &Haul::default(), DebriefStats { left: 5, ..default() });
        assert_eq!(d.head(), ("SHIP LOST  ·  RECALLED HOME", Tone::Coral));
        assert_eq!(d.summary(), "LAST VOYAGE  ·  SHIPYARD APPROACH  ·  LOST  ·  1:01  ·  THE HAUL STAYED BEHIND");
        assert!(d.opened.is_empty() && !d.first_clear);
        assert_eq!((amount(18.0), amount(4.5), amount(0.04)), ("18".into(), "4.5".into(), "0".into()));
    }
}
