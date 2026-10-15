//! The HUD in the world: the dotted lock ring around the target, markers at the screen's edge for enemies and for a
//! voyage's sites (kept clear of the HUD's texts and widgets), faded with the HUD.

use super::*;

/// A dotted ring around the locked target (world space).
#[allow(clippy::too_many_arguments)]
pub(super) fn lock_ring(
    time: Res<Time>,
    world: Res<SimWorld>,
    input: Res<PlayerInput>,
    rig: Res<CameraRig>,
    fixed: Res<Time<Fixed>>,
    regions: Option<Res<FxRegions>>,
    style: Res<Style>,
    views: Query<&PieceView>,
    hud: Query<&ScreenFx, With<HudRoot>>,
    mut queue: ResMut<FxQueue>,
) {
    let (Some(regions), Some(lock)) = (regions, input.lock) else { return };
    let fade = hud_fade(&hud);
    if fade <= 0.0 {
        return;
    }
    let alpha = fixed.overstep_fraction();
    let (ship, piece) = match lock {
        Target::Ship(id) => (id, world.sim.ship(id).and_then(|s| s.control_piece().or_else(|| s.main_piece()))),
        Target::Piece(id, p) => (id, Some(p)),
        Target::Point(_) => return,
    };
    let Some(piece_id) = piece else { return };
    let Some(p) = world.sim.ship(ship).and_then(|s| s.piece(piece_id)) else { return };
    let centre = piece_com(p, alpha);
    let r = world.views.get(&(ship, piece_id)).and_then(|e| views.get(*e).ok()).map(|v| v.radius).unwrap_or(3.0) + 1.2;
    let px = 1.0 / rig.zoom;
    let spin = time.elapsed_secs() * 0.6;
    let c = style.glow_enemy;
    let n = 36;
    for k in 0..n {
        let a = spin + k as f32 / n as f32 * std::f32::consts::TAU;
        queue.dot(&regions.hard, centre + Vec2::new(a.cos(), a.sin()) * r, 3.0 * px, c.with_alpha(0.7 * fade), Z_HUD);
    }
    for k in 0..4 {
        let a = -spin * 0.5 + k as f32 * std::f32::consts::FRAC_PI_2;
        let d = Vec2::new(a.cos(), a.sin());
        queue.line(&regions.white, centre + d * (r + 3.0 * px), centre + d * (r + 14.0 * px), 2.0 * px, c.with_alpha(0.9 * fade), Z_HUD);
    }
}

/// How much the HUD shows (its screen transition, `ui_kit::ScreenFx`): its markers in the world fade with it (H, and
/// the hand-over at the end of a voyage, `warp_fx.rs`).
pub(super) fn hud_fade(hud: &Query<&ScreenFx, With<HudRoot>>) -> f32 {
    hud.iter().next().map_or(1.0, ScreenFx::alpha)
}

/// Markers at the screen edge pointing at active enemies (of the focus; in watch mode of the watched ship) outside
/// the view, kept clear of the HUD's texts and widgets (bevy_ui draws over the world: a marker there hid them or
/// was hidden). A replayed fight framed as a whole marks every ship off screen, each in its side's colour (and
/// following a ship, the other side's in theirs).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn offscreen_markers(
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    scene: Res<Scene>,
    input: Res<PlayerInput>,
    rig: Res<CameraRig>,
    regions: Option<Res<FxRegions>>,
    style: Res<Style>,
    hud: Query<&ScreenFx, With<HudRoot>>,
    zones: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), Or<(With<HudText>, With<HudZone>)>>,
    mut rects: Local<Vec<Rect>>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(regions) = regions else { return };
    // The focus, else (watch) the watched ship; a replayed fight needs neither.
    let me = focus.0.or(scene.watched()).and_then(|id| world.sim.ship(id));
    if me.is_none() && scene.kind != SceneKind::Fight {
        return;
    }
    let fade = hud_fade(&hud);
    if fade <= 0.0 {
        return;
    }
    rects.clear();
    let shown = zones.iter().filter(|z| z.2.get());
    rects.extend(shown.filter_map(|(node, tf, _)| logical_rect(node, tf)).map(|r| r.inflate(MARKER_CLEARANCE)));
    let team = me.map(|m| m.team());
    let px = 1.0 / rig.zoom;
    let half = rig.half_extent();
    let centre = rig.view_center();
    let edge = rig.viewport / 2.0 - Vec2::splat(MARKER_INSET);
    let voyage = world.sim.voyage();
    for s in world.sim.ships() {
        // A sleeping site's ships are the site's marker (`site_markers`).
        let friend = team.is_some_and(|team| sim::friendly(team, s.team()));
        if !s.is_active() || friend || voyage.is_some_and(|v| v.asleep(s.id())) {
            continue;
        }
        let Some(p) = s.control_piece().and_then(|c| s.piece(c)) else { continue };
        let at = v2(p.com_world());
        let d = at - rig.pos;
        if d.x.abs() < half.x && d.y.abs() < half.y {
            continue;
        }
        // On screen (y down): the ray from the view centre towards the ship.
        let ray = rig.world_to_screen(at) - centre;
        let pos = rig.screen_to_world(marker_spot(centre, ray, edge, &rects));
        let dir = Vec2::new(ray.x, -ray.y).normalize_or_zero();
        let side = Vec2::new(-dir.y, dir.x);
        let locked = input.locked_ship() == Some(s.id());
        let glow = if world.looks.get(&s.owner()) == Some(&Look::Player) { style.glow_player } else { style.glow_enemy };
        let c = glow.with_alpha(if locked { 1.0 } else { 0.75 } * fade);
        let tip = pos + dir * 9.0 * px;
        queue.line(&regions.white, tip, pos - dir * 3.0 * px + side * 7.0 * px, 2.0 * px, c, Z_HUD);
        queue.line(&regions.white, tip, pos - dir * 3.0 * px - side * 7.0 * px, 2.0 * px, c, Z_HUD);
        queue.dot(&regions.soft, pos, 18.0 * px, c.with_alpha(0.25 * fade), Z_HUD - 0.1);
    }
}

