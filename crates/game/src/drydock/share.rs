//! Sharing designs with other players: `.ron` files in the content ship format and share codes, and importing
//! both (written 13 Oct 2026; Niklas: "a sharable format in addition").
//!
//! * **Export file**: the design trimmed to its parts (`PlayerDesign::export`) under its id, written by
//!   `data::blueprint_to_ron`, so it can go into a pack's `ships/` folder as is. Web: a download
//!   (`<id>.ron`); desktop: `<data dir>/exports/<id>.ron` (the path shows in the status line).
//! * **Share code**: `SGD1:` (space game design, share format 1) followed by the base64url text (no padding) of
//!   the zlib-compressed payload: RON `(packs: [("base", "0.1.0"), ...], ship: (...))`, the content packs the
//!   design was made with (id, version) and the exported ship. zlib's checksum catches damaged codes; the pack
//!   list explains missing content. About 900 characters for the Halvex Runner.
//! * **Import** ([`read`], [`check`]): a share code, or RON text (a `.ron` file, or pasted). A newer share format,
//!   a damaged code, text that is no blueprint, and parts or materials this game doesn't have give a message
//!   (with the packs the design was made with) instead of a crash; an unknown faction or behavior is replaced.
//!   Web: a file input; desktop: drop a `.ron` file onto the window; everywhere: paste a code or RON text.

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use data::{Blueprint, Content};
use serde::Deserialize;

/// The share code's tag; the full prefix is `SGD<version>:`.
const TAG: &str = "SGD";
/// The share format this build writes and reads.
pub const CODE_VERSION: u32 = 1;
/// The largest payload a code may unpack to (a full 64 × 64 frame is about 250 kB of RON).
const MAX_PAYLOAD: usize = 4 << 20;

/// A design that came from outside: the ship and the packs it was made with (empty for plain RON text).
#[derive(Clone, Debug, PartialEq)]
pub struct Shared {
    pub ship: Blueprint,
    pub packs: Vec<(String, String)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    packs: Vec<(String, String)>,
    ship: Blueprint,
}

/// Why an import failed.
#[derive(Clone, Debug, PartialEq)]
pub enum ShareError {
    /// Neither a share code nor blueprint text.
    NotADesign,
    /// A share code of a newer format.
    Newer(u32),
    /// The code is damaged or incomplete (not base64url, not zlib, wrong checksum, too large).
    Damaged,
    /// The design inside doesn't read.
    Unreadable(String),
    /// The design uses parts or materials this game doesn't have.
    Missing { what: Vec<String>, packs: String },
}

impl fmt::Display for ShareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShareError::NotADesign => f.write_str("that is neither a share code nor a blueprint"),
            ShareError::Newer(v) => write!(f, "made by a newer version of the game (share format {v}; this one reads {CODE_VERSION})"),
            ShareError::Damaged => f.write_str("the code is damaged or incomplete"),
            ShareError::Unreadable(why) => write!(f, "the design doesn't read: {why}"),
            ShareError::Missing { what, packs } => write!(f, "it uses {} this game doesn't have{packs}", what.join(", ")),
        }
    }
}

/// The share code of a blueprint (export it first: trimmed, under its id), made with `content`'s packs.
pub fn encode(content: &Content, ship: &Blueprint) -> String {
    let packs: Vec<String> = content
        .packs
        .iter()
        .map(|p| format!("({}, {})", ron::to_string(&p.id).unwrap_or_default(), ron::to_string(&p.version).unwrap_or_default()))
        .collect();
    let ship = data::blueprint_to_ron(ship).trim_end().replace('\n', "\n    ");
    let payload = format!("(\n    packs: [{}],\n    ship: {ship},\n)\n", packs.join(", "));
    let packed = miniz_oxide::deflate::compress_to_vec_zlib(payload.as_bytes(), 10);
    format!("{TAG}{CODE_VERSION}:{}", URL_SAFE_NO_PAD.encode(packed))
}

