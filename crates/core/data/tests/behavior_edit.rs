//! The behavior editing model (`data::behavior::edit`): every operation and refusal, the text round trip of the
//! content behaviors, ids kept across text edits, rule slots, undo/redo and strokes, and random edit sequences
//! that must keep the program legal and equal to its own text parsed again.

use data::behavior::{
    CmpOp, MAX_CONDITIONS, MAX_LINE_CHARS, MAX_RULES, Program, Unit, Value, check_program,
};
use data::behavior::edit::{BehaviorEditor, CondId, DEFAULT_TARGET, NodeRef, Refusal, RuleId};
use data::design::edit::UNDO_STEPS;
use data::{Content, NodeParam, NodeRole, ParamType};

fn content() -> Content {
    data::base_content()
}

fn editor(c: &Content, text: &str) -> BehaviorEditor {
    BehaviorEditor::from_text(&c.behavior_nodes, text).unwrap_or_else(|e| panic!("{text}: {e:?}"))
}

fn template(c: &Content, id: &str) -> BehaviorEditor {
    let program = c.behaviors.get(id).unwrap().program().unwrap();
    BehaviorEditor::open(&c.behavior_nodes, &program).unwrap()
}

/// The program is legal, its text gives it back (model → text → model), and setting its own text changes
/// nothing.
fn assert_round_trip(ed: &BehaviorEditor) {
    let text = ed.text();
    let again =
        BehaviorEditor::from_text(ed.nodes(), &text).unwrap_or_else(|e| panic!("{text}: {e:?}"));
    assert_eq!(again.program(), ed.program(), "{text}");
    assert_eq!(again.text(), text);
    assert!(check_program(ed.program(), ed.nodes()).is_empty());
    let mut same = ed.clone();
    assert_eq!(same.set_text(&text), Ok(false));
    assert_eq!(same.revision(), ed.revision());
    assert_eq!(ids(&same), ids(ed));
}

fn num(x: f32, unit: Option<Unit>) -> Value {
    Value::Number(x, unit)
}

fn pct(x: f32) -> Value {
    num(x, Some(Unit::Percent))
}

fn metres(x: f32) -> Value {
    num(x, Some(Unit::Metres))
}

fn word(w: &str) -> Value {
    Value::Word(w.into())
}

fn ids(ed: &BehaviorEditor) -> Vec<RuleId> {
    ed.rules().map(|r| r.id).collect()
}

fn cond_ids(ed: &BehaviorEditor, k: usize) -> Vec<CondId> {
    ed.rule_at(k).unwrap().conds.to_vec()
}

fn line(ed: &BehaviorEditor, k: usize) -> String {
    ed.rule_at(k).unwrap().rule.to_string()
}

fn bad_value<T: std::fmt::Debug>(r: Result<T, Refusal>, needle: &str) {
    match r {
        Err(Refusal::BadValue(m)) => assert!(m.contains(needle), "{m:?} lacks {needle:?}"),
        other => panic!("expected BadValue({needle:?}), got {other:?}"),
    }
}

#[test]
fn content_behaviors_open_and_print_canonically() {
    let c = content();
    for id in ["raider_basic", "skirmisher"] {
        assert!(c.behaviors.contains(id), "{id}");
    }
    for b in &c.behaviors {
        let ed = template(&c, &b.id);
        let (target, rules) = ed.program().to_parts();
        assert_eq!(target, b.target, "{}", b.id);
        assert_eq!(rules, b.rules, "{}", b.id);
        assert_eq!(ed.revision(), 0);
        assert!(!ed.can_undo() && !ed.can_redo());
        assert_round_trip(&ed);
    }
}

#[test]
fn opening_refuses_illegal_programs_with_positions() {
    let c = content();
    let errors = BehaviorEditor::from_text(
        &c.behavior_nodes,
        "target nearest\nwhen hull < 30 -> flee\nwhen sheild_down -> flee\n",
    )
    .unwrap_err();
    let at: Vec<(u32, u32)> = errors.iter().map(|e| (e.line, e.col)).collect();
    assert_eq!(at, vec![(2, 13), (3, 6)]);
    assert!(
        errors[1].message.contains("did you mean `shield_down`"),
        "{errors:?}"
    );
    let errors = BehaviorEditor::from_text(&c.behavior_nodes, "\n").unwrap_err();
    assert!(errors[0].message.contains("at least one rule"));
    // A program built in code goes through its text: a note with a line break does not survive it.
    let mut p = Program::parse("always -> flee").unwrap();
    p.rules[0].note = Some("two\nlines".into());
    let errors = BehaviorEditor::open(&c.behavior_nodes, &p).unwrap_err();
    assert_eq!(errors[0].line, 2);
}

