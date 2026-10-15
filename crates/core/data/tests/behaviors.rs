//! The behavior text format (parse, print, round trip, error columns) and the validation of behavior nodes,
//! behaviors and the rule-slot limit.

mod common;

use common::*;
use data::behavior::{
    Amount, CmpOp, LineRef, MAX_CONDITIONS, Program, Unit, Value, check_program, resolve_args,
};
use data::{Content, Pack, Problem, Severity};

fn parse(text: &str) -> Program {
    Program::parse(text).unwrap_or_else(|e| panic!("{text}: {e:?}"))
}

fn text_errors(text: &str) -> Vec<(u32, u32, String)> {
    match Program::parse(text) {
        Ok(_) => Vec::new(),
        Err(e) => e.into_iter().map(|e| (e.line, e.col, e.message)).collect(),
    }
}

#[test]
fn base_behaviors_round_trip_through_text_and_ron_form() {
    let c = content();
    assert!(c.behaviors.len() >= 2);
    for b in &c.behaviors {
        let p = b.program().unwrap();
        assert!(check_program(&p, &c.behavior_nodes).is_empty(), "{}", b.id);
        let text = p.to_text();
        let again = parse(&text);
        assert_eq!(again, p, "{}", b.id);
        assert_eq!(again.to_text(), text, "{}: print is stable", b.id);
        let (target, rules) = p.to_parts();
        assert_eq!(Program::from_parts(target.as_deref(), &rules).unwrap(), p);
        // The RON strings are already canonical.
        assert_eq!(rules, b.rules, "{}: write rules in canonical form", b.id);
        assert_eq!(target, b.target, "{}", b.id);
    }
}

#[test]
fn text_form_is_forgiving_and_prints_canonically() {
    let typed = "\n  Target Weakest(Engines)\n\nIf Hull≤25 %   →  Flee()   #  run!\nwhen not target_in_range(120%)and cycle(9 s)>=5s->orbit( 45 , CW )\nALWAYS -> approach(66.70%)\n";
    let p = parse(typed);
    assert_eq!(
        p.to_text(),
        "target weakest(engines)\nwhen hull <= 25% -> flee # run!\nwhen not target_in_range(120%) and cycle(9s) >= 5s -> orbit(45, cw)\nalways -> approach(66.7%)\n"
    );
    assert_eq!(parse(&p.to_text()), p);
    let r = &p.rules[1];
    assert!(r.when[0].not);
    assert_eq!(r.when[1].cmp.as_ref().unwrap().op, CmpOp::Ge);
    assert_eq!(
        r.when[1].call.args[0].value,
        Value::Number(9.0, Some(Unit::Seconds))
    );
    assert_eq!(
        p.rules[2].action.args[0].value,
        Value::Number(66.7, Some(Unit::Percent))
    );
}

/// Deterministic pseudo-random programs from the vocabulary: print → parse → equal, print → print stable.
#[test]
fn generated_programs_round_trip() {
    let c = content();
    let conds: Vec<&data::BehaviorNode> = c
        .behavior_nodes
        .iter()
        .filter(|n| n.role == data::NodeRole::Condition)
        .collect();
    let actions: Vec<&data::BehaviorNode> = c
        .behavior_nodes
        .iter()
        .filter(|n| n.role == data::NodeRole::Action)
        .collect();
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut rnd = |n: usize| -> usize {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % n.max(1)
    };
    let numbers = [0.0f32, 1.0, 2.5, 30.0, 45.0, 66.7, 100.0, 0.1, 1234.5];
    for _ in 0..300 {
        let mut lines = Vec::new();
        if rnd(2) == 0 {
            lines.push("target nearest(weapons)".to_string());
        }
        for _ in 0..1 + rnd(6) {
            let n_conds = rnd(4);
            let mut parts = Vec::new();
            for _ in 0..n_conds {
                let node = conds[rnd(conds.len())];
                let mut s = String::new();
                if rnd(3) == 0 {
                    s += "not ";
                }
                s += &node.id;
                if !node.params.is_empty() && rnd(2) == 0 {
                    s += &format!("({}m)", numbers[rnd(numbers.len())]);
                }
                if node.value.is_some() {
                    let op = ["<", "<=", ">", ">="][rnd(4)];
                    let unit = ["", "%", "s", "m"][rnd(4)];
                    s += &format!(" {op} {}{unit}", numbers[rnd(numbers.len())]);
                }
                parts.push(s);
            }
            let action = actions[rnd(actions.len())];
            let mut a = action.id.clone();
            if !action.params.is_empty() && rnd(2) == 0 {
                a += &format!("({}%)", numbers[rnd(numbers.len())]);
            }
            let note = if rnd(4) == 0 {
                " # a note, with (symbols) -> # and all"
            } else {
                ""
            };
            if parts.is_empty() {
                lines.push(format!("always -> {a}{note}"));
            } else {
                lines.push(format!("when {} -> {a}{note}", parts.join(" and ")));
            }
        }
        let text = lines.join("\n");
        let p = parse(&text);
        let printed = p.to_text();
        assert_eq!(printed.trim_end(), text, "canonical input prints unchanged");
        assert_eq!(parse(&printed), p);
        let (t, r) = p.to_parts();
        assert_eq!(Program::from_parts(t.as_deref(), &r).unwrap(), p);
    }
}

