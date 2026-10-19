//! The LOG (GDR-0091): a layer over home that lists the last voyages, newest first: where each went, how it ended,
//! how long it took, what came home and what that gave. It replaced the one-line summary that sat over home after
//! the debrief.
//!
//! ```text
//! (o) SHIPYARD › LOG  [EVOLVING 12 / 50]          VOYAGES 12  HOME 9  LOST 3  SALVAGED 140   [CLOSE]
//!     Log
//!   THE LAST 12 VOYAGES  ·  NEWEST FIRST
//!   NEW  The Warden's Gate    GOAL CLEARED   8:15   7 PARTS  »  HULL AND ARMOUR +1  ·  PROGRAMMABLE MATTER +6
//!        Shipyard approach    LOST           1:01   THE HAUL STAYED BEHIND
//!   …                                                                                          (scrolls)
//! ```
//!
//! * Home's records mast by the gate (L, once the camera has flown there) opens it; the ring button top left,
//!   CLOSE and Esc close it, and the camera flies back out.
//! * What it lists is the career's history (`data::career::Career::history`, the last
//!   [`data::career::MAX_HISTORY`]); the counts top right are the career's log, which keeps every voyage.
//! * A voyage's result is new until the LOG opens: the mast glows meanwhile (`shipyard::lights`) and its row is
//!   tagged NEW this once; opening marks them read and writes the save.
//! * The mast's tooltip is the last voyage in one line ([`LogRow::line`]).
//! * **Logs to decode** (galaxy plan E6, GDR-0097: a data core brought home is decoded here, free, GDR-0080) head the
//!   list: one row per core ([`cores`]: the source's name and summary from the content) with DECODE, which turns it
//!   into a signal (`Career::decode`, saved): the row then shows the signal's words (`SIGNAL · A GATE`, its source's
//!   line) and `ON THE MAP`, until the LOG closes. The mast glows while a core waits.
//! * **The memory log** (galaxy plan E8, GDR-0080, GDR-0098): the lore fragments found at the ends of lore trails
//!   ([`memories`]: each one's name and text from the content), newest first, after the logs.
//!
//! UI words are chrome in code, like home's; the voyages', families' and materials' names are content.

use bevy::prelude::*;
use data::{Career, Content};
use data::career::{Outcome, TrailSignal, VoyageResult};
use kit::style::{Style, ui_color};
use kit::ui_kit::{CompactText, Edge, HudFonts, Pill, ScreenFx, ScreenPart, Scrollbar, Tip, Tone, UiClick, UiInputSet, UiStyleSet, frame, label, number, prose, show_screen, spawn_close, spawn_head, spawn_pill, tracked};
use world::career::{Note, PlayerCareer};
use world::mode::AppMode;
use world::sim_bridge::GameContent;
use world::storage::Storage;
use world::voyage::{clock, signal_words};
use world::voyage::debrief::{amount, outcome_words};

use crate::home::{COMPACT_TEXT, HomeAction, by_class};

/// The stage and the list's box on it, like FLEET's: 1280 × 720, and on a phone 844 × 390 at `compact_scale` with the
/// list across it (docs/client.md "Phones").
const STAGE: (Vec2, Vec2) = (Vec2::new(1280.0, 720.0), Vec2::new(844.0, 390.0));
const LIST: [(Vec2, Vec2); 2] = [(Vec2::new(120.0, 104.0), Vec2::new(1040.0, 580.0)), (Vec2::new(16.0, 64.0), Vec2::new(812.0, 312.0))];
/// A row's height.
const ROW: f32 = 38.0;

/// One voyage as the LOG shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct LogRow {
    /// The voyage's name (content), as written.
    pub name: String,
    pub outcome: Outcome,
    /// How long it took (`8:15`).
    pub time: String,
    /// Items that came home (none for a lost voyage).
    pub parts: Option<u32>,
    /// What they gave: research data per family, then materials (`HULL AND ARMOUR +1`).
    pub gains: Vec<String>,
    /// Not looked at in the LOG before.
    pub new: bool,
}

