//! The fleet bar (GDR-0013, GDR-0044; look GDR-0008; what it shows and when: GDR-0050): a compact strip at the
//! top centre, under the scene title, listing the player's vessels (`Sim::vessels(PLAYER)`, ascending ship id:
//! the V / Shift+V order).
//!
//! * A **mind row**: the logic cores the mind runs against its capacity (`Sim::mind_load`), as numbers and a thin
//!   violet bar.
//! * One **chip** per vessel: its number, one violet pip per logic core, a thin bar of its control load against
//!   its capacity (teal, gold above 85 %, coral while parts idle) and a state word: `PILOT` (the focus), `AUTO`
//!   (running its behavior), `DARK` (cores unpowered or overheated; also for a dark focus), `DORMANT` (over the
//!   mind capacity). The focus has a teal number and bright teal corner brackets.
//! * Click or tap a chip: a running or dark vessel takes the focus (`ShipCommand::Focus`), a dormant one wakes
//!   (`ShipCommand::Wake`: it swaps in, the newest other vessel goes dormant).
//! * Shown only while the player flies a vessel and has two or more, or any dormant one; single-ship play stays
//!   clean. No number hotkeys (1/2/3 are render toggles): V / Shift+V cycle.
//!
//! Chips are a fixed pool of nodes updated in place (no spawning while playing); text and colours are written only
//! when they change. Hit tests use the laid-out node rectangles, so clicks on the bar never reach the world
//! (`controls` asks [`FleetBar::blocks`]).

use std::fmt::Write as _;

use bevy::app::RunFixedMainLoopSystems;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use bevy_egui::input::egui_wants_any_pointer_input;
use sim::{ShipCommand, ShipId, VesselInfo, VesselState};

use crate::params::RenderToggles;
use crate::camera::CameraRig;
use crate::controls::ControlsSet;
use crate::flight_hud::HudZone;
use crate::mode::{AppMode, FlightUi};
use crate::scene::PLAYER;
use crate::sim_bridge::PilotFocus;
use crate::sim_bridge::SimWorld;
use crate::style::{Style, UiPalette, ui_color};
use crate::ui_kit::{self, HudFonts};

/// Chips in the pool (more vessels: a window around the focus and a "+N").
const MAX_CHIPS: usize = 12;
/// Chip size in logical pixels (touch targets of at least 44 px).
const CHIP_W: f32 = 68.0;
const CHIP_H: f32 = 50.0;
const CHIP_GAP: f32 = 6.0;
/// Core pips per chip (bigger cores counts show this many).
const MAX_PIPS: usize = 4;
const BAR_W: f32 = 42.0;
const MIND_BAR_W: f32 = 84.0;
/// Room the hull block (left) and the target panel (right) keep free at the top.
const SIDE_ROOM: f32 = 270.0;
/// Corner bracket arm length.
const CORNER: f32 = 9.0;
/// Control load share above which the bar turns gold.
const BUSY_SHARE: f32 = 0.85;

/// The bar's state for input: where the chips are (logical window pixels) and which vessel each shows.
#[derive(Resource, Default)]
pub struct FleetBar {
    shown: bool,
    /// The vessel in each chip slot (`None`: unused).
    slots: [Option<ShipId>; MAX_CHIPS],
    /// Laid-out chip rectangles (logical pixels, last frame), per slot.
    rects: [Option<Rect>; MAX_CHIPS],
    /// The whole bar (mind row and chips).
    block: Option<Rect>,
    hover: Option<usize>,
    /// The player's vessels this frame (reused buffer).
    vessels: Vec<VesselInfo>,
    scratch: String,
}

impl FleetBar {
    /// Is this logical window point on the bar (so a click or tap there is not for the world)?
    pub fn blocks(&self, p: Vec2) -> bool {
        self.shown && self.block.is_some_and(|r| r.contains(p))
    }

    fn chip_at(&self, p: Vec2) -> Option<usize> {
        if !self.shown {
            return None;
        }
        (0..MAX_CHIPS).find(|&k| self.slots[k].is_some() && self.rects[k].is_some_and(|r| r.contains(p)))
    }
}

#[derive(Component)]
struct FleetRoot;
#[derive(Component)]
struct FleetBlock;
#[derive(Component)]
struct MindValue;
#[derive(Component)]
struct MindFill;
#[derive(Component)]
struct MoreText;
#[derive(Component)]
struct Chip(usize);
#[derive(Component)]
struct ChipCorner(usize);
#[derive(Component)]
struct ChipNumber(usize);
#[derive(Component)]
struct ChipPip(usize, usize);
#[derive(Component)]
struct ChipFill(usize);
#[derive(Component)]
struct ChipTrack(usize);
#[derive(Component)]
struct ChipState(usize);