#[test]
fn rules_add_duplicate_move_and_delete() {
    let c = content();
    let mut ed = editor(&c, "always -> approach(67%)\n");
    let base = ids(&ed)[0];
    let top = ed.add_rule(0, "flee").unwrap();
    assert_eq!(ed.text(), "always -> flee\nalways -> approach(67%)\n");
    let bottom = ed.add_rule(2, "orbit").unwrap();
    assert_eq!(line(&ed, 2), "always -> orbit", "defaults stay implicit");
    assert_eq!(ed.add_rule(4, "hold"), Err(Refusal::BadIndex(4)));
    assert_eq!(
        ed.add_rule(0, "flea"),
        Err(Refusal::UnknownNode("flea".into()))
    );
    assert_eq!(
        ed.add_rule(0, "hull"),
        Err(Refusal::WrongRole {
            node: "hull".into(),
            expected: NodeRole::Action
        })
    );
    assert_eq!(
        Refusal::WrongRole {
            node: "hull".into(),
            expected: NodeRole::Action
        }
        .to_string(),
        "`hull` is not an action"
    );

    ed.set_rule_note(base, Some("main")).unwrap();
    let copy = ed.duplicate_rule(base).unwrap();
    assert_eq!(ids(&ed), vec![top, base, copy, bottom]);
    assert_eq!(line(&ed, 2), "always -> approach(67%) # main");

    assert_eq!(ed.move_rule(bottom, 0), Ok(true));
    assert_eq!(ids(&ed), vec![bottom, top, base, copy]);
    assert_eq!(ed.move_rule(bottom, 0), Ok(false));
    assert_eq!(ed.move_rule(top, 4), Err(Refusal::BadIndex(4)));
    assert_eq!(ed.move_rule(top, 3), Ok(true));
    assert_eq!(ids(&ed), vec![bottom, base, copy, top]);
    assert_eq!(ed.rule(top).unwrap().index, 3);

    assert_eq!(ed.delete_rule(copy), Ok(true));
    assert_eq!(ed.delete_rule(copy), Err(Refusal::NoSuchRule(copy)));
    assert!(ed.rule(copy).is_none());
    assert!(ed.node(NodeRef::Action(copy)).is_none());
    assert_eq!(ed.move_rule(copy, 0), Err(Refusal::NoSuchRule(copy)));
    ed.delete_rule(bottom).unwrap();
    ed.delete_rule(base).unwrap();
    assert_eq!(ed.delete_rule(top), Err(Refusal::LastRule));
    assert_eq!(ed.text(), "always -> flee\n");
    assert_round_trip(&ed);
}

#[test]
fn at_most_64_rules() {
    let c = content();
    let mut ed = editor(&c, "always -> hold");
    while ed.rules().len() < MAX_RULES {
        ed.add_rule(0, "hold").unwrap();
    }
    assert_eq!(ed.add_rule(0, "hold"), Err(Refusal::TooManyRules));
    let first = ids(&ed)[0];
    assert_eq!(ed.duplicate_rule(first), Err(Refusal::TooManyRules));
    assert_round_trip(&ed);
}

#[test]
fn notes_are_one_trimmed_line() {
    let c = content();
    let mut ed = editor(&c, "target weakest\nalways -> hold\n");
    let r = ids(&ed)[0];
    assert_eq!(ed.set_rule_note(r, Some("  stay put  ")), Ok(true));
    assert_eq!(line(&ed, 0), "always -> hold # stay put");
    assert_eq!(ed.set_rule_note(r, Some("stay put")), Ok(false));
    assert_eq!(ed.set_rule_note(r, Some("a # b -> c (d)")), Ok(true));
    assert_round_trip(&ed);
    assert_eq!(
        ed.set_rule_note(r, Some("two\nlines")),
        Err(Refusal::BadNote)
    );
    assert_eq!(ed.set_rule_note(r, Some("a\tb")), Err(Refusal::BadNote));
    let room = MAX_LINE_CHARS - "always -> hold # ".len();
    assert_eq!(
        ed.set_rule_note(r, Some(&"n".repeat(room + 1))),
        Err(Refusal::LineTooLong)
    );
    assert_eq!(ed.set_rule_note(r, Some(&"n".repeat(room))), Ok(true));
    assert_eq!(line(&ed, 0).chars().count(), MAX_LINE_CHARS);
    assert_round_trip(&ed);
    assert_eq!(ed.set_rule_note(r, Some("   ")), Ok(true));
    assert_eq!(ed.program().rules[0].note, None);
    assert_eq!(ed.set_rule_note(r, None), Ok(false));

    assert_eq!(ed.set_target_note(Some("the weak first")), Ok(true));
    assert_eq!(
        ed.text(),
        "target weakest # the weak first\nalways -> hold\n"
    );
    assert!(ed.clear_target());
    assert!(!ed.clear_target());
    assert_eq!(ed.set_target_note(None), Ok(false), "nothing to clear");
    assert_eq!(ed.program().target, None);
    assert_eq!(ed.set_target_note(Some("x")), Ok(true));
    assert_eq!(ed.text(), "target nearest # x\nalways -> hold\n");
    assert_round_trip(&ed);
}

