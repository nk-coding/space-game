//! Scanning in flight (galaxy plan E3, GDR-0093): the SCAN pill under the target panel (shown for a flown ship with
//! a scanner; the G key or a click scans) with what the scanner does now and a bar for its charge and cooldown; in the
//! world, in the vector look: an arc filling round each of the player's ships while its scan charges, the sweep as a
//! ring running out to the scanner's range (or the screen's corners, zoomed in), its edge fading after, a ping at every site it (or the sensors) found, the bearings as dashed
//! gold lines from where each scan was taken, and a dotted ring at every found site on screen.

use super::*;
use super::markers::{hud_fade, site_look};
use sim::{Scan, SimEvent};

/// The SCAN block (hidden for a ship without a scanner).
#[derive(Component)]
pub(super) struct ScanBlock;

/// The SCAN pill.
#[derive(Component)]
pub(super) struct ScanButton;

/// The line under the pill.
#[derive(Component)]
pub(super) struct ScanLine;

/// The bar's fill: the charge, or what is left of the cooldown.
#[derive(Component)]
pub(super) struct ScanBar;

/// The bar's width (logical px).
const BAR: f32 = 64.0;
/// A sweep's ring runs out to the scanner's range in this many seconds, then its edge fades over [`EDGE_FADE`].
const SWEEP: f64 = 1.1;
const EDGE_FADE: f64 = 2.5;
/// A found site's ping: a ring growing to this many px in this many seconds.
const PING_PX: f32 = 70.0;
const PING: f64 = 0.9;
/// A bearing's line reaches this far past the cluster's edge from where it was taken (m), dashed in px.
const BEARING_LENGTH: f32 = 6000.0;
const DASH_PX: f32 = 14.0;
const GAP_PX: f32 = 9.0;

/// What the scans drew lately (sim seconds).
#[derive(Resource, Default)]
pub(super) struct ScanFx {
    /// Sweeps: where, how far, when.
    sweeps: Vec<(Vec2, f32, f64)>,
    /// Sites found: where, when.
    pings: Vec<(usize, f64)>,
}

/// The block under the target panel.
pub(super) fn spawn(c: &mut ChildSpawnerCommands, fonts: &HudFonts, ui: &kit::style::UiPalette) {
    c.spawn((
        ScanBlock,
        HudZone,
        Visibility::Hidden,
        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: px(5), margin: UiRect::top(px(16)), ..default() },
    ))
    .with_children(|b| {
        b.spawn(Node::default()).with_children(|r| {
            spawn_pill(r, fonts, "SCAN", 12.0, 34.0, None, Pill::new(Tone::Violet), (ScanButton, HudButton, HudZone, Tip::new("SCAN"), ActionTip(Action::Scan)));
        });
        b.spawn((Node { width: px(BAR), height: px(3), ..default() }, BackgroundColor(ui_color(ui.dim, 0.5)))).with_children(|bar| {
            bar.spawn((ScanBar, Node { width: px(0), height: percent(100), ..default() }, BackgroundColor(ui_color(ui.violet, 0.95))));
        });
        b.spawn((ScanLine, label(fonts, "", 9.5, ui_color(ui.violet, 1.0))));
    });
}

/// What the scanner does now, in words, and the bar's share (0..1).
pub(super) fn scan_words(ship: &sim::Ship, key: &str) -> (String, f32) {
    let spec = |part: sim::PartIdx| ship.design().parts[part as usize].scanner.as_ref();
    match ship.scan() {
        Scan::Charging { part, charged } => {
            let share = spec(part).map_or(0.0, |s| (charged / s.charge).clamp(0.0, 1.0));
            (format!("CHARGING  ·  {:.0} %", share * 100.0), share)
        }
        Scan::Cooling { part, left } => {
            let share = spec(part).map_or(0.0, |s| (left / s.cooldown.max(1e-3)).clamp(0.0, 1.0));
            (format!("COOLING  ·  {:.0} S", left.ceil()), share)
        }
        Scan::Ready => match ship.scanner() {
            Some((_, s)) if ship.is_active() => (format!("READY  ·  {key}  ·  {:.1} KM", s.range / 1000.0), 1.0),
            _ => ("NO WORKING SCANNER".into(), 0.0),
        },
    }
}