#[test]
fn syntax_errors_name_line_and_column() {
    let errs = text_errors(
        "target nearest\nwhen hull < 30% -> flee\nwhen hull < 30% flee\nalways -> hold",
    );
    assert_eq!(
        errs,
        vec![(3, 17, "expected `and` or `->`, found `flee`".to_string())]
    );

    let errs = text_errors("when hull < 30% or shield_down -> flee");
    assert_eq!(errs[0].1, 17);
    assert!(errs[0].2.contains("no `or`"), "{errs:?}");

    let errs = text_errors("when hull < 30% -> orbit(45m, cw");
    assert_eq!((errs[0].1, errs[0].2.as_str()), (33, "missing `)`"));

    let errs = text_errors("when hull < 3O% -> flee");
    assert_eq!(errs[0].1, 14, "{errs:?}");

    let errs = text_errors("when hull < .5 -> flee");
    assert!(errs[0].2.contains("leading digit"), "{errs:?}");

    let errs = text_errors("# my behavior\nalways -> hold");
    assert_eq!((errs[0].0, errs[0].1), (1, 1));
    assert!(errs[0].2.contains("end of a rule"), "{errs:?}");

    let errs = text_errors("always -> hold\ntarget nearest");
    assert!(errs[0].2.contains("before the rules"), "{errs:?}");

    let errs = text_errors("when hull < 30% and and -> flee");
    assert!(errs[0].2.contains("keyword `and`"), "{errs:?}");

    let errs = text_errors("when hull < 30% -> flee -> hold");
    assert_eq!(errs[0].1, 25);

    let long = format!(
        "when {} -> hold",
        ["shield_down"; MAX_CONDITIONS + 1].join(" and ")
    );
    assert!(text_errors(&long)[0].2.contains("at most"));

    // Phone autocorrect: arrows and comparison signs are accepted; a unit typo is not.
    assert!(text_errors("when hull ≥ 30% → hold").is_empty());
    // A spaced word after a number is the next token, not a unit.
    let errs = text_errors("when target_in_range(60 km) -> hold");
    assert_eq!(errs[0].1, 25, "{errs:?}");
    assert!(
        errs[0].2.contains("expected `,` or `)`, found `km`"),
        "{errs:?}"
    );
    let errs = text_errors("when target_in_range(60km) -> hold");
    assert!(errs[0].2.contains("unknown unit `km`"), "{errs:?}");
}

#[test]
fn vocabulary_errors_point_at_the_value() {
    let c = content();
    let (p, map) = Program::parse_with_lines(
        "target weakest\n\nwhen hull < 30 -> flee\nwhen enemies_near(5s) >= 2 -> orbit(45m, sideways)\nwhen shield_down < 1 -> kite(20000m)\nalways -> fle",
    )
    .unwrap();
    let errs: Vec<(u32, u32, String)> = check_program(&p, &c.behavior_nodes)
        .into_iter()
        .map(|e| map.to_text_error(e))
        .map(|e| (e.line, e.col, e.message))
        .collect();
    let has = |line: u32, col: u32, needle: &str| {
        assert!(
            errs.iter()
                .any(|e| e.0 == line && e.1 == col && e.2.contains(needle)),
            "expected line {line} col {col} {needle:?} in {errs:#?}"
        )
    };
    has(3, 13, "write a percentage with `%`");
    has(4, 19, "a distance cannot be in `s`");
    has(
        4,
        42,
        "`sideways` is not an option; choose one of: auto, ccw, cw",
    );
    has(5, 20, "cannot be compared");
    has(5, 30, "at most 5000m");
    has(6, 11, "unknown action `fle`; did you mean `flee`?");
    assert_eq!(errs.len(), 6, "{errs:#?}");
}

