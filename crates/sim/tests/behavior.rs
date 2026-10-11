//! Behaviors: vocabulary ↔ implementation checks, rule evaluation (first match wins, why the others
//! failed, rule slots, no target), target selectors and priorities, the content raider against the old
//! raider brain, Rhai conditions (sandbox, budget, determinism), cadence and API details.

mod common;

use common::*;
use sim::behavior::{
    BUILTIN_NODES, BehaviorLibrary, RuleStatus, ScriptFault, Why, check_vocabulary,
};

use sim::{
    Body, DamageTarget, Maneuver, PilotInput, RaiderBrain, ShipCommand, ShipId, Sim, SimEvent,
    Target, TickInputs, Vec2,
};

fn lib() -> BehaviorLibrary {
    BehaviorLibrary::new(&content()).unwrap_or_else(|p| {
        panic!(
            "{}",
            p.iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
}

fn world_with_lib(lib: BehaviorLibrary) -> Sim {
    let mut w = Sim::new(7);
    w.set_behaviors(lib);
    w
}

fn run(w: &mut Sim, ticks: usize) {
    for _ in 0..ticks {
        w.step(&TickInputs::default());
    }
}

/// Adds a program to the world's library and gives it to `ship`.
fn program(w: &mut Sim, ship: ShipId, text: &str) {
    let id = w
        .behaviors_mut()
        .add_text("test", text)
        .unwrap_or_else(|e| panic!("{text}: {e:?}"));
    w.command(ship, ShipCommand::Behavior(Some(id)));
}

fn state(w: &Sim, ship: ShipId) -> &sim::BehaviorState {
    w.ship(ship).unwrap().behavior().expect("has a behavior")
}

/// A runner at the origin (nose up) and raiders at the given points, all idle.
fn duel(raiders: &[(f32, f32)]) -> (Sim, ShipId, Vec<ShipId>) {
    let c = content();
    let mut w = world_with_lib(lib());
    let me = w.spawn_ship(design(&c, "halvex_runner"), Body::at(Vec2::ZERO, 0.0));
    let rs = raiders
        .iter()
        .map(|&(x, y)| {
            w.spawn_ship(
                design(&c, "automaton_raider"),
                Body::at(Vec2::new(x, y), 0.0),
            )
        })
        .collect();
    (w, me, rs)
}

#[test]
fn vocabulary_and_implementation_agree() {
    let c = content();
    assert!(
        check_vocabulary(&c).is_empty(),
        "{:?}",
        check_vocabulary(&c)
    );
    for sig in BUILTIN_NODES {
        let n = c.behavior_nodes.get(sig.id).unwrap();
        assert!(n.script.is_none());
        assert_eq!(n.role, sig.role);
    }
    let l = lib();
    let raider = design(&c, "automaton_raider");
    assert_eq!(l.default_for(&raider), l.id("raider_basic"));
    assert_eq!(
        l.default_for(&design(&c, "halvex_runner")),
        l.id("skirmisher")
    );
    assert_eq!(
        l.get(l.id("raider_basic").unwrap()).unwrap().rule_count(),
        2
    );
}

#[test]
fn vocabulary_mismatches_are_reported() {
    let base = data::base_pack().unwrap();
    let m = data::Pack::from_files([
        (
            "pack.ron",
            r#"(id: "m", name: "M", version: "1", schema_version: 1, depends: ["base"])"#,
        ),
        (
            "behavior_nodes/n.ron",
            r#"[
                (id: "teleport", name: "Teleport", summary: "No code.", role: Action),
                (id: "orbit", name: "Orbit", summary: "Wrong options.", role: Action, params: [
                    (id: "radius", name: "R", type: Distance, default: "60%"),
                    (id: "dir", name: "D", type: Choice([(id: "left", name: "L"), (id: "right", name: "R")]), default: "left"),
                ]),
                (id: "bad_syntax", name: "B", summary: "B.", role: Condition, script: "me.hull >"),
                (id: "uses_pow", name: "P", summary: "P.", role: Condition, script: "2.0 ** me.hull > 1.0"),
                (id: "typo", name: "T", summary: "T.", role: Condition, params: [(id: "limit", name: "L", type: Percent, default: "50%")], script: "me.hull > limt"),
                (id: "shadow", name: "S", summary: "S.", role: Condition, params: [(id: "me", name: "Me", type: Count, default: "1")], script: "true"),
            ]"#,
        ),
    ])
    .unwrap();
    let c = data::Content::merge(&[base, m]);
    let problems = check_vocabulary(&c);
    let has = |id: &str, needle: &str| {
        assert!(
            problems
                .iter()
                .any(|p| p.id.as_deref() == Some(id) && p.message.contains(needle)),
            "{id}: {needle:?} not in\n{}",
            problems
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    has("teleport", "no implementation in the sim");
    has("orbit", "implements this parameter as Choice");
    has("bad_syntax", "Rhai:");
    has("uses_pow", "Rhai:");
    has("uses_pow", "**");
    has("typo", "limt");
    has("shadow", "name of the script API");

    // A built-in without a content entry is reported too (the editor and wiki need its text).
    let mut c = content();
    let mut nodes = data::Table::default();
    for n in c.behavior_nodes.iter().filter(|n| n.id != "hold") {
        nodes.upsert(n.clone());
    }
    c.behavior_nodes = nodes;
    c.behaviors = data::Table::default();
    let problems = check_vocabulary(&c);
    assert!(
        problems
            .iter()
            .any(|p| p.id.as_deref() == Some("hold")
                && p.message.contains("no content node defines it")),
        "{problems:?}"
    );
}

#[test]
fn first_matching_rule_wins_and_the_others_say_why() {
    let (mut w, me, rs) = duel(&[(0.0, 80.0)]);
    program(
        &mut w,
        me,
        "when hull < 100% -> flee
         when target_in_range(40m) -> face
         when target_in_range(500m) -> orbit(30m, ccw)
         always -> hold",
    );
    run(&mut w, 1);
    let st = state(&w, me);
    assert_eq!(st.active_rule(), Some(2));
    assert_eq!(st.target(), Some(rs[0]));
    assert_eq!(st.slots(), 12);
    assert_eq!(
        st.rules()[0],
        RuleStatus::Failed {
            cond: 0,
            why: Why::False { value: Some(1.0) }
        }
    );
    match st.rules()[1] {
        RuleStatus::Failed {
            cond: 0,
            why: Why::False { value: Some(d) },
        } => assert!((70.0..90.0).contains(&d), "{d}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(st.rules()[2], RuleStatus::Active);
    assert_eq!(st.rules()[3], RuleStatus::Skipped);
    assert_eq!(
        st.maneuver(),
        Maneuver::Orbit {
            target: Target::Ship(rs[0]),
            radius: 30.0,
            ccw: true
        }
    );
    assert!(
        st.describe(0)
            .contains("`hull < 100%` is false (hull is 100%)"),
        "{}",
        st.describe(0)
    );
    assert!(
        st.describe(3).contains("rule 3 decides"),
        "{}",
        st.describe(3)
    );

    // Hurt the hull a little: the first rule takes over at the next think.
    let armor = w
        .ship(me)
        .unwrap()
        .design()
        .parts
        .iter()
        .position(|p| p.hull)
        .unwrap() as u32;
    w.apply_damage(me, DamageTarget::Part(armor), 0.1);
    run(&mut w, sim::behavior::tuning::THINK_PERIOD as usize);
    let st = state(&w, me);
    assert_eq!(st.active_rule(), Some(0));
    assert_eq!(
        st.maneuver(),
        Maneuver::Flee {
            from: Target::Ship(rs[0])
        }
    );
    assert_eq!(st.rules()[1], RuleStatus::Skipped);
}

#[test]
fn rules_beyond_the_working_slots_are_inactive() {
    let (mut w, me, _) = duel(&[(0.0, 80.0)]);
    let mut text = String::new();
    for _ in 0..7 {
        text += "when hull < 1% -> hold\n";
    }
    text += "always -> face\n";
    program(&mut w, me, &text);
    run(&mut w, 1);
    assert_eq!(state(&w, me).active_rule(), Some(7));
    assert_eq!(state(&w, me).slots(), 12);

    // Destroy one of the runner's two logic cores (6 slots each): rules 7 and 8 lose their slots.
    let d = w.ship(me).unwrap().design().clone();
    let core = d.parts.iter().position(|p| p.logic).unwrap() as u32;
    w.apply_damage(me, DamageTarget::Part(core), 100.0);
    run(&mut w, 3);
    assert!(w.ship(me).unwrap().is_active());
    let st = state(&w, me);
    assert_eq!(st.slots(), 6);
    assert_eq!(st.rules()[6], RuleStatus::NoSlot);
    assert_eq!(st.rules()[7], RuleStatus::NoSlot);
    assert_eq!(
        st.active_rule(),
        None,
        "nothing within the slots matches: brake"
    );
    assert_eq!(st.maneuver(), Maneuver::Brake);
    assert!(st.describe(7).contains("6 rule slot"), "{}", st.describe(7));
}

#[test]
fn without_a_target_target_rules_fail() {
    let c = content();
    let mut w = world_with_lib(lib());
    let me = w.spawn_ship(design(&c, "halvex_runner"), Body::default());
    program(
        &mut w,
        me,
        "when target_hull < 50% -> face
         when not target_on_me -> face
         always -> approach(50m)
         always -> hold",
    );
    run(&mut w, 1);
    let st = state(&w, me);
    assert_eq!(st.target(), None);
    let no_target = RuleStatus::Failed {
        cond: 0,
        why: Why::NoTarget,
    };
    assert_eq!(st.rules()[0], no_target);
    assert_eq!(
        st.rules()[1],
        no_target,
        "`not` does not turn a missing target into true"
    );
    assert_eq!(st.rules()[2], RuleStatus::NoTarget);
    assert_eq!(st.active_rule(), Some(3));
    assert_eq!(st.maneuver(), Maneuver::Brake);
    // No shield emitters on a raider: `shield` has nothing to measure.
    let r = w.spawn_ship(
        design(&c, "automaton_raider"),
        Body::at(Vec2::new(0.0, 300.0), 0.0),
    );
    program(&mut w, r, "when shield < 50% -> hold\nalways -> face");
    run(&mut w, 1);
    assert_eq!(
        state(&w, r).rules()[0],
        RuleStatus::Failed {
            cond: 0,
            why: Why::NoValue
        }
    );
}

#[test]
fn target_selectors_priorities_and_stickiness() {
    let (mut w, me, rs) = duel(&[(0.0, 60.0), (0.0, -100.0)]);
    let (a, b) = (rs[0], rs[1]);
    // Wear b down: it is the weakest.
    let bd = w.ship(b).unwrap().design().clone();
    for (i, p) in bd.parts.iter().enumerate() {
        if p.hull {
            w.apply_damage(b, DamageTarget::Part(i as u32), p.hp * 0.6);
        }
    }
    let target = |w: &mut Sim, text: &str| {
        program(w, me, text);
        run(w, 1);
        (state(w, me).target(), state(w, me).aim_part())
    };
    assert_eq!(
        target(&mut w, "target nearest\nalways -> hold"),
        (Some(a), None)
    );
    assert_eq!(target(&mut w, "target weakest\nalways -> hold").0, Some(b));
    let (t, part) = target(&mut w, "target weakest(engines)\nalways -> hold");
    assert_eq!(t, Some(b));
    assert!(bd.parts[part.unwrap() as usize].is_engine);
    let (_, part) = target(&mut w, "target nearest(weapons)\nalways -> hold");
    assert!(bd.parts[part.unwrap() as usize].weapon.is_some());
    assert_eq!(target(&mut w, "target attacker\nalways -> hold").0, None);

    // b starts hunting the runner: now it is the attacker and, aiming at us, the bigger threat.
    w.command(b, ShipCommand::Brain(Some(RaiderBrain::default())));
    run(&mut w, 1);
    assert_eq!(target(&mut w, "target attacker\nalways -> hold").0, Some(b));
    assert_eq!(target(&mut w, "target threat\nalways -> hold").0, Some(b));
    w.command(b, ShipCommand::Brain(None));

    // Stickiness: the current target stays unless another is clearly nearer.
    let near = |w: &mut Sim, y: f32| {
        w.teleport(
            b,
            w.ship(b).unwrap().control_piece().unwrap(),
            Body::at(Vec2::new(0.0, y), 0.0),
        );
        run(w, 3);
        state(w, me).target()
    };
    program(&mut w, me, "target nearest\nalways -> hold");
    run(&mut w, 1);
    assert_eq!(state(&w, me).target(), Some(a));
    assert_eq!(
        near(&mut w, -55.0),
        Some(a),
        "55 m is not clearly nearer than 60 m"
    );
    assert_eq!(near(&mut w, -40.0), Some(b));

    // An escort follows its leader's lock.
    let c = content();
    let leader = w.spawn_ship(
        design(&c, "halvex_runner"),
        Body::at(Vec2::new(30.0, 0.0), 0.0),
    );
    w.command(
        leader,
        ShipCommand::Pilot(PilotInput {
            lock: Some(Target::Ship(a)),
            ..Default::default()
        }),
    );
    assert_eq!(
        target(&mut w, "target locked\nalways -> hold").0,
        None,
        "no leader"
    );
    w.command(me, ShipCommand::Leader(Some(leader)));
    assert_eq!(
        target(&mut w, "target locked(core)\nalways -> hold").0,
        Some(a)
    );
    let ad = w.ship(a).unwrap().design().clone();
    assert!(ad.parts[state(&w, me).aim_part().unwrap() as usize].logic);
}

/// The content raider (`raider_basic`) against the old hard-coded brain: same fight, similar numbers.
#[test]
fn content_raider_fights_like_the_raider_brain() {
    fn fight(use_behavior: bool) -> (Option<u64>, u32, u32, f32, f32) {
        let c = content();
        let mut w = world_with_lib(lib());
        let runner = w.spawn_ship(design(&c, "halvex_runner"), Body::at(Vec2::ZERO, 0.0));
        let mut raiders = Vec::new();
        for k in 0..3 {
            let x = (k as f32 - 1.0) * 20.0;
            let r = w.spawn_ship(
                design(&c, "automaton_raider"),
                Body::at(Vec2::new(x, 80.0), core::f32::consts::PI),
            );
            let cmd = if use_behavior {
                ShipCommand::Behavior(w.behaviors().default_for(w.ship(r).unwrap().design()))
            } else {
                ShipCommand::Brain(Some(RaiderBrain::default()))
            };
            w.command(r, cmd);
            raiders.push(r);
        }
        let (mut first, mut shots, mut hits) = (None, 0, 0);
        let (mut dist_sum, mut samples, mut orbit_ticks) = (0.0f32, 0u32, 0u32);
        for t in 0..1200u64 {
            w.step(&TickInputs::default());
            for e in w.events() {
                match e {
                    SimEvent::ShotFired { ship, .. } if raiders.contains(ship) => {
                        shots += 1;
                        first.get_or_insert(t);
                    }
                    SimEvent::Hit { ship, .. } if *ship == runner => hits += 1,
                    _ => {}
                }
            }
            let rp = w.ship(runner).unwrap();
            let rpos = rp.piece(rp.control_piece().unwrap()).unwrap().com_world();
            for &r in &raiders {
                let s = w.ship(r).unwrap();
                if let Some(p) = s.control_piece().and_then(|c| s.piece(c)) {
                    dist_sum += (p.com_world() - rpos).length();
                    samples += 1;
                }
                let orbiting = match (s.brain(), s.behavior()) {
                    (Some(b), _) => b.phase == sim::maneuver::RaiderPhase::Orbit,
                    (_, Some(b)) => matches!(b.maneuver(), Maneuver::Orbit { .. }),
                    _ => false,
                };
                orbit_ticks += orbiting as u32;
            }
        }
        (
            first,
            shots,
            hits,
            dist_sum / samples.max(1) as f32,
            orbit_ticks as f32 / samples.max(1) as f32,
        )
    }
    let brain = fight(false);
    let content = fight(true);
    println!(
        "brain   first {:?} shots {} hits {} mean dist {:.1} orbit share {:.2}",
        brain.0, brain.1, brain.2, brain.3, brain.4
    );
    println!(
        "content first {:?} shots {} hits {} mean dist {:.1} orbit share {:.2}",
        content.0, content.1, content.2, content.3, content.4
    );
    let (b_first, c_first) = (brain.0.unwrap() as f32, content.0.unwrap() as f32);
    assert!((b_first - c_first).abs() <= 15.0, "first shot");
    let near = |a: f32, b: f32, rel: f32| (a - b).abs() <= rel * a.max(b) + 2.0;
    assert!(near(brain.1 as f32, content.1 as f32, 0.25), "shots");
    assert!(near(brain.2 as f32, content.2 as f32, 0.35), "hits");
    assert!(near(brain.3, content.3, 0.1), "mean distance");
    assert!((brain.4 - content.4).abs() < 0.1, "share of time orbiting");
    assert!(content.1 > 0 && content.2 > 0, "the content raiders fight");
}

/// Content with extra Rhai conditions on top of the base pack.
fn scripted_lib() -> BehaviorLibrary {
    let base = data::base_pack().unwrap();
    let m = data::Pack::from_files([
        (
            "pack.ron",
            r#"(id: "m", name: "M", version: "1", schema_version: 1, depends: ["base"])"#,
        ),
        (
            "behavior_nodes/scripts.ron",
            r#"[
                (id: "hull_above", name: "Hull above", summary: "S.", role: Condition,
                 params: [(id: "limit", name: "Limit", type: Percent, default: "50%")],
                 script: "me.hull > limit"),
                (id: "spin", name: "Spin", summary: "Never ends.", role: Condition,
                 script: "let x = 0; loop { x += 1; } true"),
                (id: "not_bool", name: "Not bool", summary: "Returns a number.", role: Condition, script: "me.hull * 2.0"),
                (id: "bad_prop", name: "Bad prop", summary: "Unknown property.", role: Condition, script: "me.hul > 0.0"),
                (id: "clobber", name: "Clobber", summary: "Overwrites its inputs.", role: Condition,
                 params: [(id: "limit", name: "Limit", type: Percent, default: "50%")],
                 script: "let ok = me.hull > limit && target.exists; me = 0; target = \"x\"; limit = (); ok"),
                (id: "trig", name: "Trig", summary: "Deterministic math.", role: Condition,
                 script: "let a = atan2(1.0, 1.0); let one = sin(a) * sin(a) + cos(a) * cos(a); one > 0.999 && one < 1.001 && sqrt(4.0) == 2.0 && pi() > 3.14 && target.exists && target.distance > 0.0"),
            ]"#,
        ),
    ])
    .unwrap();
    let c = data::Content::load(&[base, m]).unwrap_or_else(|p| panic!("{p:?}"));
    BehaviorLibrary::new(&c).unwrap_or_else(|p| panic!("{p:?}"))
}

#[test]
fn rhai_conditions_run_sandboxed_within_budget() {
    let c = content();
    let mut w = world_with_lib(scripted_lib());
    let me = w.spawn_ship(design(&c, "halvex_runner"), Body::default());
    let _r = w.spawn_ship(
        design(&c, "automaton_raider"),
        Body::at(Vec2::new(0.0, 80.0), 0.0),
    );
    program(
        &mut w,
        me,
        "when spin -> hold
         when not_bool -> hold
         when bad_prop -> hold
         when hull_above(150%) -> hold
         when trig and hull_above(50%) and clobber -> face
         always -> hold",
    );
    let t0 = std::time::Instant::now();
    run(&mut w, 1);
    assert!(
        t0.elapsed().as_secs_f32() < 1.0,
        "the budget stops the endless loop"
    );
    let st = state(&w, me);
    let script = |f| RuleStatus::Failed {
        cond: 0,
        why: Why::Script(f),
    };
    assert_eq!(st.rules()[0], script(ScriptFault::OpLimit));
    assert_eq!(st.rules()[1], script(ScriptFault::NotBool));
    assert_eq!(st.rules()[2], script(ScriptFault::Error));
    assert_eq!(
        st.rules()[3],
        RuleStatus::Failed {
            cond: 0,
            why: Why::False { value: None }
        }
    );
    assert_eq!(st.active_rule(), Some(4));
    let msg = st.script_error().unwrap();
    assert!(msg.contains("bad_prop"), "{msg}");
    assert!(w.script_evaluations() >= 6);
    // A script that overwrote its inputs sees fresh ones at the next think.
    run(&mut w, 6);
    assert_eq!(state(&w, me).active_rule(), Some(4));
    assert_eq!(state(&w, me).rules()[4], RuleStatus::Active);
}

#[test]
fn behavior_scenario_is_deterministic() {
    use sim::scenario::{ScenarioKind, run_kind};
    let (a, stats) = run_kind(ScenarioKind::Behavior, 42, 900);
    let (b, _) = run_kind(ScenarioKind::Behavior, 42, 900);
    assert_eq!(a, b);
    let (c, _) = run_kind(ScenarioKind::Behavior, 43, 900);
    assert_ne!(a, c);
    assert!(stats.script_evals > 0, "the Rhai condition ran");
    assert!(stats.shots > 0 && stats.hits > 0, "{stats:?}");

    // A clone continues identically (library, scratch and Rhai scopes included).
    let content = content();
    let mut sc = sim::scenario::Scenario::with_kind(&content, 42, ScenarioKind::Behavior).unwrap();
    for _ in 0..300 {
        sc.step();
    }
    let mut w2 = sc.sim.clone();
    let mut w1 = sc.sim;
    for _ in 0..300 {
        w1.step(&TickInputs::default());
        w2.step(&TickInputs::default());
        assert_eq!(w1.state_hash(), w2.state_hash());
    }
}

#[test]
fn thinks_on_the_first_tick_then_every_period_staggered() {
    let (mut w, me, rs) = duel(&[(0.0, 80.0)]);
    let id = w.behaviors().id("raider_basic").unwrap();
    w.command(rs[0], ShipCommand::Behavior(Some(id)));
    program(&mut w, me, "always -> face");
    let period = sim::behavior::tuning::THINK_PERIOD;
    let mut thinks = Vec::new();
    for _ in 0..12 {
        let tick = w.tick();
        w.step(&TickInputs::default());
        if state(&w, me).last_think() == Some(tick) {
            thinks.push(tick);
        }
    }
    assert_eq!(thinks[0], 0, "first tick after the command");
    for &t in &thinks[1..] {
        assert_eq!((t + me.0 as u64) % period, 0);
    }
    assert_eq!(
        thinks.len(),
        1 + 12 / period as usize - usize::from((me.0 as u64).is_multiple_of(period))
    );
}

#[test]
fn brain_and_behavior_replace_each_other() {
    let (mut w, me, _) = duel(&[(0.0, 80.0)]);
    w.command(me, ShipCommand::Brain(Some(RaiderBrain::default())));
    program(&mut w, me, "always -> face");
    assert!(w.ship(me).unwrap().brain().is_none());
    w.command(me, ShipCommand::Brain(Some(RaiderBrain::default())));
    assert!(w.ship(me).unwrap().behavior().is_none());
    w.command(me, ShipCommand::Brain(None));
    w.command(me, ShipCommand::Behavior(Some(sim::BehaviorId(9999))));
    assert!(
        w.ship(me).unwrap().behavior().is_none(),
        "unknown ids are ignored"
    );
    program(&mut w, me, "always -> face");
    w.command(me, ShipCommand::Behavior(None));
    assert!(w.ship(me).unwrap().behavior().is_none());
}

#[test]
fn behavior_state_is_cheap_to_inherit() {
    let (mut w, me, rs) = duel(&[(0.0, 80.0)]);
    program(&mut w, me, "target nearest(engines)\nalways -> face");
    run(&mut w, 2);
    let st = state(&w, me);
    let child = st.inherit();
    assert!(std::sync::Arc::ptr_eq(child.program(), st.program()));
    assert_eq!(child.target(), Some(rs[0]));
    assert_eq!(child.aim_part(), st.aim_part());
    assert_eq!(child.last_think(), None, "thinks on its next tick");
    assert_eq!(
        w.ship(me)
            .unwrap()
            .rule_slots_of(w.ship(me).unwrap().control_piece().unwrap()),
        12
    );
}

#[test]
fn sim_and_library_are_send_and_sync() {
    fn check<T: Send + Sync + Clone>() {}
    check::<Sim>();
    check::<BehaviorLibrary>();
}

#[test]
fn player_text_errors_point_at_line_and_column() {
    let mut l = lib();
    let e = l
        .add_text(
            "mine",
            "target nearest\n\nwhen hull < 30% -> flee\nwhen sheild_down -> orbit(45m)\n",
        )
        .unwrap_err();
    assert_eq!((e[0].line, e[0].col), (4, 6));
    assert!(
        e[0].message.contains("did you mean `shield_down`?"),
        "{e:?}"
    );
    let e = l.add_text("mine", "when hull < 30% -> flee(").unwrap_err();
    assert_eq!((e[0].line, e[0].col), (1, 25));
    let id = l.add_text("mine", "always -> hold").unwrap();
    assert_eq!(l.id("mine"), Some(id));
    assert_eq!(l.get(id).unwrap().program.to_text(), "always -> hold\n");
}
