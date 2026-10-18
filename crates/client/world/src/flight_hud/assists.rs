//! The pilot assists in the flight HUD (GDR-0088): top left, the column's last group under the speed, the assists of
//! the ship flown, one line each with its toggle key (`F2 · BRAKE`): teal while it decides its channel (it acts
//! whenever the pilot leaves the channel free), white while it is on, dim while off; an auto-lock the pilot paused by
//! dropping a lock says `PAUSED`. The title says `ASSISTS OFF` while ALL ASSISTS has switched them off. Hidden for a
//! ship without assists. Toggling one says what changed in the HUD's banner ([`switch_note`], `controls.rs`).

use super::*;
use sim::{AssistState, AssistSwitch};

/// Lines shown at most (a longer list ends in `+ N MORE`).
const ROWS: usize = 6;

/// The block (hidden without assists).
#[derive(Component)]
pub(super) struct AssistBlock;

#[derive(Component)]
pub(super) struct AssistTitle;

/// Line `k` of the block.
#[derive(Component)]
pub(super) struct AssistRow(usize);

/// The block, the last group of the HUD's top left column (under the speed), in its type.
pub(super) fn spawn(column: &mut ChildSpawnerCommands, fonts: &HudFonts, grey: Color, dim: Color) {
    let hud = &frame().hud;
    column
        .spawn((AssistBlock, HudZone, Visibility::Hidden, Node { flex_direction: FlexDirection::Column, row_gap: px(5), ..default() }))
        .with_children(|c| {
            c.spawn((AssistTitle, label(fonts, "ASSISTS", hud.head, grey), Node { margin: UiRect::bottom(px(2)), ..default() }));
            for k in 0..ROWS {
                c.spawn((AssistRow(k), label(fonts, "", hud.sub, dim)));
            }
        });
}

/// A rule's name for the HUD: its action's (content) name, with the weapon group for auto-fire (`AUTO-FIRE 2`).
pub fn assist_name(content: &data::Content, st: &AssistState, i: usize) -> String {
    let Some(rule) = st.program().program.rules.get(i) else { return String::new() };
    let mut name = content.behavior_nodes.get(&rule.action.name).map_or_else(|| rule.action.name.clone(), |n| n.name.clone()).to_uppercase();
    if let Some(sim::behavior::Channel::Fire(g)) = st.channel(i) {
        name.push_str(&format!(" {g}"));
    }
    name
}

/// What a switch does, for the banner (`BRAKE ON  ·  AUTO-FIRE 2 OFF`): the line and its tone (teal: something goes
/// on, grey: off, gold: nothing to switch).
/// `st` is the state before the switch; `key` the switch's key label.
pub fn switch_note(content: &data::Content, st: Option<&AssistState>, switch: AssistSwitch, key: &str) -> (String, Tone) {
    let Some(st) = st.filter(|st| !st.program().program.rules.is_empty()) else {
        return (format!("{key}  ·  THIS SHIP HAS NO PILOT ASSISTS"), Tone::Gold);
    };
    match switch {
        AssistSwitch::All if st.all_on() => ("ALL ASSISTS OFF".into(), Tone::Label),
        AssistSwitch::All => ("ALL ASSISTS ON".into(), Tone::Teal),
        AssistSwitch::Toggle(k) => {
            let rules: Vec<usize> = (0..st.program().program.rules.len()).filter(|&i| st.toggle(i) == Some(k)).collect();
            if rules.is_empty() {
                return (format!("{key}  ·  NO ASSIST ON TOGGLE {k}"), Tone::Gold);
            }
            // Each rule flips; the line says where each ends up.
            let lines: Vec<String> = rules.iter().map(|&i| format!("{} {}", assist_name(content, st, i), if st.is_on(i) { "OFF" } else { "ON" })).collect();
            let tone = if rules.iter().any(|&i| !st.is_on(i)) { Tone::Teal } else { Tone::Label };
            (lines.join("  ·  "), tone)
        }
    }
}

/// The block follows the flown ship's assists (8 Hz, like the HUD's texts).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn update(
    time: Res<Time<Real>>,
    mut timer: Local<Option<Timer>>,
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    controls: Res<crate::settings::Controls>,
    style: Res<Style>,
    mut block: Single<&mut Visibility, With<AssistBlock>>,
    mut title: Single<(&mut Text, &mut TextColor), (With<AssistTitle>, Without<AssistRow>)>,
    mut rows: Query<(&AssistRow, &mut Text, &mut TextColor, &mut Node), Without<AssistTitle>>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(0.125, TimerMode::Repeating));
    if !timer.tick(time.delta()).just_finished() {
        return;
    }
    let st = focus.0.and_then(|id| world.sim.ship(id)).and_then(|s| s.assists()).filter(|a| !a.program().program.rules.is_empty());
    let want = if st.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    if **block != want {
        **block = want;
    }
    let Some(st) = st else { return };
    let ui = &style.ui;
    let (text, color) = &mut *title;
    if st.all_on() {
        set_text(text, "ASSISTS");
        set_color(color, ui_color(ui.label, 1.0));
    } else {
        set_text(text, &format!("ASSISTS OFF  ·  {}", controls.0.label(Action::AllAssists)));
        set_color(color, ui_color(ui.gold, 1.0));
    }
    let n = st.program().program.rules.len();
    for (row, mut text, mut color, mut node) in &mut rows {
        let k = row.0;
        let shown = k < n;
        let display = if shown { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        if !shown {
            continue;
        }
        if k == ROWS - 1 && n > ROWS {
            set_text(&mut text, &format!("+ {} MORE", n - k));
            set_color(&mut color, ui_color(ui.dim, 1.0));
            continue;
        }
        let mut name = assist_name(&world.content, st, k);
        // Auto-lock waits after the pilot dropped a lock, until they lock again or switch it.
        if st.lock_paused() && st.channel(k) == Some(sim::behavior::Channel::Lock) {
            name.push_str("  ·  PAUSED");
        }
        let line = match st.toggle(k).and_then(Action::assist_toggle) {
            Some(a) => format!("{}  ·  {name}", controls.0.label(a)),
            None => name,
        };
        set_text(&mut text, &line);
        let c = match (st.all_on() && st.is_on(k), st.decides(k)) {
            (true, true) => ui_color(ui.teal, 1.0),
            (true, false) => ui_color(ui.number, 0.85),
            (false, _) => ui_color(ui.dim, 0.8),
        };
        set_color(&mut color, c);
    }
}