/// Reads a share code or blueprint RON text (whitespace around it, and inside a code, is ignored).
pub fn read(text: &str) -> Result<Shared, ShareError> {
    let text = text.trim();
    if text.get(..TAG.len()).is_some_and(|t| t.eq_ignore_ascii_case(TAG)) {
        return decode(&text[TAG.len()..]);
    }
    if text.starts_with('(') || text.starts_with("//") {
        let ship = data::parse_blueprint(text).map_err(|p| ShareError::Unreadable(super::library::problems_text(&p)))?;
        return Ok(Shared { ship, packs: Vec::new() });
    }
    Err(ShareError::NotADesign)
}

/// `<version>:<base64url>` after the tag.
fn decode(rest: &str) -> Result<Shared, ShareError> {
    let (version, data) = rest.split_once(':').ok_or(ShareError::Damaged)?;
    let version: u32 = version.parse().map_err(|_| ShareError::Damaged)?;
    if version > CODE_VERSION {
        return Err(ShareError::Newer(version));
    }
    if version != CODE_VERSION {
        return Err(ShareError::Damaged);
    }
    let data: String = data.chars().filter(|c| !c.is_whitespace()).collect();
    let packed = URL_SAFE_NO_PAD.decode(data.trim_end_matches('=')).map_err(|_| ShareError::Damaged)?;
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&packed, MAX_PAYLOAD).map_err(|_| ShareError::Damaged)?;
    let text = String::from_utf8(bytes).map_err(|_| ShareError::Damaged)?;
    let payload: Payload = data::ron_options().from_str(&text).map_err(|e| ShareError::Unreadable(e.to_string()))?;
    Ok(Shared { ship: payload.ship, packs: payload.packs })
}

/// Checks a shared design against the content. Parts and materials it doesn't have refuse it (naming the packs
/// it was made with that differ from the loaded ones); an unknown faction becomes `faction` (the player's), an
/// unknown behavior is dropped. Returns the blueprint to open and notes about what was replaced.
pub fn check(content: &Content, shared: Shared, faction: &str) -> Result<(Blueprint, Vec<String>), ShareError> {
    let mut ship = shared.ship;
    let mut missing: Vec<String> = Vec::new();
    let mut add = |what: String| {
        if !missing.contains(&what) {
            missing.push(what);
        }
    };
    for p in &ship.parts {
        if !content.parts.contains(&p.part) {
            add(format!("part {}", p.part));
        }
        if let Some(m) = p.material.as_ref().filter(|m| !content.materials.contains(m)) {
            add(format!("material {m}"));
        }
    }
    if !missing.is_empty() {
        return Err(ShareError::Missing { what: missing, packs: pack_note(content, &shared.packs) });
    }
    let mut notes = Vec::new();
    if !content.factions.contains(&ship.faction) {
        notes.push(format!("unknown faction {}, flies as {faction}", ship.faction));
        ship.faction = faction.to_string();
    }
    if let Some(b) = ship.behavior.take_if(|b| !content.behaviors.contains(b)) {
        notes.push(format!("unknown behavior {b} dropped"));
    }
    if ship.name.trim().is_empty() {
        ship.name = "Imported design".into();
    }
    Ok((ship, notes))
}

/// " (made with cool_mod 1.2, base 0.2.0)": the packs of the design that aren't loaded in that version.
fn pack_note(content: &Content, packs: &[(String, String)]) -> String {
    let differ: Vec<String> = packs
        .iter()
        .filter(|(id, v)| !content.packs.iter().any(|p| p.id == *id && p.version == *v))
        .map(|(id, v)| format!("{id} {v}"))
        .collect();
    if differ.is_empty() { String::new() } else { format!(" (made with {})", differ.join(", ")) }
}

/// Exports go to a download on the web.
#[cfg(target_arch = "wasm32")]
pub fn write_export(file_name: &str, text: &str) -> Result<String, String> {
    web::download(file_name, text).map(|()| format!("downloaded {file_name}"))
}