#[test]
fn conditions_add_remove_negate_and_compare() {
    let c = content();
    let mut ed = editor(&c, "always -> flee");
    let r = ids(&ed)[0];
    let hull = ed.add_condition(r, "hull").unwrap();
    assert_eq!(line(&ed, 0), "when hull < 30% -> flee");
    let range = ed.add_condition(r, "target_in_range").unwrap();
    let near = ed.add_condition(r, "enemies_near").unwrap();
    let cycle = ed.add_condition(r, "cycle").unwrap();
    assert_eq!(
        line(&ed, 0),
        "when hull < 30% and target_in_range and enemies_near < 2 and cycle < 5s -> flee"
    );
    assert_eq!(cond_ids(&ed, 0), vec![hull, range, near, cycle]);
    assert_eq!(ed.rule_of(cycle), Some(r));
    assert_eq!(
        ed.add_condition(r, "flee"),
        Err(Refusal::WrongRole {
            node: "flee".into(),
            expected: NodeRole::Condition
        })
    );
    assert_eq!(
        ed.add_condition(r, "hul"),
        Err(Refusal::UnknownNode("hul".into()))
    );

    assert_eq!(ed.toggle_not(range), Ok(true));
    assert_eq!(ed.set_compare(hull, CmpOp::Le, pct(25.0)), Ok(true));
    assert_eq!(ed.set_compare(hull, CmpOp::Le, pct(25.0)), Ok(false));
    assert_eq!(ed.set_compare(near, CmpOp::Ge, num(3.0, None)), Ok(true));
    assert_eq!(
        line(&ed, 0),
        "when hull <= 25% and not target_in_range and enemies_near >= 3 and cycle < 5s -> flee"
    );
    bad_value(ed.set_compare(hull, CmpOp::Lt, num(25.0, None)), "%");
    bad_value(ed.set_compare(hull, CmpOp::Lt, pct(-5.0)), "at least 0");
    bad_value(
        ed.set_compare(hull, CmpOp::Lt, pct(f32::NAN)),
        "not a number",
    );
    bad_value(
        ed.set_compare(near, CmpOp::Ge, num(2.5, None)),
        "whole number",
    );
    bad_value(ed.set_compare(near, CmpOp::Ge, word("many")), "count");
    bad_value(ed.set_compare(cycle, CmpOp::Ge, pct(50.0)), "%");
    assert_eq!(
        ed.set_compare(range, CmpOp::Lt, pct(5.0)),
        Err(Refusal::NotAValue("target_in_range".into()))
    );
    assert_round_trip(&ed);

    assert_eq!(ed.remove_condition(range), Ok(true));
    assert_eq!(
        ed.remove_condition(range),
        Err(Refusal::NoSuchCondition(range))
    );
    assert_eq!(ed.toggle_not(range), Err(Refusal::NoSuchCondition(range)));
    assert!(ed.node(NodeRef::Cond(range)).is_none());
    assert_eq!(cond_ids(&ed, 0), vec![hull, near, cycle]);
    for k in [near, hull, cycle] {
        ed.remove_condition(k).unwrap();
    }
    assert_eq!(ed.text(), "always -> flee\n");
}

#[test]
fn condition_and_line_limits() {
    let c = content();
    let mut ed = editor(&c, "always -> hold");
    let r = ids(&ed)[0];
    for _ in 0..MAX_CONDITIONS {
        ed.add_condition(r, "shield_down").unwrap();
    }
    assert_eq!(
        ed.add_condition(r, "shield_down"),
        Err(Refusal::TooManyConditions)
    );
    assert_round_trip(&ed);

    let mut ed = editor(&c, "always -> hold");
    let r = ids(&ed)[0];
    let room = MAX_LINE_CHARS - "when under_attack -> hold # ".len();
    ed.set_rule_note(r, Some(&"n".repeat(room))).unwrap();
    let cond = ed.add_condition(r, "under_attack").unwrap();
    assert_eq!(line(&ed, 0).chars().count(), MAX_LINE_CHARS);
    assert_eq!(ed.toggle_not(cond), Err(Refusal::LineTooLong));
    assert_eq!(
        ed.add_condition(r, "shield_down"),
        Err(Refusal::LineTooLong)
    );
    assert_eq!(
        ed.replace_node(NodeRef::Cond(cond), "target_in_range"),
        Err(Refusal::LineTooLong)
    );
    assert_eq!(
        ed.set_param(NodeRef::Action(r), 0, word("x")),
        Err(Refusal::NoSuchParam {
            node: "hold".into(),
            index: 0
        })
    );
    assert_round_trip(&ed);
}

