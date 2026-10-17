//! Fight codes (GDR-0092): a fight to re-watch as one line of text. `SGF1:` (space game fight, format 1), then the
//! base64url text (no padding) of the zlib-compressed RON of a [`FightCode`]: the spec, the content packs it was
//! fought with, the rules' fingerprint then ([`rules_fingerprint`](crate::rules_fingerprint)) and how it ended. A
//! fight is re-watched by simulating its spec again, so a code stays small (about 430 characters for the Runner
//! against three raiders, 1–2 kB with an evolved blueprint and its behavior) and shows whether today's rules still
//! give the recorded end. Made like the
//! drydock's share codes (ADR-0031): zlib's checksum catches a damaged code, a newer format says so, and a code is
//! untrusted text, so it is refused unread past [`MAX_PACKED`] and unpacks to at most [`MAX_PAYLOAD`].
//!
//! Voyage codes (GDR-0092 step 9) are the same for a voyage job of the lab: `SGV1:` and a [`VoyageCode`] (the
//! [`VoyageSpec`], the packs, the rules' fingerprint, the run's length and final state hash), re-watched by flying the
//! voyage runner again. [`Code`] reads either.
//!
//! ```
//! let content = data::base_content();
//! let mut spec = arena::FightSpec::new(3, vec![arena::Entry::ship("rim_skiff")], vec![arena::Entry::ship("automaton_raider")]);
//! spec.time_limit = 5.0;
//! let record = arena::run_fight(&content, &spec).unwrap();
//! let code = arena::FightCode::new(&content, arena::rules_fingerprint(&content), spec, record);
//! let text = code.encode();
//! assert!(text.starts_with("SGF1:"));
//! assert_eq!(arena::FightCode::decode(&text).unwrap(), code);
//! ```

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use data::Content;
use serde::{Deserialize, Serialize};

use crate::fight::FightResult;
use crate::spec::FightSpec;
use crate::voyage::VoyageSpec;

/// The code's tag; the full prefix is `SGF<version>:`.
pub const CODE_TAG: &str = "SGF";
/// A voyage code's tag; the full prefix is `SGV<version>:` (its format is [`CODE_VERSION`] too).
pub const VOYAGE_TAG: &str = "SGV";
/// The code format this build writes (it reads every format up to this one).
pub const CODE_VERSION: u32 = 1;
/// The largest compressed payload a code may carry (its base64 decoded). A spec of eight blueprint designs packs to
/// well under 100 kB.
pub const MAX_PACKED: usize = 256 << 10;
/// The largest payload a code may unpack to (zip bombs stop here).
pub const MAX_PAYLOAD: usize = 4 << 20;

/// A fight to re-watch: what was fought, under which rules, and how it ended.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FightCode {
    /// The content packs it was fought with (id, version), to explain a code whose ships this game lacks.
    pub packs: Vec<(String, String)>,
    /// The rules' fingerprint when it was fought ([`rules_fingerprint`](crate::rules_fingerprint)).
    pub rules: u64,
    pub spec: FightSpec,
    /// How it ended: outcome, ticks, both sides' numbers and the final state hash.
    pub record: FightResult,
}

/// A voyage of the lab to re-watch (GDR-0092 step 9): what was flown, under which rules, and how it ended.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoyageCode {
    pub packs: Vec<(String, String)>,
    pub rules: u64,
    pub spec: VoyageSpec,
    /// How it ended: the ticks it ran and the final state hash (`JobEnd::Voyage`).
    pub ticks: u64,
    pub state_hash: u64,
}

/// A fight code or a voyage code.
#[derive(Clone, Debug, PartialEq)]
pub enum Code {
    Fight(FightCode),
    Voyage(VoyageCode),
}

/// Why a text is no fight code this game can read.
#[derive(Clone, Debug, PartialEq)]
pub enum CodeError {
    /// It doesn't start with `SGF<version>:` (or `SGV<version>:`).
    NotACode,
    /// A code of a newer format.
    Newer(u32),
    /// Not base64url, not zlib, or the checksum is wrong: damaged or cut short.
    Damaged,
    /// Over [`MAX_PACKED`] packed or [`MAX_PAYLOAD`] unpacked.
    TooLarge,
    /// The payload is no fight (with RON's message).
    Unreadable(String),
}

impl fmt::Display for CodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodeError::NotACode => {
                write!(f, "that is no fight code (they start with {CODE_TAG}{CODE_VERSION}: or {VOYAGE_TAG}{CODE_VERSION}:)")
            }
            CodeError::Newer(v) => write!(
                f,
                "made by a newer version of the game (fight code format {v}; this one reads {CODE_VERSION})"
            ),
            CodeError::Damaged => f.write_str("the code is damaged or incomplete"),
            CodeError::TooLarge => {
                write!(f, "that is too large for a fight (over {} kB packed)", MAX_PACKED >> 10)
            }
            CodeError::Unreadable(why) => write!(f, "the fight doesn't read: {why}"),
        }
    }
}

