//! The evolution lab's content (`lab.ron`, GDR-0092 steps 5 and 8): the base pack's goals, trial presets, kinds,
//! sizes, measures, spreads and budget, a mod that retunes one, and the errors a bad file gets.

use data::{Content, GOALS, MEASURES, Pack, SPREADS, Severity, TRIAL_KINDS, TrialPresetKind};

const MOD_MANIFEST: &str =
    r#"(id: "test_mod", name: "Test mod", version: "1.0.0", schema_version: 1, depends: ["base"])"#;

fn with_lab(text: &str) -> Result<Content, Vec<data::Problem>> {
    let m = Pack::from_files([("pack.ron", MOD_MANIFEST), ("lab.ron", text)])?;
    Content::load(&[data::base_pack().unwrap(), m])
}

#[test]
fn the_base_lab_has_the_four_goals_and_the_default_trials() {
    let c = data::base_content();
    let ids: Vec<&str> = c.lab.goals.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids, GOALS);
    assert_eq!(c.lab.goal("win").unwrap().weight, 1.0, "WIN alone by default: today's score");
    assert!(c.lab.goals.iter().filter(|g| g.id != "win").all(|g| g.weight == 0.0));
    // GDR-0092's default selection: the enemies you meet (weight 2), the test-range waves, beat your start, the
    // hall of fame.
    let on: Vec<(&str, f32)> = c.lab.trials.iter().filter(|t| t.default).map(|t| (t.id.as_str(), t.weight)).collect();
    assert_eq!(on, [("enemies_you_meet", 2.0), ("test_range_waves", 1.0), ("beat_your_start", 1.0), ("hall_of_fame", 1.0)]);
    assert_eq!(c.lab.trial("test_range_waves").unwrap().kind, TrialPresetKind::Waves { target: 5 });
    assert_eq!(c.lab.trial("hall_of_fame").unwrap().kind, TrialPresetKind::HallOfFame { count: 4 });
    assert!(c.lab.goal_presets.iter().any(|p| p.id == "win_unhurt"));
    // The lab changes no fight: it is not part of the rules' fingerprint.
    let mut other = c.clone();
    other.lab.trials.clear();
    assert_eq!(other.fingerprint(), c.fingerprint());
}

#[test]
fn a_mod_retunes_a_preset_in_its_place() {
    let c = with_lab(r#"(trials: [(id: "beat_your_start", name: "Beat it", summary: "s", kind: BeatYourStart, weight: 3, seeds: 5)])"#).unwrap();
    let k = c.lab.trials.iter().position(|t| t.id == "beat_your_start").unwrap();
    assert_eq!(c.lab.trials[k].name, "Beat it");
    assert_eq!((c.lab.trials[k].weight, c.lab.trials[k].seeds, c.lab.trials[k].default), (3.0, 5, false));
    assert_eq!(k, 2, "in its place");
    assert_eq!(c.lab.goals.len(), 4, "the base's goals stay");
    assert_eq!(c.lab.source.pack, "test_mod");
}

#[test]
fn a_bad_lab_says_where() {
    let bad = r#"(
        goals: [(id: "fast", name: "Fast", summary: "s"), (id: "win", name: "Win", summary: "s", weight: 7)],
        goal_presets: [(id: "none", name: "None", weights: {"win": 0})],
        trials: [
            (id: "a", name: "A", summary: "s", kind: EnemyShips(ships: [(ship: "no_such_ship")]), weight: -1),
            (id: "b", name: "B", summary: "s", kind: Waves(target: 0), seeds: 0, time_limit: 5000),
            (id: "g", name: "G", summary: "s", kind: Gauntlet(waves: [[]])),
            (id: "v", name: "V", summary: "s", kind: Voyages, time_limit: 900),
        ],
    )"#;
    let errors: Vec<(String, String)> = with_lab(bad)
        .unwrap_err()
        .into_iter()
        .filter(|p| p.severity == Severity::Error)
        .map(|p| (p.field.unwrap_or_default(), p.message))
        .collect();
    let has = |field: &str, needle: &str| {
        assert!(errors.iter().any(|(f, m)| f == field && m.contains(needle)), "no `{needle}` at {field}: {errors:#?}")
    };
    has("lab.goals[fast]", "not a goal");
    has("lab.goals[win].weight", "0 to 3");
    has("lab.goal_presets[none].weights", "every goal weighs 0");
    has("lab.trials[a].kind.ships[0].ship", "no ship `no_such_ship`");
    has("lab.trials[a].weight", "0 to 3");
    has("lab.trials[b].seeds", "1 to 64");
    has("lab.trials[b].time_limit", "at most 1800");
    has("lab.trials[b].kind", "0 waves");
    has("lab.trials[g].kind.waves[0]", "no ships");
    has("lab.trials[v].time_limit", "at most 600");
    // An id twice in one file is the loader's error (across packs it replaces).
    let twice = r#"(trials: [(id: "a", name: "A", summary: "s", kind: Fleet), (id: "a", name: "A", summary: "s", kind: Fleet)])"#;
    let p = Pack::from_files([("pack.ron", MOD_MANIFEST), ("lab.ron", twice)]).unwrap_err();
    assert!(p.iter().any(|p| p.field.as_deref() == Some("lab.trials[a]") && p.message.contains("listed twice")), "{p:?}");
    // A lab that doesn't read is a parse error of its file.
    let unread = Pack::from_files([("pack.ron", MOD_MANIFEST), ("lab.ron", "(goals: [(id: 3)])")]).unwrap_err();
    assert!(unread.iter().any(|p| p.source.as_ref().is_some_and(|s| s.file == "lab.ron")), "{unread:?}");
}

