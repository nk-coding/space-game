//! What the screens' buttons and the drydock's actions do: open and close a page, save, open, duplicate, rename and
//! delete designs, import and export (share codes and files).

use super::*;

/// Opens a page, or closes it when it is open.
pub(super) fn toggle(shelf: &mut Shelf, page: Page, lib: &Lib, content: &Content) {
    if shelf.page == Some(page) {
        shelf.page = None;
        return;
    }
    shelf.page = Some(page);
    shelf.mode = Mode::Browse;
    shelf.code_selected = false;
    shelf.paste_armed = false;
    match page {
        Page::Blueprints => {
            refresh(shelf, lib, content);
            shelf.pick = lib.slot.id.clone().map(Pick::Design);
        }
        Page::Export => shelf.export = Export::of(lib, content),
    }
}

/// Re-reads the saved designs.
pub(super) fn refresh(shelf: &mut Shelf, lib: &Lib, content: &Content) {
    shelf.now = now_unix();
    shelf.templates = content.ships.iter().map(|s| s.id.clone()).collect();
    match library::list_designs(&lib.storage, content) {
        Ok(list) => shelf.designs = list,
        Err(e) => shelf.say(format!("can't list the designs: {e}"), Tone::Coral),
    }
    if let Some(Pick::Design(id)) = &shelf.pick
        && shelf.entry(id).is_none()
    {
        shelf.pick = None;
    }
}

/// The drydock's SAVE / EXPORT / BLUEPRINTS and the ⋯ menu's actions, the screens' buttons, keys, pasted text and
/// picked files.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(in crate::drydock) fn apply(
    mut actions: MessageReader<DrydockAction>,
    mut clicks: MessageReader<UiClick>,
    buttons: Query<(&ShelfButton, Option<&Pill>)>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    boxes: Query<(&Hot, Has<CodeBox>), Or<(With<CodeBox>, With<PasteBox>)>>,
    mut rename: Single<&mut TextField, With<RenameField>>,
    fields: Query<&TextField, Without<RenameField>>,
    mut clip: ResMut<Clipboard>,
    mut saver: ResMut<Saver>,
    mut shelf: ResMut<Shelf>,
    mut lib: Lib,
    content: Res<GameContent>,
) {
    let content = &content.0;
    // A rename ends when its field lets go of the keyboard: Esc cancels, anything else renames.
    if let Mode::Rename(id) = shelf.mode.clone()
        && !rename.focused
    {
        shelf.mode = Mode::Browse;
        let name = rename.value.trim().to_string();
        if !keys.just_pressed(KeyCode::Escape) && !name.is_empty() {
            rename_design(&mut shelf, &mut lib, content, &id, &name);
        }
    }
    let field_keys = saver.field_had_keys;
    saver.field_had_keys = rename.focused || fields.iter().any(|f| f.focused);

    let mut todo: Vec<ShelfButton> = Vec::new();
    for a in actions.read() {
        match a {
            DrydockAction::Save => {
                let (line, tone) = lib.save(content);
                shelf.say(line, tone);
                if shelf.page == Some(Page::Blueprints) {
                    refresh(&mut shelf, &lib, content);
                }
            }
            DrydockAction::Blueprints => toggle(&mut shelf, Page::Blueprints, &lib, content),
            DrydockAction::Export => toggle(&mut shelf, Page::Export, &lib, content),
            // The ⋯ menu: the screens' buttons, whether a screen is open or not.
            DrydockAction::Rare(r) => {
                let b = match r {
                    Rare::NewDesign => ShelfButton::New(lib.design.frame()),
                    Rare::Frame(f) => ShelfButton::Frame(*f),
                    Rare::ImportFile => ShelfButton::ImportFile,
                    Rare::CopyCode | Rare::Download => {
                        shelf.export = Export::of(&lib, content);
                        if *r == Rare::CopyCode { ShelfButton::Copy } else { ShelfButton::Download }
                    }
                };
                press(b, &mut shelf, &mut lib, content, &mut rename, &mut clip);
            }
            _ => {}
        }
    }
    for c in clicks.read() {
        if let Ok((b, pill)) = buttons.get(c.0)
            && !pill.is_some_and(|p| p.disabled)
        {
            todo.push(b.clone());
        }
    }
    // Touched only when files came (a write marks the shelf changed, and its texts are made again then).
    #[cfg(target_arch = "wasm32")]
    {
        let picked = share::take_picked();
        if !picked.is_empty() {
            shelf.incoming.extend(picked);
        }
    }
    if !shelf.incoming.is_empty() {
        for (file, text) in std::mem::take(&mut shelf.incoming) {
            match text {
                Ok(text) => import(&mut shelf, &mut lib, content, &text, &format!("{file}: ")),
                Err(e) => shelf.say(format!("{file} doesn't read: {e}"), Tone::Coral),
            }
        }
    }
    if let Some(done) = clip.copied.take() {
        match done {
            Ok(()) => shelf.say("share code copied", Tone::Teal),
            Err(e) => shelf.say(format!("copy failed ({e}): click the code and press ctrl+c"), Tone::Coral),
        }
    }
    let Some(page) = shelf.page else {
        clip.offer = None;
        clip.accept = false;
        return;
    };
    // The design changed under the export page (undo, redo): the file and the code follow.
    if page == Page::Export && shelf.export.revision != lib.design.revision() {
        shelf.export = Export::of(&lib, content);
    }
    // A press anywhere else lets go of the code box and the paste box.
    if mouse.just_pressed(MouseButton::Left) {
        for (hot, code) in &boxes {
            if !hot.hovered {
                if code {
                    shelf.code_selected = false;
                } else {
                    shelf.paste_armed = false;
                }
            }
        }
    }
    if keys.just_pressed(KeyCode::Escape) && !field_keys {
        if shelf.code_selected || shelf.paste_armed {
            shelf.code_selected = false;
            shelf.paste_armed = false;
        } else if shelf.mode != Mode::Browse {
            shelf.mode = Mode::Browse;
        } else {
            shelf.page = None;
        }
    }
    for b in todo {
        press(b, &mut shelf, &mut lib, content, &mut rename, &mut clip);
    }
    for text in clip.take_pasted() {
        if shelf.page == Some(Page::Blueprints) && shelf.paste_armed {
            shelf.paste_armed = false;
            import(&mut shelf, &mut lib, content, &text, "");
        }
    }
    clip.offer = (page == Page::Export && shelf.code_selected).then(|| shelf.export.code.clone());
    clip.accept = page == Page::Blueprints && shelf.paste_armed;
}