#[test]
fn replacing_a_node_keeps_what_fits() {
    let c = content();
    let mut ed = editor(
        &c,
        "target weakest(engines) # slow them\nwhen hull < 30% and not shield_down -> orbit(80%, ccw)\n",
    );
    let r = ids(&ed)[0];
    let conds = cond_ids(&ed, 0);
    let action = NodeRef::Action(r);
    let action_text = |ed: &BehaviorEditor| ed.program().rules[0].action.to_string();

    assert_eq!(ed.replace_node(action, "kite"), Ok(true));
    assert_eq!(action_text(&ed), "kite(80%)", "the radius fits as a range");
    assert_eq!(ed.replace_node(action, "flee"), Ok(true));
    assert_eq!(action_text(&ed), "flee");
    assert_eq!(ed.replace_node(action, "flee"), Ok(false));
    ed.replace_node(action, "approach").unwrap();
    ed.set_param(action, 0, metres(0.0)).unwrap();
    ed.replace_node(action, "orbit").unwrap();
    assert_eq!(
        action_text(&ed),
        "orbit",
        "0m is below orbit's minimum radius"
    );
    assert_eq!(
        ed.replace_node(action, "hull"),
        Err(Refusal::WrongRole {
            node: "hull".into(),
            expected: NodeRole::Action
        })
    );

    let cond = |k: usize| NodeRef::Cond(conds[k]);
    ed.replace_node(cond(0), "target_hull").unwrap();
    assert!(line(&ed, 0).starts_with("when target_hull < 30% and"));
    ed.set_compare(conds[0], CmpOp::Ge, pct(50.0)).unwrap();
    ed.replace_node(cond(0), "enemies_near").unwrap();
    assert!(
        line(&ed, 0).starts_with("when enemies_near >= 2 and"),
        "comparator kept, threshold reset: {}",
        line(&ed, 0)
    );
    ed.replace_node(cond(0), "under_attack").unwrap();
    ed.replace_node(cond(1), "heat").unwrap();
    assert_eq!(
        line(&ed, 0),
        "when under_attack and not heat < 30% -> orbit"
    );
    assert_eq!(cond_ids(&ed, 0), conds, "replacing keeps the ids");
    assert_eq!(
        ed.replace_node(cond(0), "nearest"),
        Err(Refusal::WrongRole {
            node: "nearest".into(),
            expected: NodeRole::Condition
        })
    );

    assert_eq!(ed.replace_node(NodeRef::Target, "threat"), Ok(true));
    assert!(
        ed.text()
            .starts_with("target threat(engines) # slow them\n")
    );
    assert_eq!(
        ed.replace_node(NodeRef::Target, "approach"),
        Err(Refusal::WrongRole {
            node: "approach".into(),
            expected: NodeRole::Target
        })
    );
    assert_round_trip(&ed);
}

#[test]
fn parameters_are_checked_and_reset() {
    let c = content();
    let mut ed = editor(&c, "always -> orbit\n");
    let a = NodeRef::Action(ids(&ed)[0]);
    assert_eq!(ed.set_param(a, 1, word("cw")), Ok(true));
    assert_eq!(
        line(&ed, 0),
        "always -> orbit(60%, cw)",
        "earlier defaults written out"
    );
    assert_eq!(ed.set_param(a, 1, word("cw")), Ok(false));
    assert_eq!(ed.set_param(a, 0, metres(45.0)), Ok(true));
    assert_eq!(line(&ed, 0), "always -> orbit(45m, cw)");

    bad_value(ed.set_param(a, 0, metres(0.5)), "at least 1");
    bad_value(ed.set_param(a, 0, metres(6000.0)), "at most 5000");
    bad_value(ed.set_param(a, 0, pct(1001.0)), "1000%");
    bad_value(ed.set_param(a, 0, num(f32::INFINITY, None)), "not a number");
    bad_value(ed.set_param(a, 0, word("far")), "distance");
    bad_value(ed.set_param(a, 0, num(5.0, Some(Unit::Seconds))), "`s`");
    bad_value(ed.set_param(a, 1, word("sideways")), "choose one of");
    bad_value(ed.set_param(a, 1, num(1.0, None)), "one of");
    assert_eq!(
        ed.set_param(a, 2, word("cw")),
        Err(Refusal::NoSuchParam {
            node: "orbit".into(),
            index: 2
        })
    );
    assert_eq!(ed.set_param(a, 0, pct(1000.0)), Ok(true));
    assert_eq!(line(&ed, 0), "always -> orbit(1000%, cw)");

    assert_eq!(ed.reset_param(a, 0), Ok(true));
    assert_eq!(
        line(&ed, 0),
        "always -> orbit(60%, cw)",
        "a value before others is written"
    );
    assert_eq!(ed.reset_param(a, 1), Ok(true));
    assert_eq!(
        line(&ed, 0),
        "always -> orbit",
        "placeholder defaults go too"
    );
    assert_eq!(ed.reset_param(a, 1), Ok(false));
    assert_eq!(ed.set_param(a, 0, pct(60.0)), Ok(true));
    assert_eq!(
        line(&ed, 0),
        "always -> orbit(60%)",
        "a value set is written, even the default"
    );
    assert_eq!(ed.reset_param(a, 0), Ok(true));
    assert_eq!(line(&ed, 0), "always -> orbit");
    assert!(matches!(
        ed.reset_param(a, 2),
        Err(Refusal::NoSuchParam { index: 2, .. })
    ));

    // Condition parameters; bare metres stay as written and equal the same value in `m`.
    let mut ed = editor(&c, "when target_in_range -> hold\n");
    let range = NodeRef::Cond(cond_ids(&ed, 0)[0]);
    assert_eq!(ed.set_param(range, 0, num(150.0, None)), Ok(true));
    assert_eq!(line(&ed, 0), "when target_in_range(150) -> hold");
    let view = ed.node(range).unwrap();
    assert_eq!(view.params[0].value, num(150.0, None));
    assert!(view.params[0].written && !view.params[0].is_default);
    assert_eq!(view.params[0].range(), (Some(0.0), Some(5000.0)));
    assert_eq!(ed.set_param(range, 0, metres(150.0)), Ok(true));
    assert_eq!(line(&ed, 0), "when target_in_range(150m) -> hold");
    assert_round_trip(&ed);
}