/// The block every frame: shown while the flown ship has a scanner in its design; the pill enabled while a scan can
/// start, lit while one charges.
#[allow(clippy::type_complexity)]
pub(super) fn scan_widgets(
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    controls: Res<crate::settings::Controls>,
    mut block: Query<&mut Visibility, With<ScanBlock>>,
    mut pill: Query<&mut Pill, With<ScanButton>>,
    mut bar: Query<&mut Node, With<ScanBar>>,
    mut line: Query<&mut Text, With<ScanLine>>,
) {
    let ship = focus.0.and_then(|id| world.sim.ship(id)).filter(|s| !s.design().scanners.is_empty());
    for mut v in &mut block {
        let want = if ship.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
    let Some(ship) = ship else { return };
    let key = controls.0.inputs(Action::Scan).next().map_or_else(|| "—".to_string(), |c| c.label());
    let (words, share) = scan_words(ship, &key);
    for mut p in &mut pill {
        let (disabled, active) = (!ship.can_scan(), matches!(ship.scan(), Scan::Charging { .. }));
        if p.disabled != disabled || p.active != active {
            p.disabled = disabled;
            p.active = active;
        }
    }
    for mut n in &mut bar {
        let w = px((BAR * share).round());
        if n.width != w {
            n.width = w;
        }
    }
    for mut t in &mut line {
        set_text(&mut t, &words);
    }
}

/// The SCAN pill: like the key.
pub(super) fn scan_click(mut clicks: MessageReader<UiClick>, pill: Query<&Pill, With<ScanButton>>, focus: Res<PilotFocus>, mut world: ResMut<SimWorld>) {
    if clicks.read().any(|c| pill.get(c.0).is_ok_and(|p| !p.disabled))
        && let Some(me) = focus.0
    {
        world.inputs.commands.push((me, sim::ShipCommand::Scan));
    }
}

/// Every tick: the sweeps of the player's ships and the sites found, for drawing.
pub(super) fn note_scans(world: Res<SimWorld>, mut fx: ResMut<ScanFx>) {
    let now = world.sim.time_seconds();
    fx.sweeps.retain(|s| now - s.2 < SWEEP + EDGE_FADE);
    fx.pings.retain(|p| now - p.1 < PING);
    for e in world.sim.events() {
        match *e {
            SimEvent::Scanned { ship, at, range, .. } if world.sim.ship(ship).is_some_and(|s| s.owner() == crate::scene::PLAYER) => {
                fx.sweeps.push((v2(at), range, now));
            }
            SimEvent::SiteRevealed { site } => fx.pings.push((site as usize, now)),
            _ => {}
        }
    }
}

/// A new flight: nothing of the last one's scans.
pub(super) fn forget_scans(mut fx: ResMut<ScanFx>) {
    *fx = ScanFx::default();
}

/// A circle of `n` dots of diameter `d` (world units) round `c` at `radius`, the first `share` of it (from the top,
/// clockwise).
fn dotted(queue: &mut FxQueue, r: &kit::atlas::Region, (c, radius): (Vec2, f32), n: usize, share: f32, d: f32, color: Color) {
    let shown = ((n as f32) * share.clamp(0.0, 1.0)).round() as usize;
    for k in 0..shown {
        let a = std::f32::consts::FRAC_PI_2 - k as f32 / n as f32 * std::f32::consts::TAU;
        queue.dot(r, c + Vec2::new(a.cos(), a.sin()) * radius, d, color, Z_HUD);
    }
}

/// A ring of `n` segments round `c`.
fn ring(queue: &mut FxQueue, r: &kit::atlas::Region, c: Vec2, radius: f32, n: usize, width: f32, color: Color) {
    let at = |k: usize| {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        c + Vec2::new(a.cos(), a.sin()) * radius
    };
    for k in 0..n {
        queue.line(r, at(k), at(k + 1), width, color, Z_HUD);
    }
}

/// The scans in the world (see the module docs), faded with the HUD.
#[allow(clippy::too_many_arguments)]
pub(super) fn scan_world(
    world: Res<SimWorld>,
    rig: Res<CameraRig>,
    fixed: Res<Time<Fixed>>,
    regions: Option<Res<FxRegions>>,
    style: Res<Style>,
    fx: Res<ScanFx>,
    hud: Query<&ScreenFx, With<HudRoot>>,
    views: Query<&PieceView>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(regions) = regions else { return };
    let fade = hud_fade(&hud);
    if fade <= 0.0 {
        return;
    }
    let ui = style.ui;
    let alpha = fixed.overstep_fraction();
    let now = world.sim.time_seconds() + alpha as f64 / sim::TICK_HZ as f64;
    let px = 1.0 / rig.zoom;
    let (half, centre) = (rig.half_extent(), rig.pos);
    let on_screen = |p: Vec2, margin: f32| (p - centre).abs().cmple(half + Vec2::splat(margin)).all();

    // Charging: an arc filling round the ship.
    for s in world.sim.ships().iter().filter(|s| s.owner() == crate::scene::PLAYER) {
        let Scan::Charging { part, charged } = s.scan() else { continue };
        let Some(spec) = s.design().parts[part as usize].scanner.as_ref() else { continue };
        let Some((id, p)) = s.control_piece().and_then(|c| s.piece(c).map(|p| (c, p))) else { continue };
        let c = piece_com(p, alpha);
        let view = world.views.get(&(s.id(), id)).and_then(|e| views.get(*e).ok()).map_or(3.0, |v| v.radius);
        let r = (view + 3.0).max(26.0 * px);
        dotted(&mut queue, &regions.hard, (c, r), 48, 1.0, 2.0 * px, ui.violet.with_alpha(0.25 * fade));
        dotted(&mut queue, &regions.hard, (c, r), 48, charged / spec.charge, 3.5 * px, ui.violet.with_alpha(0.9 * fade));
    }

    // Sweeps: the ring running out, then its edge fading. Zoomed in, the ring runs out to the screen's corners
    // instead (the scanner's range lies far beyond them), so the sweep shows at every zoom.
    let corner = rig.viewport.length() * 0.55;
    for &(at, range, t) in &fx.sweeps {
        let age = now - t;
        if age < 0.0 {
            continue;
        }
        let run = (age / SWEEP).min(1.0) as f32;
        let ease = 1.0 - (1.0 - run) * (1.0 - run);
        let r = (range * ease).min((30.0 + corner * ease) * px).max(if age < SWEEP { 0.0 } else { range });
        let a = if age < SWEEP { 0.9 } else { 0.9 * (1.0 - ((age - SWEEP) / EDGE_FADE) as f32).max(0.0) };
        let n = ((std::f32::consts::TAU * r / px) / 10.0).clamp(24.0, 240.0) as usize;
        ring(&mut queue, &regions.white, at, r, n, 2.0 * px, ui.violet.with_alpha(a * fade));
        if age < SWEEP {
            ring(&mut queue, &regions.white, at, r * 0.92, n, 1.0 * px, ui.violet.with_alpha(0.35 * fade));
        }
    }

    let Some(v) = world.sim.voyage() else { return };
    // Bearings: dashed gold lines from the scan spots.
    let gold = ui.gold;
    for b in v.bearings() {
        if v.sites().get(b.site as usize).is_some_and(|s| s.known()) {
            continue;
        }
        let (from, dir) = (v2(b.from), v2(b.dir));
        let step = (DASH_PX + GAP_PX) * px;
        let n = ((BEARING_LENGTH / step) as usize).min(4000);
        for k in 0..n {
            let a = from + dir * (k as f32 * step);
            if !on_screen(a, step) {
                continue;
            }
            queue.line(&regions.white, a, a + dir * (DASH_PX * px), 2.0 * px, gold.with_alpha(0.7 * fade), Z_HUD);
        }
        if on_screen(from, 20.0 * px) {
            dotted(&mut queue, &regions.hard, (from, 7.0 * px), 8, 1.0, 3.0 * px, gold.with_alpha(0.9 * fade));
        }
    }
    // Found sites on screen: a dotted ring; a ping where one was just found.
    for (k, s) in v.sites().iter().enumerate() {
        if !s.known() {
            continue;
        }
        let (c, a, _) = site_look(&world.content, s, &ui);
        let at = v2(s.pos());
        if on_screen(at, 60.0) {
            let r = (40.0f32).max(18.0 * px);
            dotted(&mut queue, &regions.hard, (at, r), 40, 1.0, 2.5 * px, c.with_alpha(0.55 * a * fade));
        }
        for &(_, t) in fx.pings.iter().filter(|p| p.0 == k) {
            let age = ((now - t) / PING).clamp(0.0, 1.0) as f32;
            ring(&mut queue, &regions.white, at, (12.0 + PING_PX * age) * px, 40, 2.0 * px, c.with_alpha((1.0 - age) * fade));
        }
    }
}
