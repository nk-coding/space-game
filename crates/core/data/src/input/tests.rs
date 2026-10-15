use std::collections::BTreeMap;

use super::*;

fn base() -> Bindings {
    let c = crate::base_content();
    Bindings::new(c.controls.as_slice(), &BTreeMap::new()).0
}

fn chord(s: &str) -> Chord {
    s.parse().unwrap()
}

/// Does `action` fire when exactly `pressed` is down with `held` modifiers?
fn fires(b: &Bindings, action: Action, pressed: &str, held: Mods) -> bool {
    let button = Button::named(pressed).unwrap();
    b.fires(action, |x| x == button, held)
}

const SHIFT: Mods = Mods { ctrl: false, shift: true, alt: false };
const CTRL: Mods = Mods { ctrl: true, shift: false, alt: false };
const NONE: Mods = Mods { ctrl: false, shift: false, alt: false };

/// Chords read and write the same text, and read on screen as the settings screen shows them.
#[test]
fn chords_read_write_and_label() {
    for s in ["KeyW", "Ctrl+Shift+KeyZ", "MouseRight", "Alt+F4", "Shift+ArrowUp", "NumpadAdd"] {
        assert_eq!(chord(s).to_string(), s);
    }
    assert_eq!(chord("Shift+Ctrl+KeyZ").to_string(), "Ctrl+Shift+KeyZ", "modifiers in a fixed order");
    assert_eq!(chord("Ctrl+Shift+KeyZ").label(), "CTRL+SHIFT+Z");
    assert_eq!(chord("ArrowUp").label(), "UP");
    assert_eq!(chord("MouseRight").label(), "RMB");
    assert_eq!(chord("NumpadSubtract").label(), "NUM −");
    assert_eq!(chord("Digit7").label(), "7");
    assert_eq!(chord("Shift+ShiftLeft"), chord("ShiftLeft"), "a modifier key needs no modifiers");
    for bad in ["", "Ctrl+", "Hyper+KeyA", "KeyAA", "Ctrl+Ctrl+KeyA", "Mouse9"] {
        assert!(bad.parse::<Chord>().is_err(), "{bad}");
    }
}

/// The base pack binds every action once, with nothing taken twice on a screen.
#[test]
fn the_base_pack_binds_every_action() {
    let c = crate::base_content();
    for a in Action::ALL {
        assert!(c.controls.contains(a.id()), "{a:?} has no control");
    }
    assert!(clashes(c.controls.as_slice()).is_empty(), "{:?}", clashes(c.controls.as_slice()));
    let b = base();
    assert_eq!(b.label(Action::ThrustForward), "W / UP");
    assert_eq!(b.label(Action::Fire), "SPACE / RMB");
    assert_eq!(b.label(Action::Redo), "CTRL+SHIFT+Z / CTRL+Y");
    assert!(b.changed().is_empty());
}

/// Shift is ignored unless the key is also bound with Shift where the action acts; Ctrl and Alt must match.
#[test]
fn modifiers_match_like_the_module_says() {
    let b = base();
    // W thrusts with Shift held (nothing in flight binds Shift+W) but not with Ctrl.
    assert!(fires(&b, Action::ThrustForward, "KeyW", NONE));
    assert!(fires(&b, Action::ThrustForward, "KeyW", SHIFT));
    assert!(!fires(&b, Action::ThrustForward, "KeyW", CTRL));
    // R recalls, Shift+R restarts the test range, never both.
    assert!(fires(&b, Action::Recall, "KeyR", NONE) && !fires(&b, Action::Recall, "KeyR", SHIFT));
    assert!(fires(&b, Action::ResetScene, "KeyR", SHIFT) && !fires(&b, Action::ResetScene, "KeyR", NONE));
    // The drydock: R turns right, Shift+R left.
    assert!(fires(&b, Action::RotateRight, "KeyR", NONE) && !fires(&b, Action::RotateRight, "KeyR", SHIFT));
    assert!(fires(&b, Action::RotateLeft, "KeyR", SHIFT));
    // Ctrl+Z undoes, Ctrl+Shift+Z redoes, a plain Z does neither.
    assert!(fires(&b, Action::Undo, "KeyZ", CTRL) && !fires(&b, Action::Undo, "KeyZ", Mods { shift: true, ..CTRL }));
    assert!(fires(&b, Action::Redo, "KeyZ", Mods { shift: true, ..CTRL }));
    assert!(!fires(&b, Action::Undo, "KeyZ", NONE));
    // A mouse button.
    assert!(fires(&b, Action::Fire, "MouseRight", NONE));
}

