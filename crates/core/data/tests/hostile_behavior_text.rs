//! Hostile and damaged behavior text (the TEXT view, stored behaviors, behaviors carried by share codes, mods):
//! every input ends in a clean error or a legal program whose canonical text parses back to it. The editor
//! checks that invariant with a `debug_assert` after every change, so a case that slips through panics here.

mod common;

use common::*;
use data::behavior::{MAX_LINE_CHARS, MAX_RULES, Program, check_program, check_text};
use data::behavior::edit::BehaviorEditor;
use data::{Content, Pack, Problem, Severity};

fn nodes() -> data::Table<data::BehaviorNode> {
    content().behavior_nodes
}

/// What every reader of behavior text does with `text`; none may panic. When the text checks, the program is
/// legal: every printed line within the limit, the print parses back to the same program, and the editor takes
/// it (set_text runs the editor's invariant check).
fn every_reader(nodes: &data::Table<data::BehaviorNode>, text: &str) -> bool {
    let _ = Program::parse(text);
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let _ = Program::from_parts(None, &lines);
    let _ = Program::from_parts(Some(text), &[]);
    let checked = check_text(text, nodes);
    let mut ed = BehaviorEditor::from_text(nodes, "always -> hold").unwrap();
    assert_eq!(ed.set_text(text).is_ok(), checked.is_ok(), "{text:?}");
    assert_eq!(BehaviorEditor::from_text(nodes, text).is_ok(), checked.is_ok(), "{text:?}");
    let Ok(program) = checked else { return false };
    let printed = program.to_text();
    for line in printed.lines() {
        assert!(line.chars().count() <= MAX_LINE_CHARS, "{text:?} prints {line:?}");
    }
    assert_eq!(Program::parse(&printed).as_ref(), Ok(&program), "{text:?} prints {printed:?}");
    let (target, rules) = program.to_parts();
    let back = Program::from_parts(target.as_deref(), &rules).unwrap();
    assert!(check_program(&back, nodes).is_empty(), "{text:?}");
    assert_eq!(ed.text(), printed);
    true
}

