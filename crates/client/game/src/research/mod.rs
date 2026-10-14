//! RESEARCH (track 7G; GDR-0063, GDR-0065: the tree with lanes): the
//! research building's screen. One lane per research family that has nodes, the nodes as cards in columns by
//! requirement depth with connectors from each requirement, the data pools in the lanes' heads and the materials
//! on top, the selected node's detail on the right (what it unlocks, a part preview like the drydock's, its cost
//! against the stores) and UNLOCK, which is the career model's `research`: it pays and unlocks at once, the save
//! is written, and the part shows NEW in the drydock (the career's seen-set).
//!
//! ```text
//! (o) SHIPYARD › RESEARCH                          ALSO IN STORE SHIELDS 3   ◆ PROGRAMMABLE MATTER 2
//!     Research
//!            TIER I               TIER II              │ NEW PART · ENERGY WEAPONS
//! HULL   ●──( Composite plating )┄┄( Cruiser drydock ) │ Beam laser
//! 14 DATA    NEW MATERIAL [HULL 14 / 20]               │ A continuous beam ...
//! ENERGY ●──( Beam laser       ◆)                      │ [part]  BUILD POINTS 7   MASS 4 t ...
//! 19 DATA    NEW PART [ENERGY 18]                      │ COST [ENERGY 18] IN STORE 19
//! LOGIC  ●──( Tamed core   DONE )──( Mind expansion I ◆) │ UNLOCKS Beam laser in the drydock, unlimited
//!                                                      │ [ UNLOCK · ENERGY 18 ]
//! ‹ HOME · ESC   1 READY · BEAM LASER                                                 DRYDOCK D
//! ```
//!
//! * [`tree`]: the pure part: the layout (lanes, columns, connectors), card states and words; tested there.
//! * [`screen`]: the widgets (`ui_kit`), built once from the content (the tree doesn't change while the game
//!   runs) and kept in step with the career.
//!
//! **Getting there**: home's research building (R, or a click on it) flies the camera onto the lab, then this
//! screen opens over it (the backdrop veiled, the station all but hidden, `backdrop::VeilsBackdrop`); Esc, the ring button and `‹ HOME` go back (home flies out from
//! the lab), DRYDOCK (D) opens the drydock. A LOCKED row of the drydock's palette (or a locked armour pill) opens
//! this screen on the node that unlocks it ([`DrydockAction::Research`]). `?scene=research` starts here.
//!
//! **Messages**: every button and key sends one [`ResearchAction`]; [`apply`] handles them. UI words are chrome in
//! code, like the other screens'.

pub mod screen;
pub mod tree;

use bevy::prelude::*;
use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;
use kit::ui_kit::{Tone, UiInputSet, UiPointer, UiStyleSet, ui_scale_for};
use world::camera::{CamMode, CameraRig};
use world::career::PlayerCareer;
use world::mode::AppMode;
use world::sim_bridge::GameContent;
use world::storage::Storage;

use crate::drydock::DockSet;
use crate::drydock::actions::DrydockAction;
use crate::shipyard::Yard;
use crate::shipyard::layout::{Place, fit};

use tree::{CardState, Tree};

/// What the research screen's buttons and keys do.
#[derive(Message, Clone, Debug, PartialEq)]
pub enum ResearchAction {
    /// Show this node's detail (a click on its card).
    Select(String),
    /// Research the selected node (the model's `research`): pay, unlock, write the save.
    Unlock,
    /// Back to the shipyard (Esc, the ring button, `‹ HOME`).
    Home,
    /// The drydock (D).
    Drydock,
}

/// The tree of the loaded content (built once).
#[derive(Resource, Debug, Clone)]
pub struct ResearchTree(pub Tree);

/// The screen's state: the selected node, the last thing said, and a node another screen asked to show.
#[derive(Resource, Debug, Default)]
pub struct ResearchView {
    pub selected: Option<String>,
    /// The last UNLOCK's result (teal) or refusal (coral).
    pub say: Option<(String, Tone)>,
    /// Opened on this node (the drydock's LOCKED rows); taken when the screen opens.
    pub request: Option<String>,
}

/// The flight camera's zoom while the research screen has the camera.
#[derive(Resource, Debug, Default)]
struct ResearchCamera {
    flight_zoom: f32,
}

/// The research screen's `Update` systems: input → apply → show (only on the research screen).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResearchSet {
    Input,
    Apply,
    Show,
}

