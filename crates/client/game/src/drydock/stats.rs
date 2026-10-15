//! The stats grid and the status line, from the design report ([`DrydockReport`], `sim::report`): every number
//! the right panel shows and the line next to TEST-FLY, rewritten whenever the report is (once per design
//! revision, or when the tuning changes). The mapping is pure ([`stat_cells`], [`status_line`]) so it is tested
//! without Bevy; [`update`] only writes the resources. [`voyage`] adds the career's voyage-ready line after the
//! status line (`career::voyage_line`).
//!
//! * **Labels, units and precision** come from content (`stats.ron`: `data::design::stats::SHIP_STATS` and the part
//!   stats for mass and build points). Values are shortened to three significant digits (never more decimals
//!   than the content's) so they fit the mockup's narrow cells; the main grid drops units where they would
//!   not fit (the power and control pairs, thrust over mass).
//! * **Words** (yaw, device load, the status line) are UI chrome in code, like the flight HUD's.
//! * **Tones**: teal = fine, gold = works but worse (runs hot, a pull the gyros hold, a warning), coral = broken
//!   or over a limit (power deficit, over the control capacity, overheating past [`RUNS_HOT_MAX`]). A bar over
//!   100 % fills its track and a notch marks 100 % (`ui_kit::ValueBar`).
//!
//! The grid holds [`MAIN_CELLS`] cells in the mockup's two columns (POWER, PEAK HEAT, THRUST/MASS, YAW,
//! FIREPOWER, CARGO, CONTROL, DEVICE LOAD), then [`DETAIL_CELLS`] smaller ones under them (TOP SPEED, TURN 90°,
//! MASS, BUILD POINTS).

use bevy::prelude::*;
use data::Content;
use data::design::stats::{format_number, with_unit};
use sim::DesignReport;
use sim::report::{Issue, LoadClass};
use kit::ui_kit::Tone;
use world::career::{EscortDesigns, Granted, voyage_line};
use world::player::PlayerDesign;
use world::player::design::flight_ready;
use world::sim_bridge::GameContent;

use super::report::DrydockReport;
use super::state::{DrydockStats, DrydockStatus, StatCell};

/// Cells of the main grid (two columns, with bars), then the compact block under it.
pub const MAIN_CELLS: usize = 8;
pub const DETAIL_CELLS: usize = 4;

/// GDR-0052: a part up to this share of its heat limit at full use "runs hot" (gold); above it, it overheats
/// (coral). PLACEHOLDER (Niklas: "about 120 %"). The status line, the PEAK HEAT cell and the heat overlay use it.
pub const RUNS_HOT_MAX: f32 = 1.2;

/// The yaw reads "Balanced" while full forward thrust passes the centre of mass closer than this (m): its
/// torque over its force. PLACEHOLDER (a twentieth of a cell).
pub const BALANCED_ARM: f32 = 0.05;

/// The separator of the status line (the info line under the grid uses the same).
const SEP: &str = "  ·  ";

/// The tone of a heat share (`heat / limit`): teal up to the limit, gold while it only runs hot, coral above.
pub fn heat_tone(ratio: f32) -> Tone {
    if ratio <= 1.0 {
        Tone::Teal
    } else if ratio <= RUNS_HOT_MAX {
        Tone::Gold
    } else {
        Tone::Coral
    }
}

/// How the forward thrust turns the ship: `None` without forward thrust, else `(pull, held)` with `pull` −1
/// right, 0 balanced, +1 left (the report's yaw is counter-clockwise positive) and `held` = the gyros and
/// gimbals hold it while flying straight.
pub fn yaw_pull(r: &DesignReport) -> Option<(i8, bool)> {
    let f = &r.flight;
    if f.thrust[0] <= 0.0 {
        return None;
    }
    let arm = f.yaw / f.thrust[0];
    let pull = if arm.abs() <= BALANCED_ARM { 0 } else if arm > 0.0 { 1 } else { -1 };
    Some((pull, f.yaw_balanced))
}