/// Exports go to `<data dir>/exports/` on desktop.
#[cfg(not(target_arch = "wasm32"))]
pub fn write_export(file_name: &str, text: &str) -> Result<String, String> {
    let dir = crate::storage::data_dir().ok_or("no user data directory on this platform")?.join("exports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(file_name);
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(format!("written to {}", path.display()))
}

/// Opens the browser's file chooser; the file's text arrives through [`take_picked`].
#[cfg(target_arch = "wasm32")]
pub use web::{pick_file, take_picked};

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    thread_local! {
        /// The file input (made once) and the files read from it: (name, text or why not).
        static INPUT: RefCell<Option<web_sys::HtmlInputElement>> = const { RefCell::new(None) };
        static PICKED: RefCell<Vec<(String, Result<String, String>)>> = const { RefCell::new(Vec::new()) };
    }

    fn js(e: wasm_bindgen::JsValue) -> String {
        e.as_string().unwrap_or_else(|| format!("{e:?}"))
    }

    /// A download of `text` as `file_name` (a Blob behind a temporary link).
    pub fn download(file_name: &str, text: &str) -> Result<(), String> {
        let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
        let parts = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text));
        let props = web_sys::BlobPropertyBag::new();
        props.set_type("text/plain;charset=utf-8");
        let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &props).map_err(js)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js)?;
        let a: web_sys::HtmlAnchorElement = document.create_element("a").map_err(js)?.unchecked_into();
        a.set_href(&url);
        a.set_download(file_name);
        a.click();
        web_sys::Url::revoke_object_url(&url).map_err(js)
    }

    /// Opens the file chooser for `.ron` files.
    pub fn pick_file() -> Result<(), String> {
        INPUT.with_borrow_mut(|input| {
            if input.is_none() {
                *input = Some(make_input()?);
            }
            let i = input.as_ref().expect("made above");
            i.set_value("");
            i.click();
            Ok(())
        })
    }

    /// The files read since the last call: (name, text or why not).
    pub fn take_picked() -> Vec<(String, Result<String, String>)> {
        PICKED.with_borrow_mut(std::mem::take)
    }

    fn make_input() -> Result<web_sys::HtmlInputElement, String> {
        let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
        let input: web_sys::HtmlInputElement = document.create_element("input").map_err(js)?.unchecked_into();
        input.set_type("file");
        input.set_accept(".ron,text/plain");
        input.style().set_property("display", "none").map_err(js)?;
        document.body().ok_or("no body")?.append_child(&input).map_err(js)?;
        let on_change = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
            let Some(input) = e.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) else { return };
            let Some(file) = input.files().and_then(|f| f.get(0)) else { return };
            let name = file.name();
            let Ok(reader) = web_sys::FileReader::new() else { return };
            let on_load = Closure::<dyn FnMut(web_sys::Event)>::once(move |e: web_sys::Event| {
                let text = e
                    .target()
                    .and_then(|t| t.dyn_into::<web_sys::FileReader>().ok())
                    .and_then(|r| r.result().ok())
                    .and_then(|v| v.as_string())
                    .ok_or_else(|| "the file doesn't read as text".to_string());
                PICKED.with_borrow_mut(|p| p.push((name, text)));
            });
            reader.set_onload(Some(on_load.as_ref().unchecked_ref()));
            // The reader holds the only reference to the callback until it fires once.
            on_load.forget();
            if let Err(e) = reader.read_as_text(&file) {
                PICKED.with_borrow_mut(|p| p.push((file.name(), Err(js(e)))));
            }
        });
        input.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref()).map_err(js)?;
        // The input lives as long as the page.
        on_change.forget();
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner(content: &Content) -> Blueprint {
        content.ships.get("halvex_runner").unwrap().clone()
    }

    #[test]
    fn codes_round_trip() {
        let content = data::base_content();
        for ship in content.ships.iter() {
            let code = encode(&content, ship);
            assert!(code.starts_with("SGD1:"), "{code}");
            assert!(code[5..].chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
            let back = read(&code).unwrap();
            let want = Blueprint { source: Default::default(), ..ship.clone() };
            assert_eq!(back, Shared { ship: want, packs: vec![("base".into(), "0.1.0".into())] });
        }
        let code = encode(&content, &runner(&content));
        assert!(code.len() < 1400, "{} characters", code.len());
        // Wrapped by a chat program, lower-case tag, padding: still reads.
        let wrapped = format!("  sgd1:{}\n{}==\n", &code[5..40], &code[40..]);
        assert_eq!(read(&wrapped).unwrap().ship.parts, runner(&content).parts);
        // Strings with quotes, backslashes and unicode survive.
        let odd = Blueprint { name: "Läufer \"Mk\" \\ II ★".into(), ..runner(&content) };
        assert_eq!(read(&encode(&content, &odd)).unwrap().ship.name, odd.name);
    }

    #[test]
    fn damaged_and_newer_codes_say_so() {
        let content = data::base_content();
        let code = encode(&content, &runner(&content));
        // A changed character: base64 still decodes, zlib's checksum or the inflater refuses it.
        let mut bytes = code.clone().into_bytes();
        let k = bytes.len() / 2;
        bytes[k] = if bytes[k] == b'A' { b'B' } else { b'A' };
        assert_eq!(read(&String::from_utf8(bytes).unwrap()), Err(ShareError::Damaged));
        // Cut short, not base64, no version, version 0.
        assert_eq!(read(&code[..code.len() - 30]), Err(ShareError::Damaged));
        assert_eq!(read("SGD1:not*base64!"), Err(ShareError::Damaged));
        assert_eq!(read("SGD:abc"), Err(ShareError::Damaged));
        assert_eq!(read("SGDx"), Err(ShareError::Damaged));
        assert_eq!(read("SGD0:abc"), Err(ShareError::Damaged));
        // A newer format is named as such, whatever follows.
        let newer = read("SGD7:whatever").unwrap_err();
        assert_eq!(newer, ShareError::Newer(7));
        assert!(newer.to_string().contains("newer version"));
        // Valid zlib around something that is no payload.
        let junk = format!("SGD1:{}", URL_SAFE_NO_PAD.encode(miniz_oxide::deflate::compress_to_vec_zlib(b"(packs: [])", 6)));
        assert!(matches!(read(&junk), Err(ShareError::Unreadable(_))));
        assert_eq!(read("hello"), Err(ShareError::NotADesign));
        assert_eq!(read(""), Err(ShareError::NotADesign));
    }

    #[test]
    fn ron_text_imports_too() {
        let content = data::base_content();
        let text = data::blueprint_to_ron(&runner(&content));
        assert_eq!(read(&format!("// shared\n{text}")).unwrap().ship.parts, runner(&content).parts);
        let e = read("(id: \"x\", name: 3)").unwrap_err();
        assert!(matches!(&e, ShareError::Unreadable(why) if why.contains("line 1")), "{e}");
    }

    #[test]
    fn missing_content_is_explained_and_unknown_factions_replaced() {
        let content = data::base_content();
        let mut ship = runner(&content);
        ship.parts[0].part = "plasma_lance".into();
        ship.parts[1].material = Some("starglass".into());
        ship.parts[2].part = "plasma_lance".into();
        let packs = vec![("base".to_string(), "0.1.0".to_string()), ("lances".to_string(), "2.0".to_string())];
        let e = check(&content, Shared { ship: ship.clone(), packs }, "ship_mind").unwrap_err();
        let msg = e.to_string();
        assert_eq!(e, ShareError::Missing { what: vec!["part plasma_lance".into(), "material starglass".into()], packs: " (made with lances 2.0)".into() });
        assert!(msg.contains("plasma_lance") && msg.contains("lances 2.0"), "{msg}");
        // Without pack information (plain RON) the message just names what is missing.
        let e = check(&content, Shared { ship, packs: Vec::new() }, "ship_mind").unwrap_err();
        assert!(!e.to_string().contains("made with"));
        // Unknown faction and behavior: replaced, with notes.
        let mut ship = runner(&content);
        ship.faction = "pirates".into();
        ship.behavior = Some("ambush".into());
        ship.name = "  ".into();
        let (fixed, notes) = check(&content, Shared { ship, packs: Vec::new() }, "ship_mind").unwrap();
        assert_eq!((fixed.faction.as_str(), fixed.behavior.as_deref(), fixed.name.as_str()), ("ship_mind", None, "Imported design"));
        assert_eq!(notes.len(), 2);
    }
}