pub struct ResearchPlugin;

impl Plugin for ResearchPlugin {
    fn build(&self, app: &mut App) {
        let tree = Tree::new(&app.world().resource::<GameContent>().0);
        app.insert_resource(ResearchTree(tree))
            .init_resource::<ResearchView>()
            .init_resource::<ResearchCamera>()
            .add_message::<ResearchAction>()
            .configure_sets(
                Update,
                (ResearchSet::Input, ResearchSet::Apply, ResearchSet::Show)
                    .chain()
                    .after(UiInputSet)
                    .before(UiStyleSet)
                    .run_if(in_state(AppMode::Research)),
            )
            .add_systems(Startup, screen::spawn.after(world::flight_hud::spawn_hud))
            .add_systems(PostStartup, remember_start_zoom.run_if(in_state(AppMode::Research)))
            .add_systems(OnEnter(AppMode::Research), enter)
            .add_systems(OnExit(AppMode::Research), exit)
            .add_systems(Update, screen::show.run_if(state_changed::<AppMode>))
            .add_systems(
                Update,
                (
                    (screen::clicks, keys).in_set(ResearchSet::Input),
                    (ui_scale, hold_camera, apply).chain().in_set(ResearchSet::Apply),
                    (screen::sync, screen::style_cards).chain().in_set(ResearchSet::Show),
                ),
            )
            // The drydock's LOCKED rows (a handler in the drydock's Apply set, as its own handlers).
            .add_systems(Update, from_drydock.in_set(DockSet::Apply));
    }
}

/// The node the screen opens on: the one asked for, else the one selected last time, else the first that can be
/// researched now, else the first whose requirements are met, else the first (in reading order).
fn opening_node(tree: &Tree, content: &data::Content, career: &data::Career, request: Option<String>, last: Option<&str>) -> Option<String> {
    let known = |n: &str| tree.card_of(n).is_some();
    if let Some(r) = request.filter(|r| known(r)) {
        return Some(r);
    }
    if let Some(l) = last.filter(|l| known(l)) {
        return Some(l.to_string());
    }
    // In reading order: lane by lane, then row, then column.
    let mut cards: Vec<&tree::Card> = tree.cards.iter().collect();
    cards.sort_by_key(|c| (c.lane, c.row, c.column));
    let first = |want: fn(&CardState) -> bool| cards.iter().find(|c| want(&tree::state(content, career, &c.node))).map(|c| c.node.clone());
    first(|s| *s == CardState::Ready)
        .or_else(|| first(|s| matches!(s, CardState::Short(_))))
        .or_else(|| first(|s| !matches!(s, CardState::Done)))
        .or_else(|| cards.first().map(|c| c.node.clone()))
}

/// Opening the screen: remember the flight zoom (given back on leaving), free the camera, pick the node shown.
fn enter(
    mut rig: ResMut<CameraRig>,
    mut cam: ResMut<ResearchCamera>,
    mut view: ResMut<ResearchView>,
    tree: Res<ResearchTree>,
    content: Res<GameContent>,
    career: Res<PlayerCareer>,
) {
    cam.flight_zoom = rig.zoom_target;
    rig.mode = CamMode::Free;
    rig.auto_mode = CamMode::Free;
    let request = view.request.take();
    view.selected = opening_node(&tree.0, &content.0, career.career(), request, view.selected.as_deref());
    view.say = None;
}

/// At start on the research screen: the flight zoom of the URL (`?zoom=`, applied at Startup) is the one to give
/// back.
fn remember_start_zoom(rig: Res<CameraRig>, mut cam: ResMut<ResearchCamera>) {
    cam.flight_zoom = rig.zoom_target;
}

/// Leaving: flight gets its zoom back and the UI its normal scale (home starts on the lab and flies out).
fn exit(mut rig: ResMut<CameraRig>, cam: Res<ResearchCamera>, mut ui_scale: ResMut<UiScale>) {
    rig.view_offset = Vec2::ZERO;
    rig.zoom = cam.flight_zoom;
    rig.zoom_target = cam.flight_zoom;
    ui_scale.0 = 1.0;
}

/// The layout is made for 1280 × 720 logical px and grows with larger windows (as the drydock's and home's).
fn ui_scale(window: Single<&Window, With<PrimaryWindow>>, mut ui_scale: ResMut<UiScale>) {
    let s = ui_scale_for(&window);
    if (ui_scale.0 - s).abs() > 1e-3 {
        ui_scale.0 = s;
    }
}