#[test]
fn defaults_fill_missing_arguments() {
    let c = content();
    let p = parse("always -> orbit(45m)\nalways -> orbit\nwhen target_in_range -> approach(10)");
    let orbit = c.behavior_nodes.get("orbit").unwrap();
    assert_eq!(
        resolve_args(&p.rules[0].action, orbit).unwrap(),
        vec![Amount::Metres(45.0), Amount::Choice(0)]
    );
    assert_eq!(
        resolve_args(&p.rules[1].action, orbit).unwrap(),
        vec![Amount::Reach(0.6), Amount::Choice(0)]
    );
    let in_range = c.behavior_nodes.get("target_in_range").unwrap();
    assert_eq!(
        resolve_args(&p.rules[2].when[0].call, in_range).unwrap(),
        vec![Amount::Reach(1.0)]
    );
}

// ---- validation in packs ----

const MOD_MANIFEST: &str =
    r#"(id: "test_mod", name: "Test mod", version: "1.0.0", schema_version: 1, depends: ["base"])"#;

fn errors_with(files: &[(&str, &str)]) -> Vec<Problem> {
    let mut all = vec![("pack.ron", MOD_MANIFEST)];
    all.extend_from_slice(files);
    let problems = match Pack::from_files(all) {
        Err(p) => p,
        Ok(m) => match Content::load(&[data::base_pack().unwrap(), m]) {
            Ok(_) => Vec::new(),
            Err(p) => p,
        },
    };
    problems
        .into_iter()
        .filter(|p| p.severity == Severity::Error)
        .collect()
}

fn assert_error(problems: &[Problem], id: &str, field: &str, needle: &str) {
    assert!(
        problems.iter().any(|p| p.id.as_deref() == Some(id)
            && p.field.as_deref() == Some(field)
            && p.message.contains(needle)),
        "expected an error for `{id}` field `{field}` containing {needle:?}, got:\n{}",
        show(problems)
    );
}

#[test]
fn behavior_errors_name_the_rule_and_column() {
    let p = errors_with(&[(
        "behaviors/b.ron",
        r#"(id: "bad", name: "Bad", summary: "Bad.", target: "nearst", rules: [
            "when hul < 30% -> flee",
            "always -> orbit(45m, cw, 3)",
            "when hull < 30% -> flee(",
        ])"#,
    )]);
    // Syntax errors stop the vocabulary check of that behavior.
    assert_error(&p, "bad", "rules[2]", "column 25: missing `)`");
    assert_eq!(p.len(), 1, "{}", show(&p));
    let p = errors_with(&[(
        "behaviors/b.ron",
        r#"(id: "bad", name: "Bad", summary: "Bad.", target: "nearst", rules: [
            "when hul < 30% -> flee",
            "always -> orbit(45m, cw, 3)",
        ])"#,
    )]);
    assert_error(
        &p,
        "bad",
        "target",
        "column 1: unknown target selector `nearst`; did you mean `nearest`?",
    );
    assert_error(
        &p,
        "bad",
        "rules[0]",
        "column 6: unknown condition `hul`; did you mean `hull`?",
    );
    assert_error(
        &p,
        "bad",
        "rules[1]",
        "column 26: `orbit` takes at most 2 value(s): radius, dir",
    );
    let p = errors_with(&[(
        "behaviors/b.ron",
        r#"(id: "empty", name: "E", summary: "E.", rules: [])"#,
    )]);
    assert_error(&p, "empty", "rules", "at least one rule");
}

#[test]
fn unknown_behavior_references_are_errors() {
    let p = errors_with(&[(
        "factions/f.ron",
        r#"(id: "pirates", name: "Pirates", summary: "Arr.", glow: "enemy", behavior: "raider_basc")"#,
    )]);
    assert_error(
        &p,
        "pirates",
        "behavior",
        "unknown behavior `raider_basc`; did you mean `raider_basic`?",
    );
}