impl LogRow {
    pub fn of(content: &Content, v: &VoyageResult) -> LogRow {
        let lost = v.outcome == Outcome::Lost;
        let families = v.report.data.iter().map(|(f, x)| format!("{} +{}", content.families.get(f).map_or(f.as_str(), |f| f.name.as_str()).to_uppercase(), amount(*x)));
        let materials = v.report.materials.iter().map(|(m, n)| format!("{} +{n}", content.materials.get(m).map_or(m.as_str(), |m| m.name.as_str()).to_uppercase()));
        LogRow {
            name: content.voyages.get(&v.voyage).map_or_else(|| v.voyage.clone(), |x| x.name.clone()),
            outcome: v.outcome,
            time: clock(v.ticks),
            parts: (!lost).then(|| v.report.items()),
            gains: families.chain(materials).collect(),
            new: v.new,
        }
    }

    /// What came home: `7 PARTS  »  HULL AND ARMOUR +1  ·  PROGRAMMABLE MATTER +6`, or that nothing did. `»` for
    /// "turned into": Barlow has no arrows (an `→` showed as a box).
    pub fn haul(&self) -> String {
        let Some(n) = self.parts else { return "THE HAUL STAYED BEHIND".into() };
        let parts = format!("{n} {}", if n == 1 { "PART" } else { "PARTS" });
        if self.gains.is_empty() { parts } else { format!("{parts}  »  {}", self.gains.join("  ·  ")) }
    }

    /// The voyage in one line (the mast's tooltip): `LAST VOYAGE  ·  THE WARDEN'S GATE  ·  GOAL CLEARED  ·  8:15  ·
    /// 7 PARTS  »  …`.
    pub fn line(&self) -> String {
        format!("LAST VOYAGE  ·  {}  ·  {}  ·  {}  ·  {}", self.name.to_uppercase(), outcome_words(self.outcome).0, self.time, self.haul())
    }
}

/// A data core at the LOG: waiting to be decoded, or decoded while the LOG is open.
#[derive(Clone, Debug, PartialEq)]
pub struct CoreRow {
    /// The source's name and summary (`Hauler's log`), as the content writes them.
    pub name: String,
    pub summary: String,
    /// Decoded: the signal's words (`A GATE`, `a log from a hauler's wreck`); `None` waiting (its index in the
    /// career's logs is the row's).
    pub signal: Option<(String, String)>,
}

/// The logs waiting in the career (in its order), then those decoded since the LOG opened.
pub fn cores(content: &Content, career: &Career, decoded: &[TrailSignal]) -> Vec<CoreRow> {
    let Some(g) = career.exploration() else { return Vec::new() };
    let source = |s: &TrailSignal| g.skeleton().trails.get(s.trail as usize).and_then(|t| t.signals.get(s.signal as usize)).and_then(|x| content.signal_sources.get(&x.source));
    let row = |s: &TrailSignal, signal| CoreRow {
        name: source(s).map_or_else(|| "A data core".to_string(), |x| x.name.clone()),
        summary: source(s).map_or_else(String::new, |x| x.summary.clone()),
        signal,
    };
    let waiting = g.logs().iter().map(|s| row(s, None));
    waiting.chain(decoded.iter().map(|s| row(s, Some(signal_words(content, career, *s))))).collect()
}

/// The memory log: the fragments found (name, text), newest first.
pub fn memories(content: &Content, career: &Career) -> Vec<(String, String)> {
    let Some(g) = career.exploration() else { return Vec::new() };
    g.memories().iter().rev().filter_map(|l| content.lore.get(l)).map(|l| (l.name.clone(), l.text.clone())).collect()
}

/// The career's history as rows, newest first.
pub fn rows(content: &Content, history: &[VoyageResult]) -> Vec<LogRow> {
    history.iter().rev().map(|v| LogRow::of(content, v)).collect()
}

/// The LOG's state (a resource): open or not, and the logs decoded since it opened.
#[derive(Resource, Debug, Default)]
pub struct VoyageLog {
    pub open: bool,
    decoded: Vec<TrailSignal>,
}

