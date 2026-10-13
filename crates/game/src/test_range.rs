//! TEST IN RANGE (GDR-0057, written 13 Oct 2026): fly a behavior against raider waves, started by the behavior
//! screen (a [`StartTest`] message) or the URL (`?scene=watch&behavior=<id>`, `?scene=alongside&behavior=<id>`;
//! without `behavior`, the one the design names).
//!
//! * **Watch** ([`TestMode::Watch`]): the player's design flies unpiloted on the behavior, raider waves on, the
//!   camera follows it and the HUD shows it; nothing is piloted (focus switches are dropped).
//! * **Fly alongside** ([`TestMode::Alongside`]): the player flies the design as usual; a second copy joins as a
//!   wingman running the behavior (its leader is the player's ship, so `target locked` follows the player's lock).
//! * The behavior ([`TestProgram`]) is a content or saved player behavior by id, or a program not saved yet (the
//!   editor's text), which the flight's library gets under [`TEST_BEHAVIOR`]. [`check`] says beforehand whether
//!   it can fly; one that can't (a URL naming a behavior with errors) leaves the design's own, with a banner.
//! * T returns to where the test started ([`FlightExit`]: the screen that sent the request; the drydock for the
//!   URL), and so does losing the tested ship (watch) or every vessel (alongside), after 3 s. Shift+R starts over.
//! * [`BehaviorWatch`] (a resource) holds the tested ship's behavior live, refreshed after every think: which rule
//!   decides, every rule's status and why. The F3 overlay shows it.
//!
//! The scene (`scene.rs`) builds and runs the test world; this module holds the request, the checks and the live
//! view.

use bevy::prelude::*;
use data::Content;
use data::behavior::Program;
use sim::{BehaviorId, RuleStatus, ShipDesign, ShipId};

use crate::drydock::behaviors::{PlayerBehaviors, check_text, errors_note};
use crate::mode::{AppMode, FlightExit};
use crate::params::Params;
use crate::scene::{ResetRequest, Scene};
use crate::sim_bridge::{SimSet, SimWorld};

/// The id a program not saved yet flies under in a test (`:` never appears in content or storage ids).
pub const TEST_BEHAVIOR: &str = "test:behavior";

/// How TEST IN RANGE flies the behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestMode {
    /// The design flies unpiloted on the behavior; the camera follows it.
    Watch,
    /// The player flies the design; a second copy joins as a wingman running the behavior.
    Alongside,
}

/// The behavior under test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TestProgram {
    /// The one the design names (or its faction's).
    Design,
    /// A content or saved player behavior, by id.
    Id(String),
    /// A program not saved yet (the editor's text form), with the name to show.
    #[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen sends it (GDR-0057 wave 2)"))]
    Text { name: String, text: String },
}

/// What a test flies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BehaviorTest {
    pub mode: TestMode,
    pub program: TestProgram,
}

impl BehaviorTest {
    /// `?scene=watch` or `?scene=alongside`, with `&behavior=<id>` (else the design's own).
    pub fn from_params(params: &Params) -> Option<Self> {
        let mode = match params.text("scene")? {
            "watch" => TestMode::Watch,
            "alongside" => TestMode::Alongside,
            _ => return None,
        };
        let program = params.text("behavior").filter(|id| !id.is_empty()).map_or(TestProgram::Design, |id| TestProgram::Id(id.into()));
        Some(Self { mode, program })
    }
}

/// Starts TEST IN RANGE (the behavior screen sends it): the flight is built for the test, and T (or losing the
/// tested ship) returns to `back`.
#[derive(Message, Clone, Debug)]
pub struct StartTest {
    pub test: BehaviorTest,
    pub back: AppMode,
}

/// Can this program be tested? A content behavior, a saved one without errors, or text without errors (the
/// first error and how many, if not). `content` is the game's (`GameContent`).
#[cfg_attr(not(test), expect(dead_code, reason = "the behavior screen calls it (GDR-0057 wave 2)"))]
pub fn check(content: &Content, behaviors: &PlayerBehaviors, program: &TestProgram) -> Result<(), String> {
    match program {
        TestProgram::Design => Ok(()),
        TestProgram::Id(id) if content.behaviors.contains(id) => Ok(()),
        TestProgram::Id(id) => match behaviors.get(id) {
            Some(p) if p.runs() => Ok(()),
            Some(p) => Err(format!("{} has {}", p.behavior.name, errors_note(&p.errors))),
            None => Err(format!("there is no behavior {id}")),
        },
        TestProgram::Text { text, .. } => check_text(text, content).map(|_| ()).map_err(|e| errors_note(&e)),
    }
}

