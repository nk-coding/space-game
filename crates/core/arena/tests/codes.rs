//! Fight codes and the rules' fingerprint (GDR-0092): a code round-trips and replays as recorded, damaged and
//! foreign text is refused with a reason, changed rules are detected, and a client stepping the world itself with
//! the setup and the referee gets the same fight.

use std::sync::Arc;

use arena::{
    BehaviorRef, CodeError, DesignRef, Entry, Fight, FightCode, FightSetup, FightSpec, Formation, run_fight,
    rules_fingerprint, standing,
};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// A fight of a blueprint design under a behavior text (multi-line strings in the code) against two content ships.
fn spec(content: &data::Content) -> FightSpec {
    let skiff = data::blueprint_to_ron(content.ships.get("rim_skiff").unwrap());
    let behavior = content.behaviors.get("skirmisher").unwrap().program().unwrap().to_text();
    let mut spec = FightSpec::new(
        4,
        vec![Entry { design: DesignRef::Blueprint(skiff), behavior: Some(BehaviorRef::Text(behavior)), count: 1, formation: Formation::Line }],
        vec![Entry { count: 2, ..Entry::ship("automaton_raider") }],
    );
    spec.time_limit = 30.0;
    spec
}

#[test]
fn a_fight_code_round_trips_and_replays_as_recorded() {
    let content = data::base_content();
    let spec = spec(&content);
    let record = run_fight(&content, &spec).unwrap();
    let rules = rules_fingerprint(&content);
    let code = FightCode::new(&content, rules, spec, record);
    assert_eq!(code.packs, vec![("base".to_string(), content.packs[0].version.clone())]);
    let text = code.encode();
    assert!(text.starts_with("SGF1:") && text.len() < 4000, "{} characters", text.len());
    assert!(text[5..].chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "safe in a URL as it is");
    let back = FightCode::decode(&text).unwrap();
    assert_eq!(back, code);
    assert_eq!(run_fight(&content, &back.spec).unwrap(), back.record, "replays as recorded");
    // Pasted with a line break and spaces around it, and with a lower-case tag.
    let wrapped = format!("  {}\n{}  ", &text[..60], &text[60..]);
    assert_eq!(FightCode::decode(&wrapped).unwrap(), code);
    assert_eq!(FightCode::decode(&text.replacen("SGF", "sgf", 1)).unwrap(), code);
    // A content-ship fight is short.
    let short = FightSpec::new(1, vec![Entry::ship("halvex_runner")], vec![Entry { count: 3, ..Entry::ship("automaton_raider") }]);
    let r = run_fight(&content, &short).unwrap();
    assert!(FightCode::new(&content, rules, short, r).encode().len() < 500, "about 430 characters");
}

#[test]
fn damaged_and_foreign_codes_are_refused_with_a_reason() {
    let content = data::base_content();
    let spec = FightSpec::new(2, vec![Entry::ship("rim_skiff")], vec![Entry::ship("automaton_raider")]);
    let mut short = spec.clone();
    short.time_limit = 3.0;
    let record = run_fight(&content, &short).unwrap();
    let text = FightCode::new(&content, 7, short, record).encode();
    let err = |t: &str| FightCode::decode(t).unwrap_err();
    assert_eq!(err("hello"), CodeError::NotACode);
    assert_eq!(err("SGD2:abc"), CodeError::NotACode, "a design's share code");
    assert_eq!(err("SGFx:abc"), CodeError::NotACode);
    assert_eq!(err("SGF9:abc"), CodeError::Newer(9));
    assert_eq!(err("SGF0:abc"), CodeError::Damaged);
    assert_eq!(err(&text[..text.len() - 6]), CodeError::Damaged, "cut short");
    assert_eq!(err(&format!("{}*", &text[..text.len() - 1])), CodeError::Damaged, "not base64url");
    let mid = text.len() / 2;
    let flipped = if &text[mid..mid + 1] == "A" { "B" } else { "A" };
    assert_eq!(err(&format!("{}{flipped}{}", &text[..mid], &text[mid + 1..])), CodeError::Damaged, "checksum");
    assert_eq!(err(&format!("SGF1:{}", "A".repeat(arena::MAX_PACKED / 3 * 4 + 8))), CodeError::TooLarge);
    let bomb = miniz_oxide::deflate::compress_to_vec_zlib(&vec![b' '; arena::MAX_PAYLOAD + 1], 10);
    assert_eq!(err(&format!("SGF1:{}", URL_SAFE_NO_PAD.encode(bomb))), CodeError::TooLarge);
    let junk = miniz_oxide::deflate::compress_to_vec_zlib(b"(packs: [])", 6);
    assert!(matches!(err(&format!("SGF1:{}", URL_SAFE_NO_PAD.encode(junk))), CodeError::Unreadable(_)));
    assert!(err("SGF9:abc").to_string().contains("newer"));
}