/// The camera rests on the lab, exactly where home's flight there ended (so nothing jumps when the screen opens),
/// refitted when the window changes.
fn hold_camera(window: Single<&Window, With<PrimaryWindow>>, yard: Res<Yard>, mut rig: ResMut<CameraRig>) {
    let size = window.size();
    let (centre, zoom) = fit(yard.layout.focus(Place::Research), size);
    if rig.pos != centre || rig.zoom != zoom || rig.viewport != size || rig.view_offset != Vec2::ZERO {
        rig.viewport = size;
        rig.view_offset = Vec2::ZERO;
        rig.end_zoom_anchor();
        rig.pos = centre;
        rig.zoom = zoom;
        rig.zoom_target = zoom;
    }
}

/// Esc and the ring button go home, D to the drydock (not while a text field has the keyboard, nor with Ctrl or
/// Cmd held, as everywhere).
fn keys(keys: Res<ButtonInput<KeyCode>>, pointer: Res<UiPointer>, mut out: MessageWriter<ResearchAction>) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]);
    if pointer.typing || ctrl {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        out.write(ResearchAction::Home);
    } else if keys.just_pressed(KeyCode::KeyD) {
        out.write(ResearchAction::Drydock);
    }
}

/// What the actions do. UNLOCK asks the model first: a refusal is said and nothing is written; a research is
/// written into the save at once (a failed write keeps the unlock for this game and says why).
#[allow(clippy::too_many_arguments)]
pub fn apply(
    mut actions: MessageReader<ResearchAction>,
    mut view: ResMut<ResearchView>,
    mut career: ResMut<PlayerCareer>,
    mut storage: ResMut<Storage>,
    content: Res<GameContent>,
    tree: Res<ResearchTree>,
    mut next: ResMut<NextState<AppMode>>,
) {
    let content = &content.0;
    for a in actions.read() {
        match a {
            ResearchAction::Select(node) if tree.0.card_of(node).is_some() => {
                if view.selected.as_ref() != Some(node) {
                    view.selected = Some(node.clone());
                    view.say = None;
                }
            }
            ResearchAction::Select(_) => {}
            ResearchAction::Unlock => {
                let Some(node) = view.selected.clone() else { continue };
                if let Err(r) = career.career().can_pay(content, &node) {
                    view.say = Some((tree::refusal_words(content, &r), Tone::Coral));
                    continue;
                }
                let (done, written) = career.update(&mut storage, |c| c.research(content, &node).map(|n| n.id.clone()));
                view.say = Some(match (done, written) {
                    (Ok(id), Ok(())) => {
                        let n = content.research.get(&id).expect("a researched node is content's");
                        (tree::unlocked_words(n, career.career().mind_capacity(content)), Tone::Teal)
                    }
                    (Ok(_), Err(e)) => (format!("UNLOCKED, BUT NOT SAVED: {e}").to_uppercase(), Tone::Coral),
                    (Err(r), _) => (tree::refusal_words(content, &r), Tone::Coral),
                });
            }
            ResearchAction::Home => next.set(AppMode::Home),
            ResearchAction::Drydock => next.set(AppMode::Drydock),
        }
    }
}