#[test]
fn the_target_line() {
    let c = content();
    let mut ed = editor(&c, "always -> approach\n");
    let t = ed.node(NodeRef::Target).unwrap();
    assert_eq!(t.node.id, DEFAULT_TARGET);
    assert!(!t.written);
    assert_eq!(t.params[0].value, word("centre"));
    assert!(!t.params[0].written && t.params[0].is_default);
    assert_eq!(t.params[0].range(), (None, None));

    assert_eq!(ed.replace_node(NodeRef::Target, DEFAULT_TARGET), Ok(false));
    assert_eq!(ed.reset_param(NodeRef::Target, 0), Ok(false));
    assert_eq!(ed.program().target, None, "the default stays implicit");
    assert_eq!(ed.revision(), 0);

    assert_eq!(ed.set_param(NodeRef::Target, 0, word("engines")), Ok(true));
    assert_eq!(ed.text(), "target nearest(engines)\nalways -> approach\n");
    assert!(ed.node(NodeRef::Target).unwrap().written);
    ed.replace_node(NodeRef::Target, "weakest").unwrap();
    assert_eq!(ed.text(), "target weakest(engines)\nalways -> approach\n");
    bad_value(
        ed.set_param(NodeRef::Target, 0, word("bridge")),
        "choose one of",
    );
    ed.reset_param(NodeRef::Target, 0).unwrap();
    assert_eq!(ed.text(), "target weakest\nalways -> approach\n");
    assert!(ed.clear_target());
    assert_eq!(ed.text(), "always -> approach\n");
    assert_round_trip(&ed);
}

#[test]
fn inspector_and_palette_queries() {
    let c = content();
    let ed = template(&c, "skirmisher");
    let rules: Vec<_> = ed.rules().collect();
    assert_eq!(rules.len(), 4);

    let closing = ed.node(NodeRef::Cond(rules[2].conds[0])).unwrap();
    assert_eq!(closing.node.id, "closing_in");
    assert_eq!(closing.node.name, "Closing in");
    assert_eq!(closing.node.role, NodeRole::Condition);
    assert!(closing.is_rhai());
    assert!(!closing.not && closing.compare.is_none());
    let time = &closing.params[0];
    assert_eq!(time.param.id, "time");
    assert_eq!(time.value, num(2.0, Some(Unit::Seconds)));
    assert!(time.written && !time.is_default);
    assert_eq!(time.range(), (Some(0.0), Some(60.0)));

    let in_range = ed.node(NodeRef::Cond(rules[2].conds[1])).unwrap();
    assert!(in_range.not && !in_range.is_rhai());
    assert_eq!(in_range.params[0].value, pct(100.0));
    assert!(in_range.params[0].is_default, "100% is the default");
    assert_eq!(
        in_range.params[0].range(),
        (Some(0.0), Some(1000.0)),
        "reach share"
    );

    let hull = ed.node(NodeRef::Cond(rules[0].conds[0])).unwrap();
    let cmp = hull.compare.unwrap();
    assert_eq!((cmp.op, &cmp.rhs.value), (CmpOp::Lt, &pct(25.0)));
    assert!(hull.params.is_empty());

    let orbit = ed.node(NodeRef::Action(rules[1].id)).unwrap();
    assert_eq!(orbit.node.role, NodeRole::Action);
    assert_eq!(orbit.params[1].value, word("ccw"));
    assert!(matches!(&orbit.params[1].param.ty, ParamType::Choice(o) if o.len() == 3));
    let face = ed.node(NodeRef::Action(rules[2].id)).unwrap();
    assert!(face.params.is_empty());

    assert_eq!(
        rules[1].nodes().collect::<Vec<_>>(),
        vec![
            NodeRef::Cond(rules[1].conds[0]),
            NodeRef::Cond(rules[1].conds[1]),
            NodeRef::Action(rules[1].id)
        ]
    );
    assert_eq!(
        rules[3].nodes().collect::<Vec<_>>(),
        vec![NodeRef::Action(rules[3].id)]
    );

    for role in [NodeRole::Condition, NodeRole::Action, NodeRole::Target] {
        let palette: Vec<&str> = ed.palette(role).map(|n| n.id.as_str()).collect();
        let content_order: Vec<&str> = c
            .behavior_nodes
            .iter()
            .filter(|n| n.role == role)
            .map(|n| n.id.as_str())
            .collect();
        assert_eq!(palette, content_order);
        let tags = ed.palette_tags(role);
        let mut unique = tags.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), tags.len(), "{tags:?}");
    }
    assert!(ed.palette(NodeRole::Action).any(|n| n.id == "orbit"));
    assert!(ed.palette(NodeRole::Condition).any(|n| n.script.is_some()));
    let tags = ed.palette_tags(NodeRole::Condition);
    for t in ["self", "target", "rhai"] {
        assert!(tags.contains(&t), "{tags:?}");
    }
}

#[test]
fn rule_slots_mark_the_lowest_rules_inactive() {
    let c = content();
    let mut ed = template(&c, "skirmisher");
    let before = ed.program().clone();
    let runner = c.ship_rule_slots(c.ships.get("halvex_runner").unwrap());
    let raider = c.ship_rule_slots(c.ships.get("automaton_raider").unwrap());
    assert_eq!(ed.active_rules(runner), 4);
    assert_eq!(ed.active_rules(raider), raider as usize);
    assert_eq!(ed.active_rules(0), 0);
    assert_eq!(ed.active_rules(u32::MAX), 4);
    assert_eq!(ed.program(), &before, "asking changes nothing");
    assert_eq!(ed.revision(), 0);
    // Priority is survival order: the rule moved to the top is active with two slots.
    let last = ids(&ed)[3];
    ed.move_rule(last, 0).unwrap();
    let active: Vec<RuleId> = ed.rules().take(ed.active_rules(2)).map(|r| r.id).collect();
    assert_eq!(active[0], last);
    assert_eq!(active.len(), 2);
}