/// The tested behavior in a test flight's library (a program not saved yet is added to it under
/// [`TEST_BEHAVIOR`]); `Err`: why it can't fly.
pub fn resolve(world: &mut SimWorld, program: &TestProgram, design: &ShipDesign) -> Result<BehaviorId, String> {
    let lib = world.sim.behaviors_mut();
    match program {
        TestProgram::Design => lib.default_for(design).ok_or_else(|| "the design has no behavior".into()),
        TestProgram::Id(id) => lib.id(id).ok_or_else(|| format!("behavior {id} can't fly (unknown, or it has errors)")),
        TestProgram::Text { name, text } => {
            let (program, map) = Program::parse_with_lines(text).map_err(|e| errors_note(&e))?;
            lib.add_program(TEST_BEHAVIOR, name, program)
                .map_err(|e| errors_note(&e.into_iter().map(|e| map.to_text_error(e)).collect::<Vec<_>>()))
        }
    }
}

/// The tested ship's behavior, live (for the behavior screen's highlight and the F3 overlay). Refreshed after
/// every think of the ship (10 Hz), so `is_changed()` says when there is something new; empty outside a test.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BehaviorWatch {
    /// The running test, if any.
    pub mode: Option<TestMode>,
    /// The tested ship (watch: the design; alongside: the wingman).
    pub ship: Option<ShipId>,
    /// It is a vessel running its behavior now (false: not yet, piloted, or lost).
    pub running: bool,
    /// The behavior's id in the flight's library (a content or player behavior id, or [`TEST_BEHAVIOR`]) and name.
    pub behavior: Option<String>,
    pub name: String,
    /// The rule that decides (0-based); `None`: no rule matches (the ship brakes and holds).
    pub active: Option<usize>,
    /// Every rule's status at the last think, in program order.
    pub rules: Vec<RuleStatus>,
    /// Why each rule is (not) active, one line per rule ("`hull < 30%` is false (hull is 82%)").
    pub why: Vec<String>,
    /// Working rule slots (rules from this index on are `NoSlot`).
    pub slots: u32,
    /// The ship the rules are about.
    pub target: Option<ShipId>,
    /// The tick of the last think.
    pub think: Option<u64>,
}

pub struct TestRangePlugin;

impl Plugin for TestRangePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<StartTest>()
            .init_resource::<BehaviorWatch>()
            .add_systems(Update, start)
            .add_systems(FixedUpdate, watch.after(SimSet::Events).run_if(in_state(AppMode::Flight)))
            .add_systems(OnExit(AppMode::Flight), |mut watch: ResMut<BehaviorWatch>| *watch = BehaviorWatch::default());
    }
}

/// A [`StartTest`]: the scene becomes the test (from the drydock or the behavior screen: a launch; in flight: a
/// rebuild).
fn start(
    mut requests: MessageReader<StartTest>,
    mut scene: ResMut<Scene>,
    mut exit: ResMut<FlightExit>,
    mode: Res<State<AppMode>>,
    mut next: ResMut<NextState<AppMode>>,
    mut reset: ResMut<ResetRequest>,
) {
    let Some(request) = requests.read().last().cloned() else { return };
    scene.begin_test(request.test);
    exit.0 = request.back;
    if *mode.get() == AppMode::Flight {
        reset.0 = true;
    } else {
        scene.launch = true;
        next.set(AppMode::Flight);
    }
}