/// A LOCKED row of the drydock's palette (or a locked armour pill): the research screen opens on its node.
fn from_drydock(mut actions: MessageReader<DrydockAction>, mut view: ResMut<ResearchView>, mut next: ResMut<NextState<AppMode>>) {
    for a in actions.read() {
        if let DrydockAction::Research(node) = a {
            view.request = Some(node.clone());
            next.set(AppMode::Research);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::state::app::StatesPlugin;
    use data::Career;

    use super::*;
    use world::career::{CareerDoc, Switches, load};
    use world::storage::{Backend, MemoryBackend, Storage};

    fn app(save: &str) -> App {
        let content = data::base_content();
        let mut mem = MemoryBackend::default();
        let text = format!("// space-game: saves, format {}, saved 1\n(start: \"rim_awakening\", seed: 1, generator: {}{save})", data::career::FORMAT, data::career::GENERATOR);
        mem.write("saves", "career", &text).unwrap();
        let mut storage = Storage::new(mem);
        let switches = Switches { slot: "career".into(), new_game: false, free: false };
        let career = load(&mut storage, &content, switches, 1, 1);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_state(AppMode::Research)
            .insert_resource(ResearchTree(Tree::new(&content)))
            .insert_resource(GameContent(content))
            .insert_resource(career)
            .insert_resource(storage)
            .init_resource::<ResearchView>()
            .add_message::<ResearchAction>()
            .add_systems(Update, apply);
        app
    }

    fn send(app: &mut App, a: ResearchAction) {
        app.world_mut().write_message(a);
        app.update();
    }

    /// UNLOCK researches the selected node: paid, unlocked, fresh for the drydock, written into the save; a
    /// refusal is said and changes nothing.
    #[test]
    fn unlock_pays_unlocks_and_writes_the_save() {
        let mut app = app(", data: {\"kinetic\": 3.0, \"logic\": 2.0}");
        send(&mut app, ResearchAction::Select("tamed_core".into()));
        send(&mut app, ResearchAction::Unlock);
        let view = app.world().resource::<ResearchView>();
        assert_eq!(view.say, Some(("NOT ENOUGH DATA  ·  LOGIC 2 / 3  ·  PROGRAMMABLE MATTER 0 / 1".into(), Tone::Coral)));
        assert!(app.world().resource::<PlayerCareer>().career().researched().is_empty());

        send(&mut app, ResearchAction::Select("automaton_cannon".into()));
        assert_eq!(app.world().resource::<ResearchView>().say, None, "a new selection clears the last words");
        send(&mut app, ResearchAction::Unlock);
        let view = app.world().resource::<ResearchView>();
        assert_eq!(view.say, Some(("REVERSE-ENGINEERED CANNON  ·  NOW IN THE DRYDOCK".into(), Tone::Teal)));
        let career = app.world().resource::<PlayerCareer>().career().clone();
        assert_eq!(career.researched(), ["automaton_cannon"]);
        assert!(!career.data().contains_key("kinetic"), "paid down to nothing");
        let content = data::base_content();
        assert!(career.fresh_parts(&content).contains("automaton_cannon"), "NEW in the drydock");
        // The save on disk says the same.
        let saved = app.world().resource::<Storage>().read::<CareerDoc>("career").unwrap().unwrap();
        assert_eq!(saved.doc.0.researched(), ["automaton_cannon"]);
        // Twice: refused, nothing written twice.
        send(&mut app, ResearchAction::Unlock);
        assert_eq!(app.world().resource::<ResearchView>().say, Some(("ALREADY RESEARCHED".into(), Tone::Coral)));
    }

    /// HOME and DRYDOCK leave the screen; a node the tree doesn't have is not selected.
    #[test]
    fn the_way_out() {
        let mut app = app("");
        send(&mut app, ResearchAction::Select("nowhere".into()));
        assert_eq!(app.world().resource::<ResearchView>().selected, None);
        send(&mut app, ResearchAction::Drydock);
        app.update();
        assert_eq!(*app.world().resource::<State<AppMode>>().get(), AppMode::Drydock);
        let mut app = self::app("");
        send(&mut app, ResearchAction::Home);
        app.update();
        assert_eq!(*app.world().resource::<State<AppMode>>().get(), AppMode::Home);
    }

    /// The screen opens on the node asked for, else the last one, else the first ready one, else the first
    /// whose requirements are met.
    #[test]
    fn the_screen_opens_on_the_right_node() {
        let content = data::base_content();
        let tree = Tree::new(&content);
        let career = |extra: &str| {
            let mut c = Career::from_ron(&format!("(start: \"rim_awakening\", seed: 1, generator: {}{extra})", data::career::GENERATOR)).unwrap();
            c.reconcile(&content);
            c
        };
        let fresh = career("");
        assert_eq!(opening_node(&tree, &content, &fresh, None, None).as_deref(), Some("composite_plating"), "nothing ready: the first open one");
        let rich = career(", data: {\"energy\": 18.0}");
        assert_eq!(opening_node(&tree, &content, &rich, None, None).as_deref(), Some("beam_laser"));
        assert_eq!(opening_node(&tree, &content, &rich, None, Some("torch_drive")).as_deref(), Some("torch_drive"));
        assert_eq!(opening_node(&tree, &content, &rich, Some("cruiser_drydock".into()), Some("torch_drive")).as_deref(), Some("cruiser_drydock"));
        assert_eq!(opening_node(&tree, &content, &rich, Some("nowhere".into()), None).as_deref(), Some("beam_laser"));
    }
}