pub struct FleetBarPlugin;

impl Plugin for FleetBarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FleetBar>()
            .add_systems(Startup, spawn_bar.after(crate::flight_hud::spawn_hud))
            // Clicks with the other input, before the frame's sim ticks, so a focus switch reaches the next tick.
            .add_systems(
                RunFixedMainLoop,
                fleet_input
                    .run_if(not(egui_wants_any_pointer_input))
                    .run_if(in_state(AppMode::Flight))
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop)
                    .before(ControlsSet),
            )
            .add_systems(Update, update_bar.run_if(in_state(AppMode::Flight)));
    }
}

fn spawn_bar(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
    let ui = style.ui;
    let label = |text: &str, size: f32, color: Color| ui_kit::label(&fonts, text, size, ui_color(color, 1.0));
    let number = |text: &str, size: f32, color: Color| ui_kit::number(&fonts, text, size, ui_color(color, 1.0));
    let track = |w: f32| Node { width: px(w), height: px(2), ..default() };
    commands
        .spawn((
            FleetRoot,
            FlightUi,
            Node {

                position_type: PositionType::Absolute,
                top: px(62),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                FleetBlock,
                HudZone,
                Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(6), ..default() },
            ))
            .with_children(|block| {
                // Mind row: MIND 8 / 24 and a thin bar.
                block
                    .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..default() })
                    .with_children(|row| {
                        row.spawn(label("MIND", 10.0, ui.label));
                        row.spawn((MindValue, number("", 12.0, ui.number)));
                        row.spawn((track(MIND_BAR_W), BackgroundColor(ui_color(ui.dim, 0.35)))).with_children(|t| {
                            t.spawn((MindFill, Node { width: percent(0), height: percent(100), ..default() }, BackgroundColor(ui_color(ui.violet, 0.9))));
                        });
                    });
                // Chips.
                block
                    .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(CHIP_GAP), ..default() })
                    .with_children(|row| {
                        for k in 0..MAX_CHIPS {
                            spawn_chip(row, k, &label, &number, &ui);
                        }
                        row.spawn((MoreText, label("", 11.0, ui.label)));
                    });
            });
        });
}

fn spawn_chip<L: Bundle, N: Bundle>(
    row: &mut ChildSpawnerCommands,
    k: usize,
    label: &impl Fn(&str, f32, Color) -> L,
    number: &impl Fn(&str, f32, Color) -> N,
    ui: &UiPalette,
) {
    row.spawn((
        Chip(k),
        Node {
            display: Display::None,
            width: px(CHIP_W),
            height: px(CHIP_H),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(8), px(4)),
            row_gap: px(4),
            ..default()
        },
    ))
    .with_children(|chip| {
        // Corner brackets: two borders each.
        let b = px(1.5);
        let corners = [
            (UiRect { left: b, top: b, ..default() }, Some(0.0), None, Some(0.0), None),
            (UiRect { right: b, top: b, ..default() }, None, Some(0.0), Some(0.0), None),
            (UiRect { left: b, bottom: b, ..default() }, Some(0.0), None, None, Some(0.0)),
            (UiRect { right: b, bottom: b, ..default() }, None, Some(0.0), None, Some(0.0)),
        ];
        for (border, l, r, t, bt) in corners {
            chip.spawn((
                ChipCorner(k),
                Node {
                    position_type: PositionType::Absolute,
                    left: l.map(px).unwrap_or(Val::Auto),
                    right: r.map(px).unwrap_or(Val::Auto),
                    top: t.map(px).unwrap_or(Val::Auto),
                    bottom: bt.map(px).unwrap_or(Val::Auto),
                    width: px(CORNER),
                    height: px(CORNER),
                    border,
                    ..default()
                },
                BorderColor::all(ui_color(ui.label, 0.35)),
            ));
        }
        // Number and core pips.
        chip.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(4), ..default() })
            .with_children(|top| {
                top.spawn((ChipNumber(k), number("", 15.0, ui.number), Node { margin: UiRect::right(px(3)), ..default() }));
                for p in 0..MAX_PIPS {
                    top.spawn((
                        ChipPip(k, p),
                        Node { width: px(5), height: px(5), border_radius: BorderRadius::MAX, ..default() },
                        BackgroundColor(ui_color(ui.violet, 0.9)),
                    ));
                }
            });
        // Control load bar.
        chip.spawn((ChipTrack(k), Node { width: px(BAR_W), height: px(2), ..default() }, BackgroundColor(ui_color(ui.dim, 0.35))))
            .with_children(|t| {
                t.spawn((ChipFill(k), Node { width: percent(0), height: percent(100), ..default() }, BackgroundColor(ui_color(ui.teal, 0.9))));
            });
        // State word.
        chip.spawn((ChipState(k), label("", 8.5, ui.label)));
    });
}