#[derive(Component)]
struct LogRoot;

/// The list's node (its rows are built again each time the LOG opens).
#[derive(Component)]
struct LogList;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum LogText {
    Voyages,
    Home,
    Lost,
    Salvaged,
}

#[derive(Component)]
struct CloseButton;

/// DECODE on the `n`th waiting log.
#[derive(Component)]
struct DecodeButton(usize);

pub struct VoyageLogPlugin;

impl Plugin for VoyageLogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoyageLog>()
            .add_systems(Startup, spawn.after(world::flight_hud::spawn_hud))
            .add_systems(OnExit(AppMode::Home), |mut log: ResMut<VoyageLog>| log.open = false)
            .add_systems(Update, show.run_if(state_changed::<AppMode>.or_eager(resource_changed::<VoyageLog>)))
            .add_systems(Update, (open, input).chain().after(UiInputSet).before(UiStyleSet).run_if(in_state(AppMode::Home)));
    }
}

/// Is the LOG open (home's input waits then)?
pub fn is_open(log: Res<VoyageLog>) -> bool {
    log.open
}

/// The mast (L): open, with the rows and counts as they are now; the new results are read from then on (saved).
#[allow(clippy::too_many_arguments)]
fn open(
    mut commands: Commands,
    mut actions: MessageReader<HomeAction>,
    mut log: ResMut<VoyageLog>,
    mut career: ResMut<PlayerCareer>,
    mut storage: ResMut<Storage>,
    content: Res<GameContent>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    list: Single<(Entity, &mut ScrollPosition), With<LogList>>,
    mut counts: Query<(&LogText, &mut Text)>,
) {
    if !actions.read().any(|a| *a == HomeAction::Log) {
        return;
    }
    log.decoded.clear();
    let (list, mut scroll) = list.into_inner();
    *scroll = ScrollPosition::default();
    refill(&mut commands, list, &fonts, &style, &content.0, career.career(), &log.decoded);
    let l = career.career().log();
    for (t, mut text) in &mut counts {
        let n = match t {
            LogText::Voyages => l.voyages,
            LogText::Home => l.recalled,
            LogText::Lost => l.lost,
            LogText::Salvaged => l.salvaged,
        };
        kit::ui_kit::set_text(&mut text, &n.to_string());
    }
    if career.career().unread() > 0 {
        let (_, written) = career.update(&mut storage, |c| c.read_log());
        if let Err(e) = written {
            warn!("log: not saved: {e}");
            career.notes.push(Note::new("NOT SAVED", e, Tone::Coral));
        }
    }
    log.open = true;
}

/// The list's rows anew: the logs to decode, then the voyages.
fn refill(commands: &mut Commands, list: Entity, fonts: &HudFonts, style: &Style, content: &Content, career: &Career, decoded: &[TrailSignal]) {
    let rows = rows(content, career.history());
    let cores = cores(content, career, decoded);
    let memories = memories(content, career);
    commands.entity(list).despawn_children();
    commands.entity(list).with_children(|p| {
        fill_cores(p, fonts, style, &cores);
        fill_memories(p, fonts, style, &memories);
        fill(p, fonts, style, &rows);
    });
}

/// CLOSE, the ring and Esc; DECODE turns a log into a signal (saved) and the list shows it.
#[allow(clippy::too_many_arguments)]
fn input(
    mut commands: Commands,
    mut clicks: MessageReader<UiClick>,
    buttons: Query<(), With<CloseButton>>,
    decode: Query<&DecodeButton>,
    keys: Res<ButtonInput<KeyCode>>,
    mut log: ResMut<VoyageLog>,
    mut career: ResMut<PlayerCareer>,
    mut storage: ResMut<Storage>,
    content: Res<GameContent>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    list: Single<Entity, With<LogList>>,
) {
    let mut closed = false;
    let mut decoded = None;
    for c in clicks.read() {
        closed |= buttons.contains(c.0);
        if let Ok(d) = decode.get(c.0) {
            decoded = Some(d.0);
        }
    }
    if !log.open {
        return;
    }
    if let Some(k) = decoded {
        let (s, written) = career.update(&mut storage, |c| c.decode(k));
        if let Err(e) = written {
            warn!("log: not saved: {e}");
            career.notes.push(Note::new("NOT SAVED", e, Tone::Coral));
        }
        if let Some(s) = s {
            info!("log: decoded into signal {}.{}", s.trail, s.signal);
            log.decoded.push(s);
            refill(&mut commands, *list, &fonts, &style, &content.0, career.career(), &log.decoded);
        }
    }
    if closed || keys.just_pressed(KeyCode::Escape) {
        log.open = false;
    }
}

