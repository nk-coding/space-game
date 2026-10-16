//! The cluster map in flight (galaxy plan E6, GDR-0097): the flight camera shows a few hundred metres, a cluster is
//! 2–3 km across, so N (the CLUSTER MAP action) lays the whole cluster over the view as a map, and again takes it
//! away. Vector, in the HUD's look, faded with the HUD; the flight goes on under it.
//!
//! * A dark disc over the view's middle, the cluster's edge a dotted ring, the entry a small diamond;
//! * every scan of the voyage a faint violet circle of what it swept; the bearings dashed gold lines from where they
//!   were taken (two crossing mark the place), until their site is found;
//! * every site found a dot in its marker's colour in a ring (a signal's site, gold, a ring more);
//! * the fleet: a teal dot per vessel, the flown one brighter with a tick along its heading.
//!
//! Over it, `CLUSTER MAP · VARDIS · 2.4 KM ACROSS` and what closes it.

use super::*;
use super::markers::{hud_fade, site_look};
use sim::SimEvent;

/// The map's radius as a share of the view's short side, and the room round the cluster's edge in it.
const RADIUS: f32 = 0.4;
const ROOM: f32 = 1.08;
/// A bearing's dashes (screen px).
const DASH: f32 = 7.0;
const GAP: f32 = 5.0;

/// The cluster map's state: open or not, and every scan of this flight (where, how far).
#[derive(Resource, Default)]
pub(super) struct Overview {
    open: bool,
    scans: Vec<(Vec2, f32)>,
}

impl Overview {
    pub(super) fn is_open(&self) -> bool {
        self.open
    }
}

/// The words over the map.
#[derive(Component)]
pub(super) struct OverviewTitle;

/// The title, hidden until the map opens.
pub(super) fn spawn(root: &mut ChildSpawnerCommands, fonts: &HudFonts, ui: &kit::style::UiPalette) {
    root.spawn((
        OverviewTitle,
        Visibility::Hidden,
        Node { position_type: PositionType::Absolute, width: percent(100), top: px(0), justify_content: JustifyContent::Center, ..default() },
    ))
    .with_child(label(fonts, "", 10.5, ui_color(ui.label, 1.0)));
}

/// N opens and closes it, in a voyage.
pub(super) fn toggle(act: crate::settings::Act, scene: Res<Scene>, mut map: ResMut<Overview>) {
    if scene.voyage().is_some() && act.just_pressed(Action::ClusterMap) {
        map.open = !map.open;
    }
}

/// Every tick: the player's scans, for the map.
pub(super) fn note(world: Res<SimWorld>, mut map: ResMut<Overview>) {
    for e in world.sim.events() {
        if let SimEvent::Scanned { ship, at, range, .. } = *e
            && world.sim.ship(ship).is_some_and(|s| s.owner() == crate::scene::PLAYER)
        {
            map.scans.push((v2(at), range));
        }
    }
}

/// A new flight: closed, no scans.
pub(super) fn forget(mut map: ResMut<Overview>) {
    *map = Overview::default();
}