/// Clicks and taps on the chips: focus a running or dark vessel, wake a dormant one.
#[allow(clippy::too_many_arguments)]
fn fleet_input(
    mut bar: ResMut<FleetBar>,
    chips: Query<(&Chip, &ComputedNode, &UiGlobalTransform, &InheritedVisibility)>,
    block: Single<(&ComputedNode, &UiGlobalTransform), With<FleetBlock>>,
    window: Single<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    focus: Res<PilotFocus>,
    mut world: ResMut<SimWorld>,
) {
    let rect = |node: &ComputedNode, tf: &UiGlobalTransform, grow: f32| {
        let s = node.inverse_scale_factor;
        Rect::from_center_size(tf.translation * s, node.size * s).inflate(grow)
    };
    bar.rects = [None; MAX_CHIPS];
    for (chip, node, tf, vis) in &chips {
        if vis.get() && node.size.cmpgt(Vec2::ZERO).all() {
            // Half the gap on each side, so a tap between two chips still lands on one.
            bar.rects[chip.0] = Some(rect(node, tf, CHIP_GAP / 2.0));
        }
    }
    let (node, tf) = *block;
    bar.block = node.size.cmpgt(Vec2::ZERO).all().then(|| rect(node, tf, 4.0));

    let cursor = window.cursor_position();
    bar.hover = cursor.and_then(|c| bar.chip_at(c));
    let mut presses: [Option<Vec2>; 4] = [None; 4];
    if buttons.just_pressed(MouseButton::Left) {
        presses[0] = cursor;
    }
    for (k, t) in touches.iter_just_pressed().take(3).enumerate() {
        presses[k + 1] = Some(t.position());
    }
    for p in presses.into_iter().flatten() {
        let Some(ship) = bar.chip_at(p).and_then(|k| bar.slots[k]) else { continue };
        let Some(v) = world.sim.vessel(ship) else { continue };
        let command = match v.state {
            VesselState::Dormant => ShipCommand::Wake,
            _ if focus.0 != Some(ship) => ShipCommand::Focus,
            _ => continue,
        };
        world.inputs.commands.push((ship, command));
    }
}