impl std::error::Error for CodeError {}

fn packs(content: &Content) -> Vec<(String, String)> {
    content.packs.iter().map(|p| (p.id.clone(), p.version.clone())).collect()
}

/// `<tag><version>:` and the base64url of the zlib-compressed RON of `value`.
fn pack<T: Serialize>(tag: &str, value: &T) -> String {
    let ron = data::ron_options().to_string(value).expect("a code always serialises");
    let packed = miniz_oxide::deflate::compress_to_vec_zlib(ron.as_bytes(), 10);
    format!("{tag}{CODE_VERSION}:{}", URL_SAFE_NO_PAD.encode(packed))
}

/// The tag a text starts with (any case), or `None`.
fn tag_of(text: &str) -> Option<&'static str> {
    let head = text.trim().get(..CODE_TAG.len())?;
    [CODE_TAG, VOYAGE_TAG].into_iter().find(|t| head.eq_ignore_ascii_case(t))
}

/// Reads a code of `tag` (whitespace around it and inside it is ignored, so a wrapped line pastes whole).
fn unpack<T: serde::de::DeserializeOwned>(tag: &str, text: &str) -> Result<T, CodeError> {
    let text = text.trim();
    let rest = text
        .get(..tag.len())
        .filter(|t| t.eq_ignore_ascii_case(tag))
        .map(|_| &text[tag.len()..])
        .ok_or(CodeError::NotACode)?;
    let (version, data) = rest.split_once(':').ok_or(CodeError::NotACode)?;
    let version: u32 = version.parse().map_err(|_| CodeError::NotACode)?;
    if version > CODE_VERSION {
        return Err(CodeError::Newer(version));
    }
    if version == 0 {
        return Err(CodeError::Damaged);
    }
    let data: String = data.chars().filter(|c| !c.is_whitespace()).collect();
    // Four base64 characters carry three bytes.
    if data.len() / 4 * 3 > MAX_PACKED {
        return Err(CodeError::TooLarge);
    }
    let packed = URL_SAFE_NO_PAD
        .decode(data.trim_end_matches('='))
        .map_err(|_| CodeError::Damaged)?;
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&packed, MAX_PAYLOAD)
        .map_err(|e| match e.status {
            miniz_oxide::inflate::TINFLStatus::HasMoreOutput => CodeError::TooLarge,
            _ => CodeError::Damaged,
        })?;
    let ron = String::from_utf8(bytes).map_err(|_| CodeError::Damaged)?;
    data::ron_options()
        .from_str(&ron)
        .map_err(|e| CodeError::Unreadable(e.to_string()))
}

impl FightCode {
    /// The code of a fight fought on `content` under the rules `rules` (its
    /// [`rules_fingerprint`](crate::rules_fingerprint)).
    pub fn new(content: &Content, rules: u64, spec: FightSpec, record: FightResult) -> FightCode {
        FightCode { packs: packs(content), rules, spec, record }
    }

    /// The code as text: `SGF1:` and the base64url of the zlib-compressed RON.
    pub fn encode(&self) -> String {
        pack(CODE_TAG, self)
    }

    /// Reads a code (whitespace around it and inside it is ignored, so a wrapped line pastes whole).
    pub fn decode(text: &str) -> Result<FightCode, CodeError> {
        unpack(CODE_TAG, text)
    }
}

impl VoyageCode {
    /// The code of a voyage flown on `content` under the rules `rules`, `ticks` long, ending with `state_hash`.
    pub fn new(content: &Content, rules: u64, spec: VoyageSpec, ticks: u64, state_hash: u64) -> VoyageCode {
        VoyageCode { packs: packs(content), rules, spec, ticks, state_hash }
    }

    /// The code as text: `SGV1:` and the base64url of the zlib-compressed RON.
    pub fn encode(&self) -> String {
        pack(VOYAGE_TAG, self)
    }

    pub fn decode(text: &str) -> Result<VoyageCode, CodeError> {
        unpack(VOYAGE_TAG, text)
    }
}

impl Code {
    /// Reads a fight code or a voyage code, by its tag.
    pub fn decode(text: &str) -> Result<Code, CodeError> {
        match tag_of(text) {
            Some(VOYAGE_TAG) => VoyageCode::decode(text).map(Code::Voyage),
            _ => FightCode::decode(text).map(Code::Fight),
        }
    }

    pub fn encode(&self) -> String {
        match self {
            Code::Fight(c) => c.encode(),
            Code::Voyage(c) => c.encode(),
        }
    }

    /// The rules' fingerprint it was fought under.
    pub fn rules(&self) -> u64 {
        match self {
            Code::Fight(c) => c.rules,
            Code::Voyage(c) => c.rules,
        }
    }
}