/// GDR-0092: the fingerprint is the same for the same rules, and a code fought under other rules is told apart:
/// here a part the fight uses got heavier, which also changes how it ends.
#[test]
fn a_stale_fingerprint_is_detected() {
    let content = data::base_content();
    let rules = rules_fingerprint(&content);
    assert_eq!(rules_fingerprint(&data::base_content()), rules);
    assert_ne!(rules, content.fingerprint(), "the probes are in it");
    let spec = FightSpec::new(3, vec![Entry::ship("halvex_runner")], vec![Entry { count: 3, ..Entry::ship("automaton_raider") }]);
    let code = FightCode::new(&content, rules, spec.clone(), run_fight(&content, &spec).unwrap());
    assert_eq!(code.rules, rules_fingerprint(&content), "fresh");
    let mut changed = content.clone();
    let thruster = changed.parts.get_mut("thruster").unwrap();
    thruster.mass = Some(thruster.mass.unwrap_or(1.0) * 3.0);
    let now = rules_fingerprint(&changed);
    assert_ne!(code.rules, now, "stale");
    assert_ne!(run_fight(&changed, &code.spec).unwrap(), code.record, "and it no longer ends as recorded");
}

/// The fight viewer's way (GDR-0092): the setup's world stepped with no inputs and the referee asked after every
/// tick gives `run_fight`'s result; the setup hands out every entry's blueprint and design.
#[test]
fn the_setup_and_referee_give_the_fights_result() {
    let content = data::base_content();
    let spec = spec(&content);
    let mut setup = FightSetup::build(&content, &spec).unwrap();
    assert_eq!(setup.designs.each_ref().map(|d| d.len()), [1, 1]);
    assert_eq!(setup.designs[0][0].0.id, "rim_skiff");
    assert!(setup.sim.ships().iter().all(|s| setup.designs.iter().flatten().any(|(_, d)| Arc::ptr_eq(d, s.design()))));
    let bp = setup.referee.build_points();
    assert_eq!([standing(&setup.sim, 0).0, standing(&setup.sim, 1).0], bp);
    assert_eq!((standing(&setup.sim, 0).1, standing(&setup.sim, 1).1), (1, 2));
    assert_eq!(setup.referee.limit(), 900);
    let idle = sim::TickInputs::default();
    let result = loop {
        setup.sim.step(&idle);
        if let Some(r) = setup.referee.observe(&mut setup.sim) {
            break r;
        }
    };
    assert_eq!(result, run_fight(&content, &spec).unwrap());
    assert_eq!(setup.referee.observe(&mut setup.sim), Some(result), "the end stays");
    // A cloned world and referee go on as the original (the viewer's keyframes).
    let mut fight = Fight::new(&content, &spec).unwrap();
    fight.step(200);
    let mut setup = FightSetup::build(&content, &spec).unwrap();
    for _ in 0..200 {
        setup.sim.step(&idle);
        setup.referee.observe(&mut setup.sim);
    }
    let (mut sim, mut referee) = (setup.sim.clone(), setup.referee.clone());
    assert_eq!(sim.state_hash(), fight.sim().state_hash());
    let end = loop {
        sim.step(&idle);
        if let Some(r) = referee.observe(&mut sim) {
            break r;
        }
    };
    assert_eq!(end, result);
}