/// A value shortened to three significant digits, with at most `decimals` decimals.
fn short(v: f32, decimals: u8) -> String {
    let d = if v.abs() >= 100.0 {
        0
    } else if v.abs() >= 10.0 {
        decimals.min(1)
    } else {
        decimals
    };
    format_number(v, d)
}

/// A content stat's label, decimals and unit (the id itself if `stats.ron` lacks it; validation reports that).
fn def(content: &Content, stat: &str) -> (String, u8, String) {
    match content.stats.get(stat) {
        Some(d) => (d.name.to_uppercase(), d.decimals, d.unit.clone()),
        None => (stat.to_uppercase(), 1, String::new()),
    }
}

/// A stat's label from content, as the stats grid shows it.
pub fn stat_label(content: &Content, stat: &str) -> String {
    def(content, stat).0
}

/// A stat's value as the stats grid shows it: shortened, with its unit.
pub fn stat_value(content: &Content, stat: &str, v: f32) -> String {
    let (_, decimals, unit) = def(content, stat);
    with_unit(&short(v, decimals), &unit)
}

/// `share` for a bar of `used` against `cap`: over 1 when used exceeds it (the notch marks 100 %).
fn share(used: f32, cap: f32) -> f32 {
    if cap > 0.0 {
        used / cap
    } else if used > 0.0 {
        f32::INFINITY
    } else {
        0.0
    }
}

/// The stats grid from a report; `None` (the design doesn't compile) shows labels and "—".
pub fn stat_cells(content: &Content, report: Option<&DesignReport>) -> Vec<StatCell> {
    let cell = |stat: &str, value: String, tone: Tone, bar: Option<(f32, Tone)>| StatCell { label: stat_label(content, stat), value, tone, bar };
    // The value of a stat: shortened, with or without its unit.
    let val = |stat: &str, v: f32, unit: bool| if unit { stat_value(content, stat, v) } else { short(v, def(content, stat).1) };
    let pair = |stat: &str, a: f32, b: f32| format!("{} / {}", val(stat, a, false), val(stat, b, false));
    let dash = || "—".to_string();

    let Some(r) = report else {
        let bars = [Some(Tone::Gold), Some(Tone::Teal), None, None, None, None, Some(Tone::Violet), None, None, None, None, None];
        return ["power_use", "peak_heat", "acceleration", "yaw", "firepower", "cargo_capacity", "control_load", "device_load", "top_speed", "turn_time", "mass", "build_points"]
            .iter()
            .zip(bars)
            .map(|(stat, bar)| cell(stat, dash(), Tone::Dim, bar.map(|t| (0.0, t))))
            .collect();
    };

    let p = &r.power;
    let short_power = p.deficit > 0.0;
    let power = cell(
        "power_use",
        pair("power_use", p.demand, p.supply),
        if short_power { Tone::Coral } else { Tone::Teal },
        Some((share(p.demand, p.supply), if short_power { Tone::Coral } else { Tone::Gold })),
    );
    let peak = r.heat.peak;
    let heat = cell("peak_heat", val("peak_heat", peak * 100.0, true), heat_tone(peak), Some((peak, heat_tone(peak))));
    let accel = r.flight.accel[0];
    let thrust = cell("acceleration", val("acceleration", accel, false), if accel > 0.0 { Tone::Teal } else { Tone::Coral }, None);
    let (yaw_word, yaw_tone) = match yaw_pull(r) {
        None => (dash(), Tone::Dim),
        Some((0, _)) => ("Balanced".into(), Tone::Teal),
        Some((pull, held)) => {
            let word = if pull > 0 { "Pulls left" } else { "Pulls right" };
            (word.into(), if held { Tone::Gold } else { Tone::Coral })
        }
    };
    let yaw = cell("yaw", yaw_word, yaw_tone, None);
    let fire = cell("firepower", val("firepower", r.firepower, true), Tone::Number, None);
    let cargo = cell("cargo_capacity", val("cargo_capacity", r.cargo, true), Tone::Number, None);
    let c = &r.control;
    let over = c.load > c.capacity;
    let control = cell(
        "control_load",
        pair("control_load", c.load as f32, c.capacity as f32),
        if over { Tone::Coral } else { Tone::Violet },
        Some((share(c.load as f32, c.capacity as f32), if over { Tone::Coral } else { Tone::Violet })),
    );
    let (word, tone) = match r.device.class {
        LoadClass::Light => ("Light", Tone::Teal),
        LoadClass::Moderate => ("Moderate", Tone::Teal),
        LoadClass::Heavy => ("Heavy", Tone::Gold),
        LoadClass::TooHeavy => ("Too heavy", Tone::Coral),
    };
    let device = cell("device_load", word.into(), tone, None);

    let f = &r.flight;
    let moves = f.thrust.iter().any(|&t| t > 0.0);
    let top = cell("top_speed", if moves { val("top_speed", f.top_speed[f.best], true) } else { dash() }, Tone::Number, None);
    let turn = match f.turn_90 {
        Some(t) => cell("turn_time", val("turn_time", t, true), Tone::Number, None),
        None => cell("turn_time", dash(), Tone::Coral, None),
    };
    let mass = cell("mass", val("mass", r.mass, true), Tone::Number, None);
    let bp = cell("build_points", val("build_points", r.build_points, true), Tone::Number, None);
    vec![power, heat, thrust, yaw, fire, cargo, control, device, top, turn, mass, bp]
}

