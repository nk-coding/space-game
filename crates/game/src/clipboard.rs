//! Copy and paste of UI text through the system clipboard, per platform (written 13 Oct 2026 for share codes).
//! Screens talk to the [`Clipboard`] resource only:
//!
//! * `offer`: the text Ctrl+C copies this frame (a selected read-only text, such as a share code);
//! * `accept`: a paste target has the focus, so Ctrl+V delivers text to [`Clipboard::take_pasted`];
//! * `copy`: copy this text now (a COPY button); the outcome arrives in `copied`.
//!
//! Platforms:
//! * **Web**: winit cancels the default action of every key on the canvas, so the browser would never copy or
//!   paste. While something is offered or accepted, a capturing `keydown` listener on the window keeps Ctrl+C /
//!   Ctrl+V (Cmd on macOS) away from the canvas; the browser then fires `copy` / `paste` events, whose
//!   listeners hand the offered text over or take the pasted one. COPY uses `document.execCommand("copy")`,
//!   which triggers the same `copy` listener, with `navigator.clipboard` as a second try. Neither needs a
//!   secure context (the dev build is served over plain http), only the user's click or key.
//! * **Desktop**: `arboard` (Ctrl+C, Ctrl+V and COPY read and write the system clipboard directly).
//! * **Android**: nothing yet (the copy button reports it; pasting comes with the phone UI).

use bevy::prelude::*;

use crate::ui_kit::UiInputSet;

/// The clipboard as screens see it.
#[derive(Resource, Default, Debug)]
pub struct Clipboard {
    /// The text Ctrl+C copies, set by the screen every frame (`None`: nothing is selected).
    pub offer: Option<String>,
    /// A paste target has the focus: Ctrl+V pastes.
    pub accept: bool,
    /// Copy this text now (a button); the platform takes it and puts the outcome into `copied`.
    pub copy: Option<String>,
    /// The outcome of the last `copy`: `Err` says why it failed.
    pub copied: Option<Result<(), String>>,
    pasted: Vec<String>,
}

impl Clipboard {
    /// The texts pasted since the last call.
    pub fn take_pasted(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pasted)
    }
}

pub struct ClipboardPlugin;