#[test]
fn undo_redo_and_revision() {
    let c = content();
    let mut ed = template(&c, "raider_basic");
    let start = (ed.program().clone(), ids(&ed));
    assert_eq!(ed.revision(), 0);
    assert!(!ed.undo() && !ed.redo());
    let approach = ids(&ed)[1];
    let flee = ed.add_rule(0, "flee").unwrap();
    let hull = ed.add_condition(flee, "hull").unwrap();
    ed.set_compare(hull, CmpOp::Lt, pct(20.0)).unwrap();
    ed.delete_rule(approach).unwrap();
    assert_eq!(ed.revision(), 4);
    // Refused edits and edits that change nothing leave no step.
    assert!(ed.delete_rule(approach).is_err());
    assert_eq!(ed.set_compare(hull, CmpOp::Lt, pct(20.0)), Ok(false));
    assert_eq!(ed.revision(), 4);
    let end = (ed.program().clone(), ids(&ed));

    assert!(ed.undo());
    assert_eq!(ed.revision(), 5);
    assert_eq!(ed.rule(approach).unwrap().index, 2, "back under its id");
    while ed.undo() {}
    assert_eq!((ed.program().clone(), ids(&ed)), start);
    assert!(ed.node(NodeRef::Cond(hull)).is_none());
    assert!(ed.can_redo() && !ed.can_undo());
    while ed.redo() {}
    assert_eq!((ed.program().clone(), ids(&ed)), end);
    assert_eq!(ed.rule_of(hull), Some(flee));
    assert_eq!(ed.revision(), 12);

    ed.undo();
    ed.add_rule(0, "hold").unwrap();
    assert!(!ed.can_redo(), "a new edit clears the redo list");
    assert_round_trip(&ed);
}

#[test]
fn undo_keeps_the_last_steps() {
    const { assert!(UNDO_STEPS >= 100) };
    let c = content();
    let mut ed = editor(&c, "always -> hold");
    let r = ids(&ed)[0];
    for k in 0..UNDO_STEPS + 10 {
        ed.set_rule_note(r, Some(&format!("n{k}"))).unwrap();
    }
    let mut steps = 0;
    while ed.undo() {
        steps += 1;
    }
    assert_eq!(steps, UNDO_STEPS);
    assert_eq!(ed.program().rules[0].note.as_deref(), Some("n9"));
}

#[test]
fn strokes_make_one_step() {
    let c = content();
    let mut ed = editor(&c, "when hull < 30% -> flee");
    let hull = cond_ids(&ed, 0)[0];
    ed.begin_stroke();
    for x in [29.0, 25.0, 20.0] {
        ed.set_compare(hull, CmpOp::Lt, pct(x)).unwrap();
    }
    assert!(ed.can_undo());
    ed.end_stroke();
    assert_eq!(ed.revision(), 3);
    assert_eq!(ed.text(), "when hull < 20% -> flee\n");
    assert!(ed.undo());
    assert_eq!(ed.text(), "when hull < 30% -> flee\n");
    assert!(!ed.undo());
    // Undo closes an open stroke.
    ed.begin_stroke();
    ed.toggle_not(hull).unwrap();
    assert!(ed.undo());
    assert_eq!(ed.text(), "when hull < 30% -> flee\n");
}

#[test]
fn the_text_view_replaces_the_program_and_keeps_ids() {
    let c = content();
    let mut ed = template(&c, "skirmisher");
    let before = ids(&ed);
    let conds = cond_ids(&ed, 1);
    let rev = ed.revision();

    // One rule edited in place keeps its id and the ids of its unchanged conditions.
    let text = ed.text().replace(
        "when shield_down and target_on_me -> orbit(80%, ccw)",
        "when shield_down and target_on_me and under_attack -> orbit(70%, ccw)",
    );
    assert_eq!(ed.set_text(&text), Ok(true));
    assert_eq!(ed.revision(), rev + 1);
    assert_eq!(ids(&ed), before);
    let now = cond_ids(&ed, 1);
    assert_eq!(&now[..2], &conds[..]);
    assert_eq!(now.len(), 3);
    assert_round_trip(&ed);

    // A condition removed from the middle: the others keep their ids.
    let text = ed.text().replace(
        "when shield_down and target_on_me and under_attack",
        "when shield_down and under_attack",
    );
    ed.set_text(&text).unwrap();
    assert_eq!(cond_ids(&ed, 1), vec![now[0], now[2]]);

    // A line inserted: the other rules keep their ids.
    let text = ed
        .text()
        .replacen("\nwhen hull", "\nwhen heat > 90% -> flee\nwhen hull", 1);
    ed.set_text(&text).unwrap();
    let inserted = ids(&ed);
    assert_eq!(&inserted[1..], &before[..]);
    assert!(!before.contains(&inserted[0]));

    // A line deleted, typed sloppily on a phone: canonical in the model.
    let text = ed.text();
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.contains("closing_in"))
        .collect();
    let sloppy = lines.join("\n").replace("when", "If").replace("->", "→");
    ed.set_text(&sloppy).unwrap();
    let mut expect = before.clone();
    expect.remove(2);
    assert_eq!(&ids(&ed)[1..], &expect[..]);
    assert!(ed.text().contains("when hull < 25% -> flee"));
    assert_round_trip(&ed);

    // Errors: line and column, the model untouched.
    let snapshot = (ed.program().clone(), ids(&ed), ed.revision());
    let errors = ed
        .set_text("target nearest\nwhen hull < 30% -> flee\nwhen hull < 30% flee\n")
        .unwrap_err();
    assert_eq!((errors[0].line, errors[0].col), (3, 17));
    let errors = ed.set_text("always -> orbit(45m, sideways)\n").unwrap_err();
    assert_eq!((errors[0].line, errors[0].col), (1, 22));
    assert!(ed.set_text("").is_err());
    assert_eq!(
        ed.check_text("always -> hold").map(|p| p.rules.len()),
        Ok(1)
    );
    assert_eq!((ed.program().clone(), ids(&ed), ed.revision()), snapshot);

    // Undo puts back the program and its ids.
    while ed.undo() {}
    assert_eq!(ids(&ed), before);
    assert_eq!(ed.text(), template(&c, "skirmisher").text());
}