/// The status line: whether the design can fly and why not, else its most important warning.
#[derive(Clone, Debug, PartialEq)]
pub struct StatusLine {
    /// Why it can't fly (lowercase; TEST-FLY is disabled), `None` = flight-ready.
    pub grounded: Option<String>,
    /// The line (uppercase).
    pub line: String,
    pub tone: Tone,
}

/// One warning of a flight-ready design: its rank (lower = more important), tone and words.
fn warnings(content: &Content, r: &DesignReport) -> Vec<(u8, Tone, String)> {
    let plural = |n: usize, one: &str, many: &str| if n == 1 { one.to_string() } else { format!("{n} {many}") };
    let mut out = Vec::new();
    let plumes = r.issues.iter().filter(|i| matches!(i, Issue::PlumeBurnsOwnPart { .. })).count();
    if plumes > 0 {
        out.push((1, Tone::Coral, plural(plumes, "A PLUME BURNS THE SHIP", "PLUMES BURN THE SHIP")));
    }
    let guns = r.issues.iter().filter(|i| matches!(i, Issue::GunBlocked { .. })).count();
    if guns > 0 {
        out.push((5, Tone::Gold, plural(guns, "GUN BLOCKED", "GUNS BLOCKED")));
    }
    for i in &r.issues {
        match i {
            Issue::FallsOff { parts } => out.push((0, Tone::Coral, plural(parts.len(), "A PART FALLS OFF", "PARTS FALL OFF"))),
            Issue::Overheat { .. } if r.heat.peak > RUNS_HOT_MAX => out.push((2, Tone::Coral, "OVERHEATS AT FULL LOAD".into())),
            Issue::Overheat { .. } => out.push((7, Tone::Gold, "RUNS HOT AT FULL LOAD".into())),
            Issue::PowerDeficit { deficit, .. } => {
                let (_, decimals, unit) = def(content, "power_use");
                out.push((3, Tone::Gold, format!("POWER SHORT BY {}", with_unit(&short(*deficit, decimals), &unit).to_uppercase())));
            }
            Issue::ControlOverCapacity { .. } => out.push((4, Tone::Gold, "OVER CONTROL CAPACITY".into())),
            _ => {}
        }
    }
    if let Some((pull, false)) = yaw_pull(r)
        && pull != 0
    {
        out.push((6, Tone::Gold, format!("PULLS {} UNDER THRUST", if pull > 0 { "LEFT" } else { "RIGHT" })));
    }
    out.sort_by_key(|w| w.0);
    out
}