/// How a site marker looks: its colour and alpha (raiders coral, wrecks only gold, cleared dim; the goal brighter),
/// and the label's first line (`RAIDER PACK · GOAL`, `... · CLEARED`).
pub(super) fn site_look(content: &data::Content, site: &sim::Site, ui: &kit::style::UiPalette) -> (Color, f32, String) {
    let mut name = voyage::encounter_name(content, site.encounter());
    if site.goal() {
        name.push_str("  ·  GOAL");
    }
    if site.state() == sim::SiteState::Cleared {
        name.push_str("  ·  CLEARED");
        return (ui.dim, 0.8, name);
    }
    let color = if site.plan().ships.is_empty() { ui.gold } else { ui.coral };
    (color, if site.goal() { 1.0 } else { 0.75 }, name)
}

/// Voyages: a marker at the screen edge for every site whose centre is off screen (like the enemy markers, kept
/// clear of the HUD), with its name and distance beside it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn site_markers(
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    scene: Res<Scene>,
    rig: Res<CameraRig>,
    regions: Option<Res<FxRegions>>,
    style: Res<Style>,
    hud: Query<&ScreenFx, With<HudRoot>>,
    fixed: Res<Time<Fixed>>,
    zones: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), Or<(With<HudText>, With<HudZone>)>>,
    mut labels: Query<(&SiteLabel, &mut Node, &ComputedNode, &mut Visibility)>,
    mut texts: Query<(&SiteLabelText, &mut Text, &mut TextColor)>,
    mut rects: Local<Vec<Rect>>,
    mut queue: ResMut<FxQueue>,
) {
    // The arrows fade with the HUD (its labels are in it).
    let fade = hud_fade(&hud);
    let voyage = world.sim.voyage().filter(|_| scene.voyage().is_some() && fade > 0.0);
    let mut spots: [Option<(Vec2, Vec2)>; SITE_LABELS] = [None; SITE_LABELS];
    if let (Some(v), Some(regions)) = (voyage, regions) {
        rects.clear();
        let visible = zones.iter().filter(|z| z.2.get());
        rects.extend(visible.filter_map(|(node, tf, _)| logical_rect(node, tf)).map(|r| r.inflate(MARKER_CLEARANCE)));
        let ui = style.ui;
        let alpha = fixed.overstep_fraction();
        let from = focus.0.and_then(|id| world.sim.ship(id)).and_then(|s| s.control_piece().and_then(|c| s.piece(c))).map_or(rig.pos, |p| piece_com(p, alpha));
        let (px, half, centre) = (1.0 / rig.zoom, rig.half_extent(), rig.view_center());
        let edge = rig.viewport / 2.0 - Vec2::splat(MARKER_INSET + 14.0);
        for (k, site) in v.sites().iter().enumerate().take(SITE_LABELS) {
            let at = v2(site.pos());
            let d = at - rig.pos;
            if d.x.abs() < half.x && d.y.abs() < half.y {
                continue;
            }
            let ray = rig.world_to_screen(at) - centre;
            let spot = marker_spot(centre, ray, edge, &rects);
            let (c, a, name) = site_look(&world.content, site, &ui);
            let dir = Vec2::new(ray.x, -ray.y).normalize_or_zero();
            let side = Vec2::new(-dir.y, dir.x);
            let pos = rig.screen_to_world(spot);
            let c = c.with_alpha(a);
            let arrow = c.with_alpha(a * fade);
            let size = if site.goal() { 1.25 } else { 1.0 };
            let tip = pos + dir * 9.0 * size * px;
            queue.line(&regions.white, tip, pos - dir * 3.0 * px + side * 7.0 * size * px, 2.0 * px, arrow, Z_HUD);
            queue.line(&regions.white, tip, pos - dir * 3.0 * px - side * 7.0 * size * px, 2.0 * px, arrow, Z_HUD);
            if site.goal() {
                queue.dot(&regions.soft, pos, 26.0 * px, c.with_alpha(0.3 * fade), Z_HUD - 0.1);
            }
            let dist = at.distance(from);
            for (t, mut text, mut color) in &mut texts {
                if t.0 != k {
                    continue;
                }
                if t.1 == 0 {
                    set_text(&mut text, &name);
                    set_color(&mut color, ui_color(c, if a < 0.8 { 0.95 } else { 1.0 }));
                } else if dist >= 1000.0 {
                    set_text(&mut text, &format!("{:.1} KM", dist / 1000.0));
                } else {
                    set_text(&mut text, &format!("{dist:.0} M"));
                }
            }
            spots[k] = Some((spot, ray.normalize_or_zero()));
        }
    }
    // Each label beside its arrow (towards the view's centre if there is room), kept inside the view and clear of
    // the HUD and of the labels placed before it.
    let mut order: Vec<_> = labels.iter_mut().collect();
    order.sort_by_key(|l| l.0.0);
    for (label, mut node, computed, mut vis) in order {
        let want = if spots[label.0].is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        let Some((spot, ray)) = spots[label.0] else { continue };
        let size = computed.size * computed.inverse_scale_factor;
        let r = label_spot(spot, ray, size, rig.viewport, &rects);
        rects.push(r.inflate(4.0));
        let (left, top) = (Val::Px(r.min.x.round()), Val::Px(r.min.y.round()));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
    }
}