impl Plugin for ClipboardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Clipboard>().add_systems(Update, platform::sync.after(UiInputSet));
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use std::cell::{Cell, RefCell};

    use bevy::prelude::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    use super::Clipboard;

    thread_local! {
        static INSTALLED: Cell<bool> = const { Cell::new(false) };
        static OFFER: RefCell<Option<String>> = const { RefCell::new(None) };
        static ACCEPT: Cell<bool> = const { Cell::new(false) };
        static PASTED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    /// Hands the screen's offer and focus to the listeners, the pasted text back, and runs a requested copy.
    pub fn sync(mut clip: ResMut<Clipboard>) {
        if !INSTALLED.get() {
            INSTALLED.set(true);
            if let Err(e) = install() {
                warn!("clipboard: {e}");
            }
        }
        if let Some(text) = clip.copy.take() {
            clip.copied = Some(copy(&text));
        }
        OFFER.set(clip.offer.clone());
        ACCEPT.set(clip.accept);
        let pasted = PASTED.with_borrow_mut(std::mem::take);
        clip.pasted.extend(pasted);
    }

    fn js(e: wasm_bindgen::JsValue) -> String {
        e.as_string().unwrap_or_else(|| format!("{e:?}"))
    }

    /// `execCommand("copy")` fires the `copy` event, whose listener supplies the text; the async clipboard API
    /// is the second try (secure contexts only).
    fn copy(text: &str) -> Result<(), String> {
        let window = web_sys::window().ok_or("no window")?;
        let document: web_sys::HtmlDocument = window.document().ok_or("no document")?.unchecked_into();
        let before = OFFER.replace(Some(text.to_string()));
        let done = document.exec_command("copy").unwrap_or(false);
        OFFER.set(before);
        if done {
            return Ok(());
        }
        if window.is_secure_context() {
            let _ = window.navigator().clipboard().write_text(text);
            return Ok(());
        }
        Err("the browser refused to copy".into())
    }

    fn install() -> Result<(), String> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        // Keep Ctrl/Cmd + C and V from the canvas (winit cancels them) while there is something to copy or a
        // place to paste, so the browser runs its copy and paste commands.
        let keys = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(|e: web_sys::KeyboardEvent| {
            if !(e.ctrl_key() || e.meta_key()) {
                return;
            }
            let wanted = match e.key().as_str() {
                "c" | "C" => OFFER.with_borrow(Option::is_some),
                "v" | "V" => ACCEPT.get(),
                _ => false,
            };
            if wanted {
                e.stop_immediate_propagation();
            }
        });
        let capture = web_sys::AddEventListenerOptions::new();
        capture.set_capture(true);
        window
            .add_event_listener_with_callback_and_add_event_listener_options("keydown", keys.as_ref().unchecked_ref(), &capture)
            .map_err(js)?;
        // Without a selection the browser only copies or pastes when `beforecopy` / `beforepaste` are cancelled.
        let enable = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
            let wanted = if e.type_() == "beforecopy" { OFFER.with_borrow(Option::is_some) } else { ACCEPT.get() };
            if wanted {
                e.prevent_default();
            }
        });
        let on_copy = Closure::<dyn FnMut(web_sys::ClipboardEvent)>::new(|e: web_sys::ClipboardEvent| {
            OFFER.with_borrow(|offer| {
                if let (Some(text), Some(data)) = (offer, e.clipboard_data())
                    && data.set_data("text/plain", text).is_ok()
                {
                    e.prevent_default();
                }
            });
        });
        let on_paste = Closure::<dyn FnMut(web_sys::ClipboardEvent)>::new(|e: web_sys::ClipboardEvent| {
            if !ACCEPT.get() {
                return;
            }
            if let Some(text) = e.clipboard_data().and_then(|d| d.get_data("text/plain").ok()) {
                PASTED.with_borrow_mut(|p| p.push(text));
                e.prevent_default();
            }
        });
        for (event, f) in [("beforecopy", enable.as_ref()), ("beforepaste", enable.as_ref()), ("copy", on_copy.as_ref()), ("paste", on_paste.as_ref())] {
            document.add_event_listener_with_callback(event, f.unchecked_ref()).map_err(js)?;
        }
        // The listeners live as long as the page.
        keys.forget();
        enable.forget();
        on_copy.forget();
        on_paste.forget();
        Ok(())
    }
}

#[cfg(target_os = "android")]
mod platform {
    use bevy::prelude::*;

    use super::Clipboard;

    /// No clipboard yet: a requested copy says so.
    pub fn sync(mut clip: ResMut<Clipboard>) {
        if clip.copy.take().is_some() {
            clip.copied = Some(Err("no clipboard on this platform yet".into()));
        }
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
mod platform {
    use bevy::prelude::*;

    use super::Clipboard;

    /// Ctrl+C copies the offer, Ctrl+V pastes into the focused target, a requested copy is written.
    pub fn sync(keys: Res<ButtonInput<KeyCode>>, mut clip: ResMut<Clipboard>, mut board: Local<Option<arboard::Clipboard>>) {
        let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]);
        let mut copy = clip.copy.take().map(|t| (t, true));
        if ctrl && keys.just_pressed(KeyCode::KeyC)
            && let Some(offer) = &clip.offer
        {
            copy = Some((offer.clone(), false));
        }
        let paste = ctrl && keys.just_pressed(KeyCode::KeyV) && clip.accept;
        if copy.is_none() && !paste {
            return;
        }
        if board.is_none() {
            match arboard::Clipboard::new() {
                Ok(b) => *board = Some(b),
                Err(e) => {
                    if copy.is_some_and(|c| c.1) {
                        clip.copied = Some(Err(e.to_string()));
                    }
                    return;
                }
            }
        }
        let Some(b) = board.as_mut() else { return };
        if let Some((text, asked)) = copy {
            let done = b.set_text(text).map_err(|e| e.to_string());
            if asked {
                clip.copied = Some(done);
            }
        }
        if paste && let Ok(text) = b.get_text() {
            clip.pasted.push(text);
        }
    }
}