fn mod_errors(behavior: &str) -> Vec<Problem> {
    let m = Pack::from_files([
        ("pack.ron", r#"(id: "m", name: "M", version: "1", schema_version: 1, depends: ["base"])"#),
        ("behaviors/b.ron", behavior),
    ])
    .unwrap_or_else(|p| panic!("{}", show(&p)));
    match Content::load(&[data::base_pack().unwrap(), m]) {
        Ok(_) => Vec::new(),
        Err(p) => p.into_iter().filter(|p| p.severity == Severity::Error).collect(),
    }
}

/// A RON string literal of `s` (for the `rules` of a mod behavior).
fn ron_str(s: &str) -> String {
    ron::to_string(&s).unwrap()
}

// ---- lines that print longer than they were typed (review finding 2) ----

#[test]
fn compact_lines_that_print_longer_than_the_limit_are_refused() {
    let nodes = nodes();
    // The review's case: 238 characters typed, 242 printed (`always -> flee # nnn…`).
    let compact = format!("always->flee#{}", "n".repeat(225));
    assert_eq!(compact.chars().count(), 238);
    let errors = check_text(&compact, &nodes).unwrap_err();
    assert!(errors[0].message.contains("too long once written out (242 characters"), "{errors:?}");
    assert!(!every_reader(&nodes, &compact));
    // Printed exactly at the limit: fine, and the print is the canonical text.
    let at_limit = format!("always->flee#{}", "n".repeat(223));
    assert!(every_reader(&nodes, &at_limit));
    assert_eq!(check_text(&at_limit, &nodes).unwrap().to_text().trim_end().chars().count(), MAX_LINE_CHARS);
    // `if`, `→`, `≤` and squeezed comparisons all grow when printed.
    let squeezed = format!("If hull≤1%and shield_down→orbit(1,cw)#{}", "x".repeat(200));
    assert!(squeezed.chars().count() <= MAX_LINE_CHARS);
    assert!(!every_reader(&nodes, &squeezed));
    // The target line counts with its keyword, in the RON form too.
    let target = format!("nearest(engines) # {}", "t".repeat(MAX_LINE_CHARS - 20));
    let program = Program::from_parts(Some(&target), &["always -> hold".to_string()]).unwrap();
    assert!(check_program(&program, &nodes).iter().any(|e| e.message.contains("too long once written out")));
    // In content (a mod, a stored behavior, a share code's behavior): an error, so the editor can always open
    // what validates.
    let b = format!(
        r#"(id: "b", name: "B", summary: "B.", rules: [{}])"#,
        ron_str(&compact)
    );
    let e = mod_errors(&b);
    assert!(e.iter().any(|p| p.field.as_deref() == Some("rules[0]") && p.message.contains("too long")), "{}", show(&e));
}

#[test]
fn compact_lines_near_the_limit_agree_everywhere() {
    let nodes = nodes();
    let bodies = [
        "always->flee",
        "when hull<30%->flee",
        "If hull<30%and shield_down→orbit(80%,ccw)",
        "when not target_in_range(120%)and cycle(9s)>=5s->approach(85%)",
        "when enemies_near(150m)>=3 and heat>50%->kite(100m)",
    ];
    for body in bodies {
        for pad in 150..=MAX_LINE_CHARS {
            let Some(room) = pad.checked_sub(body.chars().count() + 1) else { continue };
            let line = format!("{body}#{}", "é".repeat(room));
            let ok = every_reader(&nodes, &line);
            let printed = Program::parse(&line).map(|p| p.to_text().trim_end().chars().count());
            assert_eq!(ok, printed.is_ok_and(|n| n <= MAX_LINE_CHARS), "{line}");
        }
    }
}

// ---- notes with control characters (review finding 4) ----

#[test]
fn notes_with_control_characters_are_refused() {
    let nodes = nodes();
    // A share code's behavior with a line break in a note: one rule in the RON form, two in the text form.
    let split = ["when hull < 30% -> flee # run\nalways -> hold".to_string()];
    let errors = Program::from_parts(None, &split).unwrap_err();
    assert!(errors[0].message.contains("a note is one line"), "{errors:?}");
    assert_eq!(errors[0].col, 30);
    let b = format!(r#"(id: "b", name: "B", summary: "B.", rules: [{}])"#, ron_str(&split[0]));
    let e = mod_errors(&b);
    assert!(e.iter().any(|p| p.field.as_deref() == Some("rules[0]") && p.message.contains("one line")), "{}", show(&e));
    for (line, col) in [
        ("always -> flee # a\tb", 19),
        ("always -> flee #  x\u{0}", 20),
        ("always -> flee # x\u{85}y", 19),
        ("always -> flee # \u{1b}[31m red", 18),
    ] {
        let err = check_text(line, &nodes).unwrap_err();
        assert_eq!((err[0].col, err[0].message.contains("one line")), (col, true), "{line:?}: {err:?}");
        assert!(!every_reader(&nodes, line));
    }
    // Whitespace at the ends of a note is trimmed, line endings of any kind are fine.
    for ok in ["always -> flee # x\t", "always -> flee # x\r", "always -> flee #\tx", "target nearest # t\r\nalways -> flee # x\r\n"] {
        assert!(every_reader(&nodes, ok), "{ok:?}");
    }
}

// ---- size ----

#[test]
fn huge_behaviors_cost_no_more_than_a_full_program() {
    let nodes = nodes();
    let many = "always -> hold\n".repeat(100_000);
    let errors = Program::parse(&many).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!((errors[0].line, errors[0].message.as_str()), (MAX_RULES as u32 + 2, "at most 64 rules"));
    let full = "always -> hold\n".repeat(MAX_RULES);
    assert!(every_reader(&nodes, &full));
    assert!(!every_reader(&nodes, &format!("{full}always -> hold")));
    // Errors stop too.
    let junk = "@\n".repeat(100_000);
    assert_eq!(Program::parse(&junk).unwrap_err().len(), MAX_RULES + 2);
    // The RON form: refused before any rule is parsed.
    let rules = vec!["always -> hold".to_string(); 100_000];
    assert_eq!(Program::from_parts(None, &rules).unwrap_err().len(), 1);
    // One enormous line, and many blank ones.
    let long = "a".repeat(10 << 20);
    assert!(Program::parse(&long).unwrap_err()[0].message.contains("line too long"));
    assert!(every_reader(&nodes, &format!("{}always -> hold", "\n \t\n".repeat(100_000))));
}

// ---- a table of hostile lines ----

#[test]
fn hostile_lines_give_clean_errors() {
    let nodes = nodes();
    let lines = [
        "",
        "\u{feff}always -> flee",
        "->",
        "-",
        "--------->",
        "always",
        "always ->",
        "always -> ->",
        "always -> flee flee",
        "always -> flee()()",
        "always -> orbit(,)",
        "always -> orbit(1,2,3,4,5,6,7,8,9)",
        "always -> orbit(((((((((((((((((((((",
        "always -> orbit)))))))",
        "when when when -> flee",
        "when not not hull < 1% -> flee",
        "when hull < -0% -> flee",
        "when hull < 1e9% -> flee",
        "when hull < 340282350000000000000000000000000000000% -> flee",
        "when hull < 999999999999999999999999999999999999999999% -> flee",
        "when hull < 0.0000000000000000000000000000000000000000001% -> flee",
        "when hull < 30.% -> flee",
        "when hull < .5 -> flee",
        "when hull < 5km -> flee",
        "when enemies_near(150m) >= 4294967296 -> flee",
        "when enemies_near(150m) >= 1.5 -> flee",
        "when cycle(0s) >= 0s -> flee",
        "when cycle(-1s) >= 0s -> flee",
        "when target_in_range(1000%) -> flee",
        "when target_in_range(1000.0001%) -> flee",
        "when closing_in(60) -> flee",
        "when closing_in(1e3) -> flee",
        "target target nearest",
        "target",
        "target nearest(engines, engines)",
        "target nearest\ntarget nearest\nalways -> flee",
        "always -> flee\ntarget nearest",
        "# just a note",
        "always -> flee #",
        "always -> flee ## ##",
        "WHEN HULL < 30% -> FLEE",
        "when hull ≤≥ 30% -> flee",
        "when hull < 30% → → flee",
        "when shield_down and and -> flee",
        "when shield_down or under_attack -> flee",
        "when shield_down && under_attack -> flee",
        "when !shield_down -> flee",
        "when hull = 30% -> flee",
        "when hull < 30% -> flee\0",
        "\0\0\0",
        "always -> 🚀",
        "always -> flee # 🚀🚀🚀",
        "always -> fléé",
        "always → hold # \u{202e}evil",
        "\u{2028}always -> flee",
        "always\u{a0}-> flee",
        "always -> flee\r\r\r",
        "always -> orbit(80%, sideways)",
        "always -> orbit(80m, 1)",
        "always -> orbit(cw)",
    ];
    for line in lines {
        every_reader(&nodes, line);
    }
    // Pseudo-random lines from the language's own pieces (and some junk), as a phone might mangle them.
    let pieces = [
        "when ", "if ", "always", "->", "→", " and ", "not ", "hull", "shield_down", "cycle(9s)", "<", "<=", "≤",
        ">=", "30%", "5s", "45m", "1.5", "-2", "(", ")", ",", "#", " ", "\t", "orbit", "flee", "approach(85%)",
        "target ", "nearest", "(engines)", "cw", "é", "🚀", "\u{0}", "x", "99999999999999999999999999999999999999999",
    ];
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let c = content();
    let rules: Vec<&str> = c.behaviors.iter().flat_map(|b| b.rules.iter().map(String::as_str)).collect();
    let mut legal = 0;
    for _ in 0..20_000 {
        // A content rule with a few random edits (a piece inserted, a character dropped, spaces squeezed).
        let mut line: Vec<char> = rules[(next() % rules.len() as u64) as usize].chars().collect();
        for _ in 0..next() % 4 {
            let at = (next() % (line.len() as u64 + 1)) as usize;
            match next() % 3 {
                0 => {
                    let piece = pieces[(next() % pieces.len() as u64) as usize];
                    line.splice(at..at, piece.chars());
                }
                1 if at < line.len() => {
                    line.remove(at);
                }
                _ => line.retain(|&ch| ch != ' ' || next() % 2 == 0),
            }
        }
        let line: String = line.into_iter().collect();
        legal += every_reader(&nodes, &line) as u32;
        // Pure junk from the same pieces.
        let junk: String = (0..next() % 24).map(|_| pieces[(next() % pieces.len() as u64) as usize]).collect();
        every_reader(&nodes, &junk);
    }
    assert!(legal > 1000, "only {legal} legal lines");
}