/// Shown at home while open, with its transition; home's words step aside meanwhile (`home::show`).
fn show(mode: Res<State<AppMode>>, log: Res<VoyageLog>, mut root: Single<&mut ScreenFx, With<LogRoot>>) {
    show_screen(&mut root, *mode.get() == AppMode::Home && log.open);
}

/// The logs to decode (and those just decoded) at the head of the list.
fn fill_cores(p: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, cores: &[CoreRow]) {
    if cores.is_empty() {
        return;
    }
    let ui = style.ui;
    let c = |tone: Tone| ui_color(tone.color(&ui), 1.0);
    let waiting = cores.iter().filter(|r| r.signal.is_none()).count();
    let head = if waiting == 0 { "LOGS DECODED".to_string() } else { format!("LOGS TO DECODE  ·  {waiting}  ·  DATA CORES FROM YOUR VOYAGES, FREE TO READ") };
    p.spawn((label(fonts, &head, 9.5, c(Tone::Gold)), Node { margin: UiRect::bottom(px(8)), flex_shrink: 0.0, ..default() }));
    let cell = |width: f32| Node { width: px(width), flex_shrink: 0.0, ..default() };
    for (k, r) in cores.iter().enumerate() {
        p.spawn((
            Node { min_height: px(ROW + 6.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(16), border: UiRect::bottom(px(1)), ..default() },
            BorderColor::all(ui_color(ui.dim, 0.25)),
        ))
        .with_children(|row| {
            match &r.signal {
                None => {
                    row.spawn((label(fonts, "LOG", 8.5, c(Tone::Gold)), cell(34.0)));
                    row.spawn((prose(fonts, &r.name, 15.0, c(Tone::Number)), cell(210.0)));
                    row.spawn((prose(fonts, &r.summary, 12.0, c(Tone::Label)), Node { flex_grow: 1.0, flex_shrink: 1.0, ..default() }));
                    spawn_pill(row, fonts, "DECODE", 9.5, 30.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, (DecodeButton(k), Tip::new("DECODE")));
                }
                Some((hint, line)) => {
                    row.spawn((label(fonts, "NEW", 8.5, c(Tone::Gold)), cell(34.0)));
                    row.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(3), width: px(210), flex_shrink: 0.0, ..default() }).with_children(|t| {
                        t.spawn(label(fonts, &format!("SIGNAL  ·  {hint}"), 9.5, c(Tone::Gold)));
                        t.spawn(prose(fonts, line, 12.5, c(Tone::Number)));
                    });
                    row.spawn((prose(fonts, &format!("Decoded from {}.", r.name.to_lowercase()), 12.0, c(Tone::Dim)), Node { flex_grow: 1.0, flex_shrink: 1.0, ..default() }));
                    row.spawn(label(fonts, "ON THE MAP", 9.0, c(Tone::Teal)));
                }
            }
        });
    }
    p.spawn(Node { height: px(22), flex_shrink: 0.0, ..default() });
}