pub(super) fn import(shelf: &mut Shelf, lib: &mut Lib, content: &Content, text: &str, from: &str) {
    match lib.import(content, text) {
        Ok(msg) => {
            shelf.say(msg, Tone::Teal);
            refresh(shelf, lib, content);
            shelf.pick = lib.slot.id.clone().map(Pick::Design);
        }
        Err(e) => shelf.say(format!("{from}not imported: {e}"), Tone::Coral),
    }
}

pub(super) fn rename_design(shelf: &mut Shelf, lib: &mut Lib, content: &Content, id: &str, name: &str) {
    if let Err(e) = library::rename(&mut lib.storage, id, name, now_unix()) {
        shelf.say(format!("not renamed: {e}"), Tone::Coral);
        return;
    }
    if lib.slot.id.as_deref() == Some(id) {
        // The design in the drydock follows; it stays unsaved only if it had other changes.
        let clean = !lib.slot.dirty(&lib.design);
        lib.design.edit_header(|h| h.name = name.to_string());
        if clean {
            *lib.slot = DesignSlot::opened(Some(id.to_string()), &lib.design);
        }
    }
    shelf.say(format!("renamed to {name}"), Tone::Teal);
    refresh(shelf, lib, content);
}

/// One button of the screens.
pub(super) fn press(b: ShelfButton, shelf: &mut Shelf, lib: &mut Lib, content: &Content, rename: &mut TextField, clip: &mut Clipboard) {
    match b {
        ShelfButton::Close => shelf.page = None,
        ShelfButton::Row(list, k) => {
            let i = shelf.offset[list] + k;
            shelf.pick = match list {
                0 => shelf.designs.get(i).map(|e| Pick::Design(e.id.clone())),
                _ => shelf.templates.get(i).map(|id| Pick::Template(id.clone())),
            };
            shelf.mode = Mode::Browse;
        }
        ShelfButton::Open => {
            let opened = match shelf.pick.clone() {
                Some(Pick::Design(id)) if lib.slot.id.as_deref() == Some(id.as_str()) => {
                    shelf.say("it is in the drydock already", Tone::Label);
                    return;
                }
                Some(Pick::Design(id)) => library::open_saved(&lib.storage, content, &id).map(|d| (DesignSlot::opened(Some(id), &d), d)),
                Some(Pick::Template(id)) => {
                    library::open_template(content, &id, &lib.names(shelf)).map(|d| (DesignSlot::opened(None, &d), d))
                }
                None => return,
            };
            match opened.and_then(|(slot, d)| lib.switch(content, d, slot)) {
                Ok(kept) => {
                    let name = lib.design.blueprint().name.clone();
                    let kept = kept.map_or(String::new(), |k| format!("{k} saved  ·  "));
                    shelf.say(format!("{kept}opened {name}"), Tone::Teal);
                    shelf.page = None;
                }
                Err(e) => shelf.say(format!("not opened: {e}"), Tone::Coral),
            }
        }
        ShelfButton::Duplicate => {
            let names = lib.names(shelf);
            let now = now_unix();
            let made = match shelf.pick.clone() {
                Some(Pick::Design(id)) => library::duplicate(&mut lib.storage, content, &id, &names, now),
                Some(Pick::Template(id)) => library::open_template(content, &id, &names).and_then(|copy| {
                    library::save(&mut lib.storage, content, &copy, &mut DesignSlot::default(), now).map_err(|e| e.to_string())
                }),
                None => return,
            };
            match made {
                Ok(id) => {
                    refresh(shelf, lib, content);
                    let name = shelf.entry(&id).map_or(id.clone(), |e| e.name.clone());
                    shelf.say(format!("saved a copy: {name}"), Tone::Teal);
                    shelf.top[0] = true;
                    shelf.pick = Some(Pick::Design(id));
                }
                Err(e) => shelf.say(format!("no copy made: {e}"), Tone::Coral),
            }
        }
        ShelfButton::Rename => {
            if let Some(Pick::Design(id)) = &shelf.pick
                && let Some(e) = shelf.entry(id)
            {
                rename.value = e.name.clone();
                rename.focus();
                shelf.mode = Mode::Rename(id.clone());
            }
        }
        ShelfButton::Delete => {
            if let Some(Pick::Design(id)) = &shelf.pick {
                shelf.mode = Mode::Delete(id.clone());
            }
        }
        ShelfButton::ConfirmDelete => {
            let Mode::Delete(id) = std::mem::take(&mut shelf.mode) else { return };
            match lib.storage.delete::<library::DesignDoc>(&id) {
                Ok(()) => {
                    let name = shelf.entry(&id).map_or(id.clone(), |e| e.name.clone());
                    if lib.slot.id.as_deref() == Some(id.as_str()) {
                        // The design stays in the drydock, unsaved; it is not saved again unless it changes.
                        *lib.slot = DesignSlot::opened(None, &lib.design);
                    }
                    shelf.say(format!("deleted {name}"), Tone::Teal);
                    refresh(shelf, lib, content);
                }
                Err(e) => shelf.say(format!("not deleted: {e}"), Tone::Coral),
            }
        }
        ShelfButton::KeepDelete => shelf.mode = Mode::Browse,
        ShelfButton::New(frame) => {
            let made = library::new_design(content, frame, lib.design.blueprint(), &lib.names(shelf)).and_then(|d| {
                let slot = DesignSlot::opened(None, &d);
                lib.switch(content, d, slot)
            });
            match made {
                Ok(kept) => {
                    let kept = kept.map_or(String::new(), |k| format!("{k} saved  ·  "));
                    shelf.say(format!("{kept}new design in the {}", frame_name(content, frame)), Tone::Teal);
                    shelf.page = None;
                }
                Err(e) => shelf.say(format!("no new design: {e}"), Tone::Coral),
            }
        }
        ShelfButton::Frame(frame) => {
            if lib.design.frame() == frame {
                return;
            }
            let size = lib.design.export().size;
            // The camera fits the new frame by itself (`view::camera` follows the frame, undo included).
            match lib.design.edit(|d| d.set_frame(frame)) {
                Ok(_) => {
                    shelf.say(format!("moved to the {}", frame_name(content, frame)), Tone::Teal);
                }
                Err(_) => shelf.say(
                    format!("doesn't fit the {} ({}×{}): the design is {}×{}", frame_name(content, frame), frame.0, frame.1, size.0, size.1),
                    Tone::Coral,
                ),
            }
        }
        ShelfButton::ImportFile => {
            #[cfg(target_arch = "wasm32")]
            {
                if let Err(e) = share::pick_file() {
                    shelf.say(format!("no file chooser: {e}"), Tone::Coral);
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                shelf.say("drop a .ron file onto the window to import it", Tone::Label);
            }
        }
        ShelfButton::Download => match share::write_export(&shelf.export.file, &shelf.export.ron) {
            Ok(where_) => shelf.say(where_, Tone::Teal),
            Err(e) => shelf.say(format!("not exported: {e}"), Tone::Coral),
        },
        ShelfButton::Copy => {
            shelf.code_selected = true;
            clip.copy = Some(shelf.export.code.clone());
        }
        ShelfButton::CodeBox => shelf.code_selected = true,
        ShelfButton::PasteBox => shelf.paste_armed = true,
    }
}