/// Where a site marker's label of `size` goes (logical px, y down) for an arrow at `spot` whose ray points `ray`
/// (unit, screen): towards the view's centre, else below, above, right or left of the arrow, the first place inside
/// the `viewport` that meets none of `avoid`; the first one if none is free.
pub(super) fn label_spot(spot: Vec2, ray: Vec2, size: Vec2, viewport: Vec2, avoid: &[Rect]) -> Rect {
    let gap = 18.0;
    let down = Vec2::new(0.0, gap + size.y / 2.0);
    let step = Vec2::new(0.0, size.y + 6.0);
    let candidates = [
        spot - ray * (gap + 8.0 + (size.x * ray.x.abs() + size.y * ray.y.abs()) / 2.0),
        spot + down,
        spot - down,
        spot + Vec2::new(gap + size.x / 2.0, 0.0),
        spot - Vec2::new(gap + size.x / 2.0, 0.0),
        // Stacked under (or over) the others.
        spot + down + step,
        spot - down - step,
        spot + down + step * 2.0,
        spot - down - step * 2.0,
    ];
    let max = (viewport - size - Vec2::splat(6.0)).max(Vec2::splat(6.0));
    let place = |c: Vec2| {
        let at = (c - size / 2.0).clamp(Vec2::splat(6.0), max);
        Rect::from_corners(at, at + size)
    };
    candidates.iter().map(|&c| place(c)).find(|r| avoid.iter().all(|z| z.intersect(*r).is_empty())).unwrap_or_else(|| place(candidates[0]))
}

/// Where an off-screen marker sits (logical screen px, y down): on the `ray` from the view `centre` towards its
/// ship, where it meets the view's edge inset to `edge` (half sizes), then moved back along the ray until it is
/// out of every `zone`.
pub(super) fn marker_spot(centre: Vec2, ray: Vec2, edge: Vec2, zones: &[Rect]) -> Vec2 {
    let mut k = (edge.x / ray.x.abs().max(1e-3)).min(edge.y / ray.y.abs().max(1e-3));
    // Where the ray enters the slab `lo..hi` of one axis, as a multiple of the ray.
    let enter = |c: f32, d: f32, lo: f32, hi: f32| {
        if d.abs() < 1e-6 { f32::NEG_INFINITY } else { ((lo - c) / d).min((hi - c) / d) }
    };
    for _ in 0..=zones.len() {
        let p = centre + ray * k;
        let Some(r) = zones.iter().find(|r| r.contains(p)) else { break };
        let t = enter(centre.x, ray.x, r.min.x, r.max.x).max(enter(centre.y, ray.y, r.min.y, r.max.y));
        if t <= 0.0 {
            // The centre itself is in it: stay at the edge.
            break;
        }
        k = t - 1.0 / ray.length().max(1e-3);
    }
    centre + ray * k
}
