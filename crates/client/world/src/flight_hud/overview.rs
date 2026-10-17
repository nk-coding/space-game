//! The cluster map in flight (galaxy plan E6, GDR-0097): the flight camera shows a few hundred metres, a cluster is
//! 2–3 km across, so N (the CLUSTER MAP action) lays the whole cluster over the view as a map, and again takes it
//! away. A dark disc over the view's middle with the cluster drawn into it by the chart the minimap shares
//! (`chart.rs`: edge, entry, rocks, scans, bearings, sites found, contacts, the fleet); the flight goes on under it.
//!
//! Over it, `CLUSTER MAP · VARDIS · 2.4 KM ACROSS` and what closes it.

use super::*;
use super::chart::{Chart, Pen, paint};
use super::markers::hud_fade;
use sim::SimEvent;

/// The map's radius as a share of the view's short side, and the room round the cluster's edge in it.
const RADIUS: f32 = 0.4;
const ROOM: f32 = 1.08;

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

    /// Every scan of this flight: where it was taken, how far it swept.
    pub(super) fn scans(&self) -> &[(Vec2, f32)] {
        &self.scans
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
    numbers: Res<super::minimap::Minimap>,
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
    let centre = rig.viewport / 2.0;
    let r = RADIUS * rig.viewport.min_element();
    let chart = Chart { centre, origin: Vec2::ZERO, scale: r / (v.radius() * ROOM), clip: r };
    // Over the disc, clear of the HUD's title and its line under it (a short window put it on them).
    let top = (centre.y - r - 26.0).max(60.0);
    if node.top != Val::Px(top) {
        node.top = Val::Px(top);
    }
    let key = controls.0.inputs(Action::ClusterMap).next().map_or_else(|| "—".to_string(), |c| c.label());
    if let Some(mut t) = children.first().and_then(|c| texts.get_mut(*c).ok()) {
        set_text(&mut t, &format!("CLUSTER MAP  ·  {}  ·  {:.1} KM ACROSS  ·  {key} CLOSES", v.id().to_uppercase(), v.radius() * 2.0 / 1000.0));
    }
    let mut pen = Pen { rig: &rig, regions: &regions, queue: &mut queue, fade };
    pen.dot(centre, 2.0 * (r + 16.0), style.background.outer().with_alpha(0.86));
    paint(&chart, &mut pen, &world, v, &map.scans, focus.0, numbers.wreck_parts, &ui);
}
