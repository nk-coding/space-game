//! The minimap (GDR-0106): on a voyage a small disc in the bottom right corner, always on, north up and centred on
//! the flown ship, with what the fleet knows round it drawn by the chart the cluster map shares (`chart.rs`): the
//! cluster's edge, rocks, scans and bearings, sites found, contacts within the sensors, the fleet. Under it the
//! range its radius shows and the MINIMAP key, which steps near → far → hidden. A faint thin ring at half the range.
//! The numbers are style tokens (`minimap.*` in `art/tokens.json`). On touch it sits above the fire button. It
//! steps aside while the cluster map is open; the edge markers keep clear of it (`HudZone`).

use super::*;
use super::chart::{Chart, Pen, paint};
use super::markers::hud_fade;
use kit::atlas::Atlas;

/// From the window's bottom edge (logical px; from its right edge the frame's margin); on touch the bottom clears the
/// fire button.
const BOTTOM: f32 = 46.0;
const BOTTOM_TOUCH: f32 = 184.0;

/// The minimap's numbers (style tokens) and its step: 0 near, 1 far, 2 hidden.
#[derive(Resource)]
pub(super) struct Minimap {
    step: u8,
    near: f32,
    far: f32,
    radius: f32,
    pub wreck_parts: usize,
}

impl Minimap {
    /// The metres its radius shows now (`None`: hidden).
    fn range(&self) -> Option<f32> {
        match self.step {
            0 => Some(self.near),
            1 => Some(self.far),
            _ => None,
        }
    }
}

/// The whole minimap (hidden off a voyage).
#[derive(Component)]
pub(super) struct MinimapBlock;

/// The disc's node: the drawing takes its place from it.
#[derive(Component)]
pub(super) struct MinimapDisc;

/// The line under it.
#[derive(Component)]
pub(super) struct MinimapLine;

/// The numbers, from the style tokens.
pub(super) fn load(mut commands: Commands, atlas: Res<Atlas>) {
    commands.insert_resource(Minimap {
        step: 0,
        near: atlas.token_f32("minimap.near_m"),
        far: atlas.token_f32("minimap.far_m"),
        radius: atlas.token_f32("minimap.radius_px"),
        wreck_parts: atlas.token_f32("minimap.wreck_parts") as usize,
    });
}

/// The nodes, hidden until a voyage: the disc (an `N` at its top) and the line under it.
pub(super) fn spawn(root: &mut ChildSpawnerCommands, fonts: &HudFonts, ui: &kit::style::UiPalette) {
    root.spawn((
        MinimapBlock,
        HudZone,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            right: px(kit::ui_kit::frame().edge),
            bottom: px(BOTTOM),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(6),
            ..default()
        },
    ))
    .with_children(|c| {
        c.spawn((MinimapDisc, Node { justify_content: JustifyContent::Center, ..default() }))
            .with_child((label(fonts, "N", 9.0, ui_color(ui.label, 0.8)), Node { margin: UiRect::top(px(3)), ..default() }));
        c.spawn((MinimapLine, label(fonts, "", kit::ui_kit::frame().hud.sub, ui_color(ui.dim, 1.0))));
    });
}

/// MINIMAP steps it, in a voyage.
pub(super) fn toggle(act: crate::settings::Act, scene: Res<Scene>, mut map: ResMut<Minimap>) {
    if scene.voyage().is_some() && act.just_pressed(Action::Minimap) {
        map.step = (map.step + 1) % 3;
    }
}

/// Places, sizes and words the nodes; shows them on a voyage while the minimap is on and the cluster map shut.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn update(
    world: Res<SimWorld>,
    scene: Res<Scene>,
    map: Res<Minimap>,
    overview: Res<super::overview::Overview>,
    input: Res<PlayerInput>,
    controls: Res<crate::settings::Controls>,
    mut block: Query<(&mut Node, &mut Visibility), (With<MinimapBlock>, Without<MinimapDisc>)>,
    mut disc: Query<&mut Node, (With<MinimapDisc>, Without<MinimapBlock>)>,
    mut line: Query<&mut Text, With<MinimapLine>>,
) {
    let Ok((mut node, mut vis)) = block.single_mut() else { return };
    let range = map.range().filter(|_| scene.voyage().is_some() && world.sim.voyage().is_some() && !overview.is_open());
    let want = if range.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
    }
    let Some(range) = range else { return };
    let bottom = px(if input.touch_ui { BOTTOM_TOUCH } else { BOTTOM });
    if node.bottom != bottom {
        node.bottom = bottom;
    }
    if let Ok(mut d) = disc.single_mut() {
        let size = px(2.0 * map.radius);
        if d.width != size {
            d.width = size;
            d.height = size;
        }
    }
    if let Ok(mut t) = line.single_mut() {
        let r = if range < 1000.0 { format!("{range:.0} M") } else { format!("{:.1} KM", range / 1000.0) };
        let key = controls.0.inputs(Action::Minimap).next().map_or_else(String::new, |c| format!("  ·  {}", c.label()));
        set_text(&mut t, &format!("RANGE {r}{key}"));
    }
}

/// The disc and what the chart knows in it, round the flown ship (else the camera).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn draw(
    world: Res<SimWorld>,
    map: Res<Minimap>,
    overview: Res<super::overview::Overview>,
    rig: Res<CameraRig>,
    focus: Res<PilotFocus>,
    regions: Option<Res<FxRegions>>,
    style: Res<Style>,
    hud: Query<&ScreenFx, With<HudRoot>>,
    disc: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), With<MinimapDisc>>,
    mut queue: ResMut<FxQueue>,
) {
    let fade = hud_fade(&hud);
    let (Some(v), Some(regions), Some(range)) = (world.sim.voyage(), regions, map.range()) else { return };
    let Ok((node, tf, vis)) = disc.single() else { return };
    let Some(rect) = logical_rect(node, tf).filter(|_| vis.get() && fade > 0.0) else { return };
    let ui = style.ui;
    let me = focus.0.and_then(|id| world.sim.ship(id)).and_then(|s| s.control_piece().and_then(|c| s.piece(c)));
    let origin = me.map_or(rig.pos, |p| piece_com(p, 1.0));
    let r = rect.width() / 2.0;
    let chart = Chart { centre: rect.center(), origin, scale: r / range, clip: r };
    let mut pen = Pen { rig: &rig, regions: &regions, queue: &mut queue, fade };
    pen.dot(chart.centre, 2.0 * (r + 5.0), style.background.outer().with_alpha(0.82));
    pen.ring(&Chart { clip: r + 6.0, ..chart }, chart.centre, r + 4.0, 1.0, ui.label.with_alpha(0.45));
    pen.ring(&chart, chart.centre, r / 2.0, 1.0, ui.label.with_alpha(0.22));
    paint(&chart, &mut pen, &world, v, overview.scans(), focus.0, map.wreck_parts, &ui);
}