/// Fills the chips from the sim every frame (writes only what changed).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_bar(
    mut bar: ResMut<FleetBar>,
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    toggles: Res<RenderToggles>,
    rig: Res<CameraRig>,
    style: Res<Style>,
    mut root: Single<&mut Visibility, With<FleetRoot>>,
    mut chips: Query<(&Chip, &mut Node), (Without<ChipFill>, Without<MindFill>)>,
    mut corners: Query<(&ChipCorner, &mut BorderColor)>,
    mut texts: Query<
        (&mut Text, &mut TextColor, Option<&ChipNumber>, Option<&ChipState>, Has<MindValue>, Has<MoreText>),
        Or<(With<ChipNumber>, With<ChipState>, With<MindValue>, With<MoreText>)>,
    >,
    mut pips: Query<(&ChipPip, &mut Node, &mut BackgroundColor), (Without<Chip>, Without<ChipFill>, Without<MindFill>, Without<ChipTrack>)>,
    mut fills: Query<(Option<&ChipFill>, Has<MindFill>, &mut Node, &mut BackgroundColor), (Or<(With<ChipFill>, With<MindFill>)>, Without<Chip>, Without<ChipPip>)>,
    mut tracks: Query<(&ChipTrack, &mut BackgroundColor), (Without<ChipFill>, Without<MindFill>, Without<ChipPip>)>,
) {
    let ui = style.ui;
    let bar = &mut *bar;
    bar.vessels.clear();
    bar.vessels.extend(world.sim.vessels(PLAYER));
    let dormant = bar.vessels.iter().any(|v| v.state == VesselState::Dormant);
    let shown = toggles.hud && focus.0.is_some() && (bar.vessels.len() >= 2 || dormant);
    let want = if shown { Visibility::Inherited } else { Visibility::Hidden };
    if **root != want {
        **root = want;
    }
    bar.shown = shown;
    if !shown {
        bar.slots = [None; MAX_CHIPS];
        return;
    }

    // Which vessels get a chip: all of them, or a window around the focus that fits between the side panels.
    let n = bar.vessels.len();
    let fit = (((rig.viewport.x - 2.0 * SIDE_ROOM) / (CHIP_W + CHIP_GAP)).floor() as usize).clamp(3, MAX_CHIPS);
    let count = n.min(fit);
    let at = bar.vessels.iter().position(|v| v.focus).unwrap_or(0);
    let start = if n > count { at.saturating_sub(count / 2).min(n - count) } else { 0 };
    for (k, slot) in bar.slots.iter_mut().enumerate() {
        *slot = (k < count).then(|| bar.vessels[start + k].ship);
    }
    let chip_info = |k: usize| (k < count).then(|| bar.vessels[start + k]);

    for (chip, mut node) in &mut chips {
        let want = if chip_info(chip.0).is_some() { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    for (corner, mut color) in &mut corners {
        let Some(v) = chip_info(corner.0) else { continue };
        let c = if v.focus {
            ui_color(ui.teal, 0.95)
        } else if bar.hover == Some(corner.0) {
            ui_color(ui.number, 0.7)
        } else if v.state == VesselState::Dormant {
            ui_color(ui.dim, 0.5)
        } else {
            ui_color(ui.label, 0.4)
        };
        if color.top != c {
            *color = BorderColor::all(c);
        }
    }
    let (running, capacity) = world.sim.mind_load(PLAYER);
    let mut scratch = std::mem::take(&mut bar.scratch);
    for (mut text, mut color, num, state, mind, more) in &mut texts {
        scratch.clear();
        let mut c = ui.label;
        if let Some(&ChipNumber(k)) = num {
            let Some(v) = chip_info(k) else { continue };
            let _ = write!(scratch, "{}", start + k + 1);
            c = match v.state {
                _ if v.focus => ui.teal,
                VesselState::Running => ui.number,
                VesselState::Dark => ui.coral,
                VesselState::Dormant => ui.dim,
            };
        } else if let Some(&ChipState(k)) = state {
            let Some(v) = chip_info(k) else { continue };
            // A dark focus says DARK (its teal brackets still mark the focus).
            let (word, wc) = match v.state {
                VesselState::Running if v.focus => ("PILOT", ui.teal),
                VesselState::Running => ("AUTO", ui.label),
                VesselState::Dark => ("DARK", ui.coral),
                VesselState::Dormant => ("DORMANT", ui.dim),
            };
            scratch.push_str(word);
            c = wc;
        } else if mind {
            let _ = write!(scratch, "{running} / {capacity}");
            c = ui.number;
        } else if more && n > count {
            let _ = write!(scratch, "+{}", n - count);
        }
        if text.0 != scratch {
            text.0.clone_from(&scratch);
        }
        let c = ui_color(c, 1.0);
        if color.0 != c {
            color.0 = c;
        }
    }
    bar.scratch = scratch;
    for (pip, mut node, mut bg) in &mut pips {
        let Some(v) = chip_info(pip.0) else { continue };
        let want = if (pip.1 as u32) < v.cores { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
        let c = match v.state {
            VesselState::Running => ui_color(ui.violet, 0.95),
            VesselState::Dark => ui_color(ui.violet, 0.4),
            VesselState::Dormant => ui_color(ui.dim, 0.7),
        };
        if bg.0 != c {
            bg.0 = c;
        }
    }
    for (track, mut bg) in &mut tracks {
        let Some(v) = chip_info(track.0) else { continue };
        let c = ui_color(ui.dim, if v.state == VesselState::Dormant { 0.2 } else { 0.4 });
        if bg.0 != c {
            bg.0 = c;
        }
    }
    for (chip, mind, mut node, mut bg) in &mut fills {
        let (share, c) = if mind {
            ((running as f32 / capacity.max(1) as f32).min(1.0), ui_color(ui.violet, 0.9))
        } else {
            let Some(v) = chip.and_then(|c| chip_info(c.0)) else { continue };
            let ctl = v.control;
            if ctl.capacity == 0 {
                (0.0, ui_color(ui.dim, 0.6))
            } else {
                let share = ctl.load as f32 / ctl.capacity as f32;
                let c = if ctl.idle > 0 {
                    ui.coral
                } else if share > BUSY_SHARE {
                    ui.gold
                } else {
                    ui.teal
                };
                (share.min(1.0), ui_color(c, 0.9))
            }
        };
        let w = percent((share * 100.0).round());
        if node.width != w {
            node.width = w;
        }
        if bg.0 != c {
            bg.0 = c;
        }
    }
}
