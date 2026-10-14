//! The working design at start (restored, or the last saved one), the autosave about a second after a change, and
//! writing it when the drydock is left, a test flight starts or the app exits.

use super::*;

// ---------------------------------------------------------------------------------------------------------
// Start, autosave, leaving

/// At start (the plugin's build, after `DrydockPlugin` put the starter design in): the working design of the
/// last session, else a copy of the starter ship (the career's start ship). A new game (`PlayerCareer::new_game`)
/// docks the starter ship in any case; unsaved changes of the last design are saved first, so BLUEPRINTS keeps
/// them (if that fails, the last design stays). Needs the `Storage` and the career (both plugins come first).
pub(super) fn restore(world: &mut World) -> (DesignSlot, Shelf) {
    let mut shelf = Shelf::default();
    let new_game = world.resource::<PlayerCareer>().new_game();
    let (keep, note) = world.resource_scope(|world, mut storage: Mut<Storage>| {
        if let Some(why) = &storage.fallback {
            shelf.say(format!("designs are kept only until the game closes: {why}"), Tone::Gold);
        }
        let content = &world.resource::<GameContent>().0;
        match library::restore_working(&mut storage, content) {
            Some(Ok((last, slot, note))) if !new_game => {
                if let Some(note) = note {
                    shelf.say(format!("the last design came back, but {note}"), Tone::Gold);
                }
                (Some((last, slot)), None)
            }
            Some(Ok((last, mut slot, _))) if slot.dirty(&last) => {
                let name = last.blueprint().name.clone();
                match library::save(&mut storage, content, &last, &mut slot, now_unix()) {
                    Ok(_) => (None, Some(Note::new("NEW GAME", format!("Your last design, {name}, was saved to BLUEPRINTS."), Tone::Teal))),
                    Err(e) => {
                        let why = format!("{name} couldn't be saved ({e}), so it stays in the drydock.");
                        (Some((last, slot)), Some(Note::new("NEW GAME", why, Tone::Coral)))
                    }
                }
            }
            Some(Ok(_)) => (None, None),
            Some(Err(e)) => {
                shelf.say(format!("the last design didn't come back: {e}"), Tone::Coral);
                (None, None)
            }
            None => (None, None),
        }
    });
    world.resource_mut::<PlayerCareer>().notes.extend(note);
    let mut design = world.resource_mut::<PlayerDesign>();
    if let Some((next, slot)) = keep {
        design.replace(next);
        return (slot, shelf);
    }
    // The first start or a new game: the starter ship (`PlayerDesign::starter`) as "<name> copy", not saved yet.
    let name = format!("{} copy", design.blueprint().name);
    design.edit_header(|h| h.name = name);
    (DesignSlot::opened(None, &design), shelf)
}

/// Writes the working design a second after the last change. Until then the web page keeps it to write if it goes
/// away first (a reload within that second).
pub(super) fn autosave(time: Res<Time<Real>>, mut saver: ResMut<Saver>, mut lib: Lib, mut shelf: ResMut<Shelf>) {
    let now = time.elapsed_secs_f64();
    let revision = lib.design.revision();
    saver.timer.note(revision, now);
    // A save, an open or a delete changes the id the working design is saved under: written at once.
    if saver.timer.due(now) || lib.slot.is_changed() {
        write_working(&mut saver, &mut lib, &mut shelf);
    } else if saver.timer.pending() && saver.kept != Some(revision) {
        saver.kept = Some(revision);
        let Lib { storage, design, slot, .. } = &mut lib;
        storage.keep_on_leave(library::WORKING, || library::working_doc(design, slot));
    }
}

pub(super) fn write_working(saver: &mut Saver, lib: &mut Lib, shelf: &mut Shelf) {
    saver.timer.written();
    if let Err(e) = library::write_working(&mut lib.storage, &lib.design, &lib.slot, now_unix()) {
        shelf.say(format!("autosave failed: {e}"), Tone::Coral);
    }
}

/// Leaving the drydock (a test flight, the BEHAVIOR screen): the working design is written now, the screens close
/// and their layer hides at once ([`sync_shelf`] runs only in the drydock; a layer left visible would block the
/// pointer elsewhere), and the clipboard stops offering the code and taking pastes.
pub(super) fn leave(mut saver: ResMut<Saver>, mut lib: Lib, mut shelf: ResMut<Shelf>, mut clip: ResMut<Clipboard>, mut root: Single<&mut Visibility, With<ShelfRoot>>) {
    if saver.timer.pending() {
        write_working(&mut saver, &mut lib, &mut shelf);
    }
    shelf.page = None;
    shelf.mode = Mode::Browse;
    shelf.code_selected = false;
    shelf.paste_armed = false;
    clip.offer = None;
    clip.accept = false;
    **root = Visibility::Hidden;
}

/// Closing the game (desktop): a waiting autosave is written.
pub(super) fn save_on_exit(mut exit: MessageReader<AppExit>, mut saver: ResMut<Saver>, mut lib: Lib, mut shelf: ResMut<Shelf>) {
    if exit.read().next().is_some() && saver.timer.pending() {
        write_working(&mut saver, &mut lib, &mut shelf);
    }
}

/// Status messages go out once the design check has seen the current revision (else it would overwrite them).
pub(super) fn announce(design: Res<PlayerDesign>, mut status: ResMut<DrydockStatus>, mut shelf: ResMut<Shelf>) {
    if status.revision != Some(design.revision()) || shelf.say.is_none() {
        return;
    }
    if let Some((line, tone)) = shelf.say.take() {
        status.line = line;
        status.tone = tone;
    }
}

/// Files dropped onto the window (desktop) are imported; one larger than any design is not read.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn dropped_files(mut drops: MessageReader<bevy::window::FileDragAndDrop>, mut shelf: ResMut<Shelf>) {
    for d in drops.read() {
        if let bevy::window::FileDragAndDrop::DroppedFile { path_buf, .. } = d {
            let name = path_buf.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let text = match std::fs::metadata(path_buf) {
                Ok(m) if m.len() > share::MAX_TEXT as u64 => Err(share::ShareError::TooLarge.to_string()),
                _ => std::fs::read_to_string(path_buf).map_err(|e| e.to_string()),
            };
            shelf.incoming.push((name, text));
        }
    }
}