/// A vocabulary with required parameters (none in the base content): new nodes get starting values.
#[test]
fn required_parameters_get_starting_values() {
    let c = content();
    let mut nodes = c.behavior_nodes.clone();
    let mut within = nodes.get("target_in_range").unwrap().clone();
    within.id = "within".into();
    let range = &mut within.params[0];
    (range.default, range.min, range.max) = (None, Some(5.0), Some(50.0));
    nodes.upsert(within);
    let mut patrol = nodes.get("orbit").unwrap().clone();
    patrol.id = "patrol".into();
    patrol.params.remove(0);
    patrol.params[0].default = None;
    nodes.upsert(patrol);

    let mut ed = BehaviorEditor::from_text(&nodes, "always -> hold").unwrap();
    let r = ids(&ed)[0];
    let w = ed.add_condition(r, "within").unwrap();
    assert_eq!(line(&ed, 0), "when within(5m) -> hold");
    assert_eq!(
        ed.reset_param(NodeRef::Cond(w), 0),
        Err(Refusal::NoDefault {
            node: "within".into(),
            param: "range".into()
        })
    );
    ed.replace_node(NodeRef::Action(r), "patrol").unwrap();
    assert_eq!(line(&ed, 0), "when within(5m) -> patrol(auto)");
    ed.set_param(NodeRef::Cond(w), 0, metres(40.0)).unwrap();
    ed.replace_node(NodeRef::Cond(w), "target_in_range")
        .unwrap();
    assert_eq!(line(&ed, 0), "when target_in_range(40m) -> patrol(auto)");
    ed.set_param(NodeRef::Cond(w), 0, metres(100.0)).unwrap();
    ed.replace_node(NodeRef::Cond(w), "within").unwrap();
    assert_eq!(
        line(&ed, 0),
        "when within(5m) -> patrol(auto)",
        "100m is out of range: the starting value"
    );
    assert_eq!(
        ed.add_rule(1, "patrol").map(|_| line(&ed, 1)),
        Ok("always -> patrol(auto)".into())
    );
    assert_round_trip(&ed);
    assert!(BehaviorEditor::from_text(&nodes, "when within -> hold").is_err());
}

/// A value that fits a parameter: one of its options, or a whole number in its range and unit.
fn fitting(p: &NodeParam, r: usize) -> Value {
    let unit = match &p.ty {
        ParamType::Choice(options) => return word(&options[r % options.len()].id),
        ParamType::Distance => Some(Unit::Metres),
        ParamType::Percent => Some(Unit::Percent),
        ParamType::Seconds => Some(Unit::Seconds),
        ParamType::Count => None,
    };
    let (lo, hi) = (p.min.unwrap_or(0.0), p.max.unwrap_or(100.0));
    num((lo + (hi - lo) * (r % 101) as f32 / 100.0).round(), unit)
}