/// What the status line says about a report (`Err`: the design doesn't compile).
pub fn status_line(content: &Content, report: &Result<DesignReport, String>) -> StatusLine {
    let r = match flight_ready(report) {
        Err(why) => return StatusLine { line: format!("CAN'T FLY{SEP}{}", why.to_uppercase()), grounded: Some(why), tone: Tone::Coral },
        Ok(r) => r,
    };
    let w = warnings(content, r);
    let Some((_, tone, first)) = w.first() else {
        return StatusLine { grounded: None, line: "FLIGHT-READY".into(), tone: Tone::Teal };
    };
    let mut line = format!("FLIGHT-READY{SEP}{first}");
    if w.len() > 1 {
        line.push_str(&format!("{SEP}+{}", w.len() - 1));
    }
    StatusLine { grounded: None, line, tone: *tone }
}

/// Rewrites the stats grid and the status line whenever the report is recomputed.
pub(super) fn update(report: Res<DrydockReport>, content: Res<GameContent>, mut stats: ResMut<DrydockStats>, mut status: ResMut<DrydockStatus>) {
    if !report.is_changed() {
        return;
    }
    let Some(result) = &report.report else { return };
    let content = &content.0;
    let cells = stat_cells(content, report.get());
    if stats.cells != cells {
        stats.cells = cells;
    }
    let s = status_line(content, result);
    status.revision = report.revision;
    status.grounded = s.grounded;
    status.line = s.line;
    status.tone = s.tone;
}