/// The memory log's fragments, after the logs.
fn fill_memories(p: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, memories: &[(String, String)]) {
    if memories.is_empty() {
        return;
    }
    let ui = style.ui;
    let c = |tone: Tone| ui_color(tone.color(&ui), 1.0);
    let head = format!("MEMORY  ·  {}  ·  WHAT THE TRAILS LED BACK TO", memories.len());
    p.spawn((label(fonts, &head, 9.5, c(Tone::Gold)), Node { margin: UiRect::bottom(px(8)), flex_shrink: 0.0, ..default() }));
    for (name, text) in memories {
        p.spawn((
            Node { flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(16), padding: UiRect::vertical(px(10)), border: UiRect::bottom(px(1)), ..default() },
            BorderColor::all(ui_color(ui.dim, 0.25)),
        ))
        .with_children(|row| {
            row.spawn((label(fonts, "LORE", 8.5, c(Tone::Gold)), Node { width: px(34), flex_shrink: 0.0, ..default() }));
            row.spawn((prose(fonts, name, 15.0, c(Tone::Number)), Node { width: px(210), flex_shrink: 0.0, ..default() }));
            row.spawn((prose(fonts, text, 12.5, c(Tone::Label)), Node { flex_grow: 1.0, flex_shrink: 1.0, ..default() }));
        });
    }
    p.spawn(Node { height: px(22), flex_shrink: 0.0, ..default() });
}

/// The rows (or the empty state) into the list.
fn fill(p: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, rows: &[LogRow]) {
    let ui = style.ui;
    let c = |tone: Tone| ui_color(tone.color(&ui), 1.0);
    if rows.is_empty() {
        p.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(10), margin: UiRect::top(px(170)), ..default() }).with_children(|e| {
            e.spawn(label(fonts, "NO VOYAGES YET", 11.0, c(Tone::Label)));
            e.spawn(prose(fonts, "Every voyage through the gate is written here: where it went, how it ended and what came home.", 13.0, c(Tone::Dim)));
        });
        return;
    }
    let head = if rows.len() == 1 { "THE LAST VOYAGE".to_string() } else { format!("THE LAST {} VOYAGES  ·  NEWEST FIRST", rows.len()) };
    p.spawn((label(fonts, &head, 9.5, c(Tone::Dim)), Node { margin: UiRect::bottom(px(8)), flex_shrink: 0.0, ..default() }));
    let cell = |width: f32| Node { width: px(width), flex_shrink: 0.0, ..default() };
    for r in rows {
        p.spawn((
            // At least ROW high: a long haul on a phone takes a few lines and the row grows with it.
            Node { min_height: px(ROW), padding: UiRect::vertical(px(6)), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(16), border: UiRect::bottom(px(1)), ..default() },
            BorderColor::all(ui_color(ui.dim, 0.25)),
        ))
        .with_children(|row| {
            row.spawn((label(fonts, if r.new { "NEW" } else { "" }, 8.5, c(Tone::Gold)), cell(34.0)));
            row.spawn((prose(fonts, &r.name, 15.0, c(Tone::Number)), cell(210.0)));
            let (words, tone) = outcome_words(r.outcome);
            row.spawn((label(fonts, words, 9.5, c(tone)), cell(112.0)));
            row.spawn((number(fonts, &r.time, 14.0, c(Tone::Number)), cell(56.0)));
            let haul_tone = if r.parts.is_none() { Tone::Coral } else { Tone::Label };
            // Tracked less than a label: the longest hauls stay on one line.
            row.spawn((tracked(fonts, &r.haul(), 9.0, c(haul_tone), 0.18), Node { flex_grow: 1.0, flex_shrink: 1.0, ..default() }));
        });
    }
}

fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let abs = |at: Vec2, size: Option<Vec2>| Node {
        position_type: PositionType::Absolute,
        left: px(at.x),
        top: px(at.y),
        width: size.map_or(Val::Auto, |s| px(s.x)),
        height: size.map_or(Val::Auto, |s| px(s.y)),
        ..default()
    };
    commands
        .spawn((
            LogRoot,
            CompactText(COMPACT_TEXT),
            world::backdrop::VeilsBackdrop,
            ScreenFx::closed().dimming(),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn(Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|centre| {
                let stage_node = |size: Vec2| Node { width: px(size.x), height: px(size.y), flex_shrink: 0.0, ..default() };
                centre.spawn(by_class(stage_node(STAGE.0), stage_node(STAGE.1))).with_children(|stage| {
                    // The voyages, scrolling.
                    let list = |(at, size): (Vec2, Vec2)| Node { flex_direction: FlexDirection::Column, overflow: Overflow::scroll_y(), padding: UiRect::right(px(14)), ..abs(at, Some(size)) };
                    stage.spawn((LogList, ScreenPart::new(Edge::Fade, 1), Scrollbar::vertical(), ScrollPosition::default(), by_class(list(LIST[0]), list(LIST[1]))));
                });
            });
            // Top left: the way back, where we are, the title; top right: the career's counts, CLOSE (the frame's corners).
            let f = frame();
            root.spawn((ScreenPart::new(Edge::Top, 0), f.top_left())).with_children(|tl| {
                spawn_head(tl, fonts, &ui, (CloseButton, Tip::key("SHIPYARD", "ESC")), "SHIPYARD  ›  LOG", "Log");
                crate::evolution::spawn_chip(tl, fonts);
            });
            root.spawn((ScreenPart::new(Edge::Top, 0), f.top_right())).with_children(|tr| {
                for (word, t, tone) in [("VOYAGES", LogText::Voyages, Tone::Number), ("HOME", LogText::Home, Tone::Teal), ("LOST", LogText::Lost, Tone::Coral), ("SALVAGED", LogText::Salvaged, Tone::Gold)] {
                    tr.spawn(label(fonts, word, 9.0, c(Tone::Label, 1.0)));
                    tr.spawn((t, number(fonts, "0", 17.0, c(tone, 1.0)), Node { margin: UiRect::right(px(8)), ..default() }));
                }
                spawn_close(tr, fonts, CloseButton);
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::career::{Career, Haul, HaulItem, VoyageEnd};

    /// A cleared voyage and a lost one: newest first, each in words; the mast's line for the last one; nothing
    /// new once read.
    #[test]
    fn the_history_reads_newest_first() {
        let content = data::base_content();
        let mut c = Career::new(&content, "rim_awakening", 4207).unwrap();
        assert!(rows(&content, c.history()).is_empty());
        c.voyage_launched(&content, "yard_approach").unwrap();
        let haul: Haul = [(HaulItem::Broken { part: "automaton_cannon".into(), material: None }, 6), (HaulItem::Fragment { part: "automaton_core".into() }, 1)].into_iter().collect();
        c.voyage_ended(&content, "yard_approach", VoyageEnd::Recalled { haul, cleared: true, ticks: 30 * 462 }).unwrap();
        c.voyage_launched(&content, "yard_approach").unwrap();
        c.voyage_ended(&content, "yard_approach", VoyageEnd::Lost { ticks: 30 * 61 }).unwrap();

        let r = rows(&content, c.history());
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].outcome, r[0].parts, r[0].new), (Outcome::Lost, None, true));
        assert_eq!(r[0].line(), "LAST VOYAGE  ·  SHIPYARD APPROACH  ·  LOST  ·  1:01  ·  THE HAUL STAYED BEHIND");
        let cleared = &r[1];
        assert_eq!((cleared.name.as_str(), cleared.time.as_str(), cleared.parts), ("Shipyard approach", "7:42", Some(7)));
        let haul = cleared.haul();
        assert!(haul.starts_with("7 PARTS  »  KINETIC WEAPONS +") && haul.contains("PROGRAMMABLE MATTER +1"), "{haul}");
        assert_eq!(cleared.line(), format!("LAST VOYAGE  ·  SHIPYARD APPROACH  ·  GOAL CLEARED  ·  7:42  ·  {haul}"));

        assert!(c.read_log());
        assert!(rows(&content, c.history()).iter().all(|r| !r.new));
        // Home with nothing in the holds.
        let empty = LogRow { parts: Some(0), gains: Vec::new(), ..r[1].clone() };
        assert_eq!(empty.haul(), "0 PARTS");
    }
}