/// Deterministic pseudo-random edit sequences from both content behaviors: every step is legal or refused
/// without a change; the program always equals its text parsed again; undoing everything gives the start back
/// and redoing everything the end.
#[test]
fn random_edit_sequences_keep_the_text_round_trip() {
    let c = content();
    let node_ids: Vec<String> = c
        .behavior_nodes
        .iter()
        .map(|n| n.id.clone())
        .chain(["bogus".to_string()])
        .collect();
    let values = [
        pct(30.0),
        pct(120.0),
        pct(1001.0),
        metres(45.0),
        metres(0.5),
        metres(6000.0),
        num(3.0, None),
        num(2.5, None),
        num(-1.0, None),
        num(f32::NAN, None),
        num(5.0, Some(Unit::Seconds)),
        word("cw"),
        word("engines"),
        word("bogus"),
    ];
    let thresholds = [
        pct(30.0),
        num(3.0, None),
        num(5.0, Some(Unit::Seconds)),
        pct(-1.0),
    ];
    let notes = [None, Some(""), Some("dodge"), Some("a\nb"), Some("x # y")];
    let ops = [CmpOp::Lt, CmpOp::Le, CmpOp::Gt, CmpOp::Ge];
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut rnd = |n: usize| -> usize {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % n.max(1)
    };
    let (mut changed, mut refused) = (0, 0);
    for round in 0..8 {
        let mut ed = template(&c, ["raider_basic", "skirmisher"][round % 2]);
        let start = ed.program().clone();
        // Every id seen so far: some are stale by now and must be refused.
        let (mut seen_rules, mut seen_conds) = (Vec::new(), Vec::new());
        for _ in 0..200 {
            let rules = ids(&ed);
            let conds: Vec<CondId> = ed.rules().flat_map(|r| r.conds.to_vec()).collect();
            seen_rules.extend(&rules);
            seen_conds.extend(&conds);
            let rule = match rnd(8) {
                0 => seen_rules[rnd(seen_rules.len())],
                _ => rules[rnd(rules.len())],
            };
            let cond = match rnd(8) {
                0 => seen_conds.get(rnd(seen_conds.len())).copied(),
                _ => conds.get(rnd(conds.len() + 1)).copied(),
            };
            let node = match rnd(3) {
                0 => NodeRef::Target,
                1 => NodeRef::Action(rule),
                _ => cond.map_or(NodeRef::Target, NodeRef::Cond),
            };
            let id = node_ids[rnd(node_ids.len())].as_str();
            // Half the parameter values fit the parameter, the rest come from the pool.
            let index = rnd(3);
            let param = ed
                .node(node)
                .and_then(|v| v.params.get(index).map(|p| p.param.clone()));
            let value = match param {
                Some(p) if rnd(2) == 0 => fitting(&p, rnd(1000)),
                _ => values[rnd(values.len())].clone(),
            };
            let threshold = thresholds[rnd(thresholds.len())].clone();
            let before = (ed.program().clone(), ids(&ed), ed.revision());
            let len = rules.len();
            let outcome: Result<bool, String> = match rnd(16) {
                0 => ed
                    .add_rule(rnd(len + 2), id)
                    .map(|_| true)
                    .map_err(|e| e.to_string()),
                1 => ed.delete_rule(rule).map_err(|e| e.to_string()),
                2 => ed
                    .duplicate_rule(rule)
                    .map(|_| true)
                    .map_err(|e| e.to_string()),
                3 => ed.move_rule(rule, rnd(len + 1)).map_err(|e| e.to_string()),
                4 => ed
                    .set_rule_note(rule, notes[rnd(notes.len())])
                    .map_err(|e| e.to_string()),
                5 => ed
                    .add_condition(rule, id)
                    .map(|_| true)
                    .map_err(|e| e.to_string()),
                6 => match cond {
                    Some(k) if rnd(2) == 0 => ed.remove_condition(k).map_err(|e| e.to_string()),
                    Some(k) => ed.toggle_not(k).map_err(|e| e.to_string()),
                    None => Ok(false),
                },
                7 => match cond {
                    Some(k) => ed
                        .set_compare(k, ops[rnd(4)], threshold)
                        .map_err(|e| e.to_string()),
                    None => Ok(false),
                },
                8 | 9 => ed.replace_node(node, id).map_err(|e| e.to_string()),
                10 | 11 => ed.set_param(node, index, value).map_err(|e| e.to_string()),
                12 => ed.reset_param(node, index).map_err(|e| e.to_string()),
                13 => match rnd(3) {
                    0 => Ok(ed.clear_target()),
                    _ => ed
                        .set_target_note(notes[rnd(notes.len())])
                        .map_err(|e| e.to_string()),
                },
                14 => {
                    // The text view: swap two lines, drop one or repeat one.
                    let mut lines: Vec<String> = ed.text().lines().map(String::from).collect();
                    let (a, b) = (rnd(lines.len()), rnd(lines.len()));
                    match rnd(3) {
                        0 => lines.swap(a, b),
                        1 => drop(lines.remove(a)),
                        _ => lines.insert(b, lines[a].clone()),
                    }
                    ed.set_text(&lines.join("\n")).map_err(|e| format!("{e:?}"))
                }
                _ => Ok(if rnd(2) == 0 { ed.undo() } else { ed.redo() }),
            };
            let after = (ed.program().clone(), ids(&ed), ed.revision());
            match outcome {
                Ok(true) => {
                    changed += 1;
                    assert_eq!(after.2, before.2 + 1);
                }
                Ok(false) => assert_eq!(after, before),
                Err(_) => {
                    refused += 1;
                    assert_eq!(after, before, "a refused edit changes nothing");
                }
            }
            assert_round_trip(&ed);
            let mut all: Vec<CondId> = ed.rules().flat_map(|r| r.conds.to_vec()).collect();
            all.sort_unstable();
            all.dedup();
            assert_eq!(all.len(), ed.rules().map(|r| r.conds.len()).sum::<usize>());
            for r in ed.rules() {
                assert_eq!(r.conds.len(), r.rule.when.len());
            }
        }
        while ed.redo() {}
        let end = ed.program().clone();
        while ed.undo() {}
        assert_eq!(ed.program(), &start, "round {round}: undo all");
        while ed.redo() {}
        assert_eq!(ed.program(), &end, "round {round}: redo all");
    }
    assert!(
        changed > 400 && refused > 100,
        "{changed} changed, {refused} refused"
    );
}