/// The voyage-ready line after the status line: the career's readiness of the fleet with this design as the
/// flagship and FLEET's escorts (`career::voyage_line`). A design that can't fly is not voyage-ready either.
pub(super) fn voyage(
    design: Res<PlayerDesign>,
    granted: Res<Granted>,
    escorts: Res<EscortDesigns>,
    content: Res<GameContent>,
    mut status: ResMut<DrydockStatus>,
) {
    if !(design.is_changed() || granted.is_changed() || escorts.is_changed() || status.is_changed()) {
        return;
    }
    let (line, tone) = match status.grounded {
        Some(_) => ("NOT VOYAGE-READY".to_string(), Tone::Coral),
        None => voyage_line(&content.0, &granted, design.blueprint(), &escorts.0),
    };
    if status.voyage != line || status.voyage_tone != tone {
        status.voyage = line;
        status.voyage_tone = tone;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use world::player::PlayerDesign;

    fn report_of(content: &Content, design: &PlayerDesign) -> Result<DesignReport, String> {
        sim::design_report(content, design.blueprint(), &sim::SimConfig::default())
    }

    fn runner() -> (Content, DesignReport) {
        let content = data::base_content();
        let r = report_of(&content, &PlayerDesign::starter(&content, "halvex_runner")).unwrap();
        (content, r)
    }

    fn value<'a>(cells: &'a [StatCell], label: &str) -> &'a StatCell {
        cells.iter().find(|c| c.label == label).unwrap_or_else(|| panic!("no cell {label}"))
    }

    #[test]
    fn the_runner_fills_every_cell_with_content_labels() {
        let (content, r) = runner();
        let cells = stat_cells(&content, Some(&r));
        assert_eq!(cells.len(), MAIN_CELLS + DETAIL_CELLS);
        let labels: Vec<&str> = cells.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(
            labels,
            ["POWER", "PEAK HEAT", "THRUST/MASS", "YAW", "FIREPOWER", "CARGO", "CONTROL", "DEVICE LOAD", "TOP SPEED", "TURN 90°", "MASS", "BUILD POINTS"]
        );
        // docs/sim.md "Design report (drydock)": 54 / 50 MW, 109 %, 10.3 m/s², 8.3 dps, 18 / 32, Light, 22 m/s, 1.5 s
        // (90° turn since GDR-0055)
        // (shown with the content's one decimal).
        let power = value(&cells, "POWER");
        assert_eq!(power.value, "50 / 54");
        assert_eq!((power.tone, power.bar.map(|b| b.1)), (Tone::Teal, Some(Tone::Gold)));
        assert!((power.bar.unwrap().0 - 50.0 / 54.0).abs() < 1e-4);
        let heat = value(&cells, "PEAK HEAT");
        assert_eq!((heat.value.as_str(), heat.tone), ("109%", Tone::Gold));
        assert!(heat.bar.unwrap().0 > 1.0);
        assert_eq!(value(&cells, "THRUST/MASS").value, "10.3");
        assert_eq!((value(&cells, "YAW").value.as_str(), value(&cells, "YAW").tone), ("Balanced", Tone::Teal));
        assert_eq!(value(&cells, "FIREPOWER").value, "8.3/s");
        let control = value(&cells, "CONTROL");
        assert_eq!((control.value.as_str(), control.tone), ("18 / 32", Tone::Violet));
        assert_eq!(value(&cells, "DEVICE LOAD").value, "Light");
        assert_eq!(value(&cells, "TOP SPEED").value, "22 m/s");
        assert_eq!(value(&cells, "TURN 90°").value, "1.5 s");
        assert!(value(&cells, "MASS").value.ends_with(" t"));
        assert!(value(&cells, "BUILD POINTS").value.ends_with(" BP"));
    }

    #[test]
    fn no_report_shows_dashes() {
        let content = data::base_content();
        let cells = stat_cells(&content, None);
        assert_eq!(cells.len(), MAIN_CELLS + DETAIL_CELLS);
        assert!(cells.iter().all(|c| c.value == "—" && c.tone == Tone::Dim));
        assert_eq!(cells[0].bar, Some((0.0, Tone::Gold)));
    }

    #[test]
    fn deficits_and_overloads_turn_coral_and_overflow_the_bar() {
        let (content, mut r) = runner();
        r.power.demand = 70.0;
        r.power.deficit = 16.0;
        r.control.load = 40;
        r.heat.peak = 1.5;
        let cells = stat_cells(&content, Some(&r));
        let power = value(&cells, "POWER");
        assert_eq!((power.value.as_str(), power.tone), ("70 / 54", Tone::Coral));
        let (share, tone) = power.bar.unwrap();
        assert!(share > 1.0 && tone == Tone::Coral);
        let control = value(&cells, "CONTROL");
        assert_eq!((control.value.as_str(), control.tone), ("40 / 32", Tone::Coral));
        assert_eq!(value(&cells, "PEAK HEAT").tone, Tone::Coral);
        r.power.supply = 0.0;
        assert_eq!(stat_cells(&content, Some(&r))[0].bar.unwrap().0, f32::INFINITY);
    }

    #[test]
    fn yaw_words_follow_the_pull_and_the_hold() {
        let (content, mut r) = runner();
        let f = r.flight.thrust[0];
        // Counter-clockwise torque turns the nose left.
        r.flight.yaw = f * 0.5;
        r.flight.yaw_balanced = true;
        let yaw = |r: &DesignReport| {
            let c = stat_cells(&content, Some(r));
            (c[3].value.clone(), c[3].tone)
        };
        assert_eq!(yaw(&r), ("Pulls left".into(), Tone::Gold));
        r.flight.yaw = -f * 0.5;
        r.flight.yaw_balanced = false;
        assert_eq!(yaw(&r), ("Pulls right".into(), Tone::Coral));
        // An unheld pull outranks running hot.
        assert_eq!(status_line(&content, &Ok(r.clone())).line, "FLIGHT-READY  ·  PULLS RIGHT UNDER THRUST  ·  +1");
        r.flight.yaw = f * BALANCED_ARM * 0.5;
        assert_eq!(yaw(&r).0, "Balanced");
        r.flight.thrust[0] = 0.0;
        assert_eq!(yaw(&r), ("—".into(), Tone::Dim));
    }

    #[test]
    fn heat_tones_follow_gdr_0052() {
        assert_eq!(heat_tone(0.0), Tone::Teal);
        assert_eq!(heat_tone(1.0), Tone::Teal);
        assert_eq!(heat_tone(1.01), Tone::Gold);
        assert_eq!(heat_tone(RUNS_HOT_MAX), Tone::Gold);
        assert_eq!(heat_tone(RUNS_HOT_MAX + 0.01), Tone::Coral);
    }

    #[test]
    fn the_runner_is_flight_ready_and_runs_hot() {
        let (content, r) = runner();
        let s = status_line(&content, &Ok(r));
        assert_eq!(s, StatusLine { grounded: None, line: "FLIGHT-READY  ·  RUNS HOT AT FULL LOAD".into(), tone: Tone::Gold });
    }

    #[test]
    fn warnings_pick_the_most_important_and_count_the_rest() {
        let (content, mut r) = runner();
        let line = |r: &DesignReport| {
            let s = status_line(&content, &Ok(r.clone()));
            assert_eq!(s.grounded, None);
            (s.line, s.tone)
        };
        r.issues = vec![Issue::GunBlocked { gun: 0, blockers: vec![1] }, Issue::GunBlocked { gun: 2, blockers: vec![1] }];
        assert_eq!(line(&r), ("FLIGHT-READY  ·  2 GUNS BLOCKED".into(), Tone::Gold));
        r.issues.push(Issue::PowerDeficit { deficit: 4.0, short: vec![3] });
        assert_eq!(line(&r), ("FLIGHT-READY  ·  POWER SHORT BY 4 MW  ·  +1".into(), Tone::Gold));
        r.issues.push(Issue::PlumeBurnsOwnPart { engine: 5, parts: vec![6] });
        assert_eq!(line(&r), ("FLIGHT-READY  ·  A PLUME BURNS THE SHIP  ·  +2".into(), Tone::Coral));
        r.issues.push(Issue::FallsOff { parts: vec![7, 8] });
        assert_eq!(line(&r), ("FLIGHT-READY  ·  2 PARTS FALL OFF  ·  +3".into(), Tone::Coral));
        r.issues = vec![Issue::Overheat { parts: vec![1] }];
        r.heat.peak = 1.3;
        assert_eq!(line(&r), ("FLIGHT-READY  ·  OVERHEATS AT FULL LOAD".into(), Tone::Coral));
        r.issues.clear();
        assert_eq!(line(&r), ("FLIGHT-READY".into(), Tone::Teal));
    }

    #[test]
    fn blockers_ground_the_design() {
        let (content, mut r) = runner();
        r.issues = vec![Issue::NoLogicCore, Issue::NoThrust, Issue::GunBlocked { gun: 0, blockers: vec![1] }];
        let s = status_line(&content, &Ok(r.clone()));
        assert_eq!(s, StatusLine { grounded: Some("no logic core".into()), line: "CAN'T FLY  ·  NO LOGIC CORE".into(), tone: Tone::Coral });
        r.issues = vec![Issue::CoresUnpowered];
        assert_eq!(status_line(&content, &Ok(r.clone())).line, "CAN'T FLY  ·  LOGIC CORES UNPOWERED");
        r.issues = vec![Issue::NoThrust];
        assert_eq!(status_line(&content, &Ok(r)).grounded.as_deref(), Some("no thrust"));
        let s = status_line(&content, &Err("unknown part `x`".into()));
        assert_eq!((s.line.as_str(), s.grounded.as_deref()), ("CAN'T FLY  ·  UNKNOWN PART `X`", Some("unknown part `x`")));
    }

    #[test]
    fn real_designs_without_cores_or_parts_are_grounded() {
        let content = data::base_content();
        let mut d = PlayerDesign::starter(&content, "halvex_runner");
        d.edit(|dock| dock.mirror = false);
        let cores: Vec<_> = d
            .blueprint()
            .parts
            .iter()
            .filter(|p| content.parts.get(&p.part).is_some_and(|p| p.logic.is_some()))
            .map(|p| (p.x, p.y))
            .collect();
        for cell in cores {
            d.edit(|dock| dock.erase(cell)).unwrap();
        }
        let s = status_line(&content, &report_of(&content, &d));
        assert_eq!(s.grounded.as_deref(), Some("no logic core"));
        let empty = data::Blueprint { parts: Vec::new(), ..d.export() };
        d.replace(PlayerDesign::open(&content, &empty, d.frame()).unwrap());
        let s = status_line(&content, &report_of(&content, &d));
        assert_eq!((s.line.as_str(), s.tone), ("CAN'T FLY  ·  EMPTY FRAME", Tone::Coral));
    }
}