/// Copies the tested ship's behavior state into [`BehaviorWatch`] after each of its thinks.
fn watch(scene: Res<Scene>, world: Res<SimWorld>, mut watch: ResMut<BehaviorWatch>) {
    let (mode, ship) = (scene.test_mode(), scene.test_ship());
    if (watch.mode, watch.ship) != (mode, ship) {
        *watch = BehaviorWatch { mode, ship, ..default() };
    }
    let Some(id) = ship else { return };
    let Some(b) = world.sim.ship(id).filter(|s| s.is_vessel()).and_then(|s| s.behavior()) else {
        if watch.running {
            watch.running = false;
            watch.active = None;
        }
        return;
    };
    if watch.running && watch.think == b.last_think() {
        return;
    }
    let w = &mut *watch;
    w.running = true;
    w.think = b.last_think();
    if w.behavior.as_deref() != Some(b.program().id.as_str()) {
        w.behavior = Some(b.program().id.clone());
        w.name = b.program().name.clone();
    }
    w.active = b.active_rule();
    w.rules.clear();
    w.rules.extend_from_slice(b.rules());
    w.why.clear();
    w.why.extend((0..b.rules().len()).map(|i| b.describe(i)));
    w.slots = b.slots();
    w.target = b.target();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{MemoryBackend, Storage};

    #[test]
    fn url_parameters_name_the_mode_and_the_behavior() {
        let t = |q: &str| BehaviorTest::from_params(&Params::parse(q));
        assert_eq!(t("?scene=watch&behavior=careful"), Some(BehaviorTest { mode: TestMode::Watch, program: TestProgram::Id("careful".into()) }));
        assert_eq!(t("?scene=alongside"), Some(BehaviorTest { mode: TestMode::Alongside, program: TestProgram::Design }));
        assert_eq!(t("?scene=drydock&behavior=careful"), None);
        assert_eq!(t("?behavior=careful"), None);
    }

    #[test]
    fn checks_say_what_can_fly() {
        let content = data::base_content();
        let mut storage = Storage::new(MemoryBackend::default());
        let mut behaviors = PlayerBehaviors::default();
        let mut good = behaviors.copy_of(&content, "skirmisher").unwrap();
        good.rules[0] = "when hull < 50% -> flee".into();
        let good = behaviors.save(&mut storage, &content, &good, 1).unwrap();
        let mut bad = behaviors.copy_of(&content, "raider_basic").unwrap();
        bad.rules[0] = "when hull < 50 -> flee".into();
        let bad = behaviors.save(&mut storage, &content, &bad, 2).unwrap();
        let id = |s: &str| TestProgram::Id(s.into());
        assert_eq!(check(&content, &behaviors, &TestProgram::Design), Ok(()));
        assert_eq!(check(&content, &behaviors, &id("raider_basic")), Ok(()));
        assert_eq!(check(&content, &behaviors, &id(&good)), Ok(()));
        assert!(check(&content, &behaviors, &id(&bad)).unwrap_err().contains("1 error: line 2"));
        assert!(check(&content, &behaviors, &id("nope")).is_err());
        let text = |t: &str| TestProgram::Text { name: "Draft".into(), text: t.into() };
        assert_eq!(check(&content, &behaviors, &text("target weakest\nalways -> orbit(50%)")), Ok(()));
        assert!(check(&content, &behaviors, &text("always -> orbitt")).unwrap_err().contains("line 1, column 11"));
    }

    #[test]
    fn programs_resolve_in_the_flight_library() {
        let content = data::base_content();
        let mut sim = sim::Sim::new(1);
        sim.set_behaviors(sim::BehaviorLibrary::new(&content).unwrap());
        let mut world = SimWorld::new(sim, content.clone());
        let runner = ShipDesign::compile(&content, content.ships.get("halvex_runner").unwrap()).unwrap();
        let skirmisher = world.sim.behaviors().id("skirmisher");
        assert_eq!(resolve(&mut world, &TestProgram::Design, &runner).ok(), skirmisher);
        assert_eq!(resolve(&mut world, &TestProgram::Id("raider_basic".into()), &runner).ok(), world.sim.behaviors().id("raider_basic"));
        assert!(resolve(&mut world, &TestProgram::Id("careful".into()), &runner).is_err());
        // Unsaved text flies under its own id, never shadowing a saved one.
        let draft = TestProgram::Text { name: "Draft".into(), text: "always -> hold".into() };
        let h = resolve(&mut world, &draft, &runner).unwrap();
        let b = world.sim.behaviors().get(h).unwrap();
        assert_eq!((b.id.as_str(), b.name.as_str(), b.rule_count()), (TEST_BEHAVIOR, "Draft", 1));
        let broken = TestProgram::Text { name: "Draft".into(), text: "always -> hold\nwhen sheild_down -> flee".into() };
        assert!(resolve(&mut world, &broken, &runner).unwrap_err().contains("line 2, column 6"));
    }
}