#[test]
fn too_many_rules_for_the_slots() {
    // The raider has one automaton core (2 slots); give automatons a 3-rule default.
    let p = errors_with(&[
        (
            "behaviors/b.ron",
            r#"(id: "long", name: "Long", summary: "Long.", rules: [
                "when hull < 30% -> flee", "when shield_down -> hold", "always -> approach"])"#,
        ),
        (
            "factions/automatons.ron",
            r#"(id: "automatons", name: "Rogue automatons", summary: "Drones.", glow: "enemy", behavior: "long")"#,
        ),
    ]);
    assert_error(
        &p,
        "automaton_raider",
        "faction",
        "its faction's default behavior `long` has 3 rules, but the ship's logic cores give only 2 rule slots",
    );
    // Ships with more cores are fine.
    assert!(
        !p.iter().any(|x| x.id.as_deref() == Some("halvex_runner")),
        "{}",
        show(&p)
    );
    let c = content();
    assert_eq!(
        c.ship_rule_slots(c.ships.get("automaton_raider").unwrap()),
        2
    );
    assert_eq!(c.ship_rule_slots(c.ships.get("halvex_runner").unwrap()), 12);
}

/// A ship checked as it flies a program of its own (an arena entry, an evolved genome, GDR-0092): its rule slots
/// count that program's rules, not its faction's default behavior's.
#[test]
fn a_ship_flying_its_own_program_counts_its_rules() {
    let c = content();
    let raider = c.ships.get("automaton_raider").unwrap();
    let errors = |text: &str| -> Vec<String> {
        let program = data::behavior::Program::parse(text).unwrap();
        data::validate_ship_flying(&c, raider, &program)
            .into_iter()
            .filter(|p| p.is_error())
            .map(|p| format!("{}: {}", p.field.as_deref().unwrap_or_default(), p.message))
            .collect()
    };
    assert!(errors("always -> approach\nwhen hull < 30% -> flee\n").is_empty());
    assert_eq!(
        errors("when hull < 30% -> flee\nwhen shield_down -> hold\nalways -> approach\n"),
        ["behavior: its behavior has 3 rules, but the ship's logic cores give only 2 rule slots"]
    );
}

#[test]
fn node_definitions_are_checked() {
    let p = errors_with(&[(
        "behavior_nodes/n.ron",
        r#"[
            (id: "and", name: "And", summary: "Bad id.", role: Condition),
            (id: "spin", name: "Spin", summary: "Bad params.", role: Action, value: Percent, params: [
                (id: "rate", name: "Rate", type: Percent, default: "30"),
                (id: "side", name: "Side", type: Choice([(id: "left", name: "Left"), (id: "left", name: "Left")])),
                (id: "far", name: "Far", type: Distance, default: "9000m", min: 0, max: 100),
            ]),
            (id: "lua", name: "Lua", summary: "Script on an action.", role: Action, script: "true"),
        ]"#,
    )]);
    assert_error(&p, "and", "id", "keyword");
    assert_error(&p, "spin", "value", "only conditions yield a value");
    assert_error(
        &p,
        "spin",
        "params[0].default",
        "write a percentage with `%`",
    );
    assert_error(&p, "spin", "params[1].type[1].id", "listed twice");
    assert_error(&p, "spin", "params[1].default", "must come before");
    assert_error(&p, "spin", "params[2].default", "at most 100m");
    assert_error(&p, "lua", "script", "only conditions can be scripted");
}

#[test]
fn param_texts_are_keyed() {
    let c = content();
    let texts = c.texts();
    assert_eq!(texts["behavior_node.orbit.params.radius.name"], "Radius");
    assert_eq!(
        texts["behavior_node.orbit.params.dir.options.cw.name"],
        "Clockwise"
    );
    assert_eq!(texts["behavior.raider_basic.name"], "Raider pack");
}

#[test]
fn base_ships_fly_their_faction_default() {
    let c = content();
    let raider = c.ships.get("automaton_raider").unwrap();
    assert_eq!(c.ship_behavior(raider).unwrap().id, "raider_basic");
    let runner = c.ships.get("halvex_runner").unwrap();
    assert_eq!(c.ship_behavior(runner).unwrap().id, "skirmisher");
    let _ = LineRef::Whole;
}