#[test]
fn the_base_lab_names_every_kind_and_measure_and_has_sizes_spreads_and_a_budget() {
    let c = data::base_content();
    let kinds: Vec<&str> = c.lab.kinds.iter().map(|k| k.id.as_str()).collect();
    assert_eq!(kinds, TRIAL_KINDS);
    assert!(c.lab.trials.iter().all(|t| c.lab.kind(t.kind.id()).is_some()));
    let measures: Vec<&str> = c.lab.measures.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(measures, MEASURES);
    assert_eq!(c.lab.measure_words(&c.stats, "acceleration"), Some(("Thrust/mass", "m/s²")), "a stat's words");
    assert_eq!(c.lab.measure_words(&c.stats, "engage_distance"), Some(("Engage distance", "m")), "its own");
    assert_eq!(c.lab.measure_words(&c.stats, "rules"), Some(("Rules", "")));
    assert!(SPREADS.iter().all(|id| c.lab.spread(id).is_some()));
    let size = c.lab.default_size().unwrap();
    assert_eq!((size.id.as_str(), size.population, size.generations), ("standard", 32, 50));
    assert!(c.lab.budget.is_some_and(|b| b.layer_open > b.elsewhere));
    // Its texts are keyed like the entries' (a translation's template).
    let texts = c.texts();
    assert_eq!(texts["lab.goal.unhurt.name"], "Unhurt");
    assert_eq!(texts["lab.size.deep.name"], "Deep");
    assert!(texts.contains_key("lab.kind.co_evolution.summary") && texts.contains_key("lab.measure.engage_distance.unit"));
}

#[test]
fn bad_sizes_measures_and_spreads_say_where() {
    let bad = r#"(
        sizes: [(id: "huge", name: "H", summary: "s", population: 0, generations: 5, default: true), (id: "b", name: "B", summary: "s", population: 4, generations: 4, default: true)],
        measures: [(id: "speed", name: "S", summary: "s"), (id: "mass", stat: "no_stat", summary: "s"), (id: "parts", summary: "s")],
        spreads: [(id: "design", axes: [(measure: "nothing", min: 5, max: 1, cells: 4)])],
        kinds: [(id: "duel", name: "D", summary: "s")],
        budget: (layer_open: 0, elsewhere: 4),
    )"#;
    let errors: Vec<(String, String)> = with_lab(bad)
        .unwrap_err()
        .into_iter()
        .filter(|p| p.severity == Severity::Error)
        .map(|p| (p.field.unwrap_or_default(), p.message))
        .collect();
    let has = |field: &str, needle: &str| {
        assert!(errors.iter().any(|(f, m)| f == field && m.contains(needle)), "no `{needle}` at {field}: {errors:#?}")
    };
    has("lab.sizes[huge].population", "1 to 4096");
    has("lab.sizes", "more than one size");
    has("lab.measures[speed]", "not a measure");
    has("lab.measures[mass].stat", "no stat `no_stat`");
    has("lab.measures[parts]", "a stat or a name");
    has("lab.spreads[design].axes[0].measure", "no measure `nothing`");
    has("lab.spreads[design].axes[0]", "min below max");
    has("lab.kinds[duel]", "not a trial kind");
    has("lab.budget.layer_open", "above 0");
}