/// Rebinding takes the input away from an action sharing a screen, not from one elsewhere; the change is saved as
/// text, read back the same, and a reset brings the defaults back.
#[test]
fn rebinding_moves_a_taken_input_and_saves_the_changes() {
    let mut b = base();
    // F is the camera's in flight, FLEET's at home and the drydock's flip: binding it to drift takes it from the
    // camera only.
    assert_eq!(b.set(Action::Drift, 1, chord("KeyF")), Some((Action::CameraFollow, 0)));
    assert_eq!(b.label(Action::Drift), "X / F");
    assert_eq!(b.label(Action::CameraFollow), "—");
    assert_eq!(b.label(Action::OpenFleet), "F");
    assert_eq!(b.label(Action::Flip), "F");
    // The same chord in the other slot moves over.
    assert_eq!(b.set(Action::Drift, 0, chord("KeyF")), None);
    assert_eq!(b.label(Action::Drift), "F");
    b.clear(Action::Fire, 1);
    let saved = b.changed();
    assert_eq!(saved.keys().collect::<Vec<_>>(), ["camera_follow", "drift", "fire"]);
    assert_eq!(saved["drift"], ["KeyF"]);
    let c = crate::base_content();
    let (back, notes) = Bindings::new(c.controls.as_slice(), &saved);
    assert!(notes.is_empty());
    assert_eq!(back, b);
    b.reset();
    assert_eq!(b, base());
}

/// Saved controls the game doesn't know, or that don't read, are skipped with a note.
#[test]
fn unreadable_saved_controls_are_skipped() {
    let c = crate::base_content();
    let saved = BTreeMap::from([
        ("warp_drive".to_string(), vec!["KeyJ".to_string()]),
        ("fire".to_string(), vec!["KeyJ".to_string(), "KeyK".to_string(), "KeyL".to_string()]),
        ("drift".to_string(), vec!["KeyJ".to_string()]),
    ]);
    let (b, notes) = Bindings::new(c.controls.as_slice(), &saved);
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert_eq!(b.label(Action::Fire), "SPACE / RMB");
    assert_eq!(b.label(Action::Drift), "J");
}

/// Content validation refuses an unknown action, unreadable or too many inputs, and an input taken twice on a
/// screen.
#[test]
fn validation_checks_controls() {
    let mut files: Vec<(String, String)> = crate::base_pack_files().iter().map(|(p, t)| (p.to_string(), t.to_string())).collect();
    let k = files.iter().position(|(p, _)| p == "controls.ron").unwrap();
    files[k].1 = files[k].1.replace(r#"inputs: ["KeyX"]),
    (id: "focus_next""#, r#"inputs: ["KeyX", "KeyW", "F13"]),
    (id: "focus_next""#);
    files[k].1 = files[k].1.replace(r#"(id: "pause","#, r#"(id: "warp", name: "Warp", summary: "Warps.", section: flight, contexts: [flight], inputs: []),
    (id: "pause","#);
    let pack = crate::Pack::from_files(files.iter().map(|(p, t)| (p.as_str(), t.as_str()))).unwrap();
    let c = crate::Content::merge(&[pack]);
    let problems: Vec<String> = crate::validate(&c).iter().filter(|p| p.kind == Some(crate::Kind::Control)).map(|p| p.message.clone()).collect();
    assert!(problems.iter().any(|m| m.contains("at most 2")), "{problems:?}");
    assert!(problems.iter().any(|m| m.contains("`F13`")), "{problems:?}");
    assert!(problems.iter().any(|m| m.contains("`KeyW` is also")), "{problems:?}");
    assert!(problems.iter().any(|m| m.contains("`warp` is not an action")), "{problems:?}");
}