/// The map (see the module doc) and its title.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn draw(
    world: Res<SimWorld>,
    map: Res<Overview>,
    rig: Res<CameraRig>,
    focus: Res<PilotFocus>,
    regions: Option<Res<FxRegions>>,
    style: Res<Style>,
    controls: Res<crate::settings::Controls>,
    hud: Query<&ScreenFx, With<HudRoot>>,
    mut title: Query<(&mut Node, &mut Visibility, &Children), With<OverviewTitle>>,
    mut texts: Query<&mut Text>,
    mut queue: ResMut<FxQueue>,
) {
    let fade = hud_fade(&hud);
    let v = world.sim.voyage().filter(|_| map.open && fade > 0.0);
    let Ok((mut node, mut vis, children)) = title.single_mut() else { return };
    let want = if v.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
    }
    let (Some(v), Some(regions)) = (v, regions) else { return };
    let ui = style.ui;
    let px = 1.0 / rig.zoom;
    let centre = rig.viewport / 2.0;
    let r = RADIUS * rig.viewport.min_element();
    let s = r / (v.radius() * ROOM);
    // Cluster metres (y up) → the screen (y down) → the world, where the queue draws.
    let to = |p: Vec2| rig.screen_to_world(centre + Vec2::new(p.x, -p.y) * s);
    let top = (centre.y - r - 26.0).max(8.0);
    if node.top != Val::Px(top) {
        node.top = Val::Px(top);
    }
    let key = controls.0.inputs(Action::ClusterMap).next().map_or_else(|| "—".to_string(), |c| c.label());
    if let Some(mut t) = children.first().and_then(|c| texts.get_mut(*c).ok()) {
        set_text(&mut t, &format!("CLUSTER MAP  ·  {}  ·  {:.1} KM ACROSS  ·  {key} CLOSES", v.id().to_uppercase(), v.radius() * 2.0 / 1000.0));
    }

    // The disc, the edge, the entry.
    queue.dot(&regions.hard, rig.screen_to_world(centre), 2.0 * (r + 16.0) * px, style.background.outer().with_alpha(0.86 * fade), Z_HUD);
    let edge = v.radius() * s;
    let n = ((std::f32::consts::TAU * edge / 8.0) as usize).max(24);
    for k in 0..n {
        let q = centre + Vec2::from_angle(k as f32 / n as f32 * std::f32::consts::TAU) * edge;
        queue.dot(&regions.hard, rig.screen_to_world(q), 2.2 * px, ui.label.with_alpha(0.5 * fade), Z_HUD);
    }
    let entry = to(v2(v.entry().pos));
    for (a, b) in [(Vec2::Y, Vec2::X), (Vec2::X, Vec2::NEG_Y), (Vec2::NEG_Y, Vec2::NEG_X), (Vec2::NEG_X, Vec2::Y)] {
        queue.line(&regions.white, entry + a * 5.0 * px, entry + b * 5.0 * px, 1.2 * px, ui.number.with_alpha(0.7 * fade), Z_HUD);
    }

    // What the scans swept, inside the map.
    for &(at, range) in &map.scans {
        let (c, rr) = (centre + Vec2::new(at.x, -at.y) * s, range * s);
        let m = ((std::f32::consts::TAU * rr / 6.0) as usize).clamp(24, 160);
        let a = |j: usize| c + Vec2::from_angle(j as f32 / m as f32 * std::f32::consts::TAU) * rr;
        for k in 0..m {
            let (p, q) = (a(k), a(k + 1));
            if (p - centre).length() <= r && (q - centre).length() <= r {
                queue.line(&regions.white, rig.screen_to_world(p), rig.screen_to_world(q), 1.0 * px, ui.violet.with_alpha(0.35 * fade), Z_HUD);
            }
        }
    }
    // Bearings, dashed to the edge of the map.
    for b in v.bearings() {
        if v.sites().get(b.site as usize).is_some_and(|x| x.known()) {
            continue;
        }
        let (from, dir) = (v2(b.from), v2(b.dir));
        let a = centre + Vec2::new(from.x, -from.y) * s;
        let d = Vec2::new(dir.x, -dir.y);
        let mut t = 0.0;
        while t < 2.0 * r {
            let (p, q) = (a + d * t, a + d * (t + DASH));
            if (p - centre).length() > r + 12.0 {
                break;
            }
            queue.line(&regions.white, rig.screen_to_world(p), rig.screen_to_world(q), 1.6 * px, ui.gold.with_alpha(0.8 * fade), Z_HUD);
            t += DASH + GAP;
        }
        queue.dot(&regions.hard, rig.screen_to_world(a), 5.0 * px, ui.gold.with_alpha(0.9 * fade), Z_HUD);
    }
    // The sites found.
    for site in v.sites().iter().filter(|x| x.known()) {
        let (c, a, _) = site_look(&world.content, site, &ui);
        let at = to(v2(site.pos()));
        queue.dot(&regions.hard, at, 6.0 * px, c.with_alpha(a * fade), Z_HUD);
        let rings: &[f32] = if site.signal() { &[9.0, 13.0] } else { &[9.0] };
        for rr in rings {
            let m = 28;
            for k in 0..m {
                let p = |j: usize| at + Vec2::from_angle(j as f32 / m as f32 * std::f32::consts::TAU) * rr * px;
                queue.line(&regions.white, p(k), p(k + 1), 1.0 * px, c.with_alpha(0.7 * a * fade), Z_HUD);
            }
        }
    }
    // The fleet.
    for vessel in world.sim.vessels(crate::scene::PLAYER) {
        let Some(ship) = world.sim.ship(vessel.ship) else { continue };
        let Some(p) = ship.control_piece().and_then(|c| ship.piece(c)) else { continue };
        let (_, angle) = p.render_pose(1.0);
        let at = to(piece_com(p, 1.0));
        let flown = focus.0 == Some(vessel.ship);
        queue.dot(&regions.hard, at, if flown { 7.0 } else { 5.0 } * px, ui.teal.with_alpha(if flown { 1.0 } else { 0.75 } * fade), Z_HUD);
        if flown {
            let h = Vec2::from_angle(angle + std::f32::consts::FRAC_PI_2);
            queue.line(&regions.white, at, at + h * 11.0 * px, 1.4 * px, ui.teal.with_alpha(fade), Z_HUD);
        }
    }
}
