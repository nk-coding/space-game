//! Fight codes (GDR-0092): a fight to re-watch as one line of text. `SGF1:` (space game fight, format 1), then the
//! base64url text (no padding) of the zlib-compressed RON of a [`FightCode`]: the spec, the content packs it was
//! fought with, the rules' fingerprint then ([`rules_fingerprint`](crate::rules_fingerprint)) and how it ended. A
//! fight is re-watched by simulating its spec again, so a code stays small (about 430 characters for the Runner
//! against three raiders, 1–2 kB with an evolved blueprint and its behavior) and shows whether today's rules still
//! give the recorded end. Made like the
//! drydock's share codes (ADR-0031): zlib's checksum catches a damaged code, a newer format says so, and a code is
//! untrusted text, so it is refused unread past [`MAX_PACKED`] and unpacks to at most [`MAX_PAYLOAD`].
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

/// The code's tag; the full prefix is `SGF<version>:`.
pub const CODE_TAG: &str = "SGF";
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

/// Why a text is no fight code this game can read.
#[derive(Clone, Debug, PartialEq)]
pub enum CodeError {
    /// It doesn't start with `SGF<version>:`.
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
                write!(f, "that is no fight code (they start with {CODE_TAG}{CODE_VERSION}:)")
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

impl FightCode {
    /// The code of a fight fought on `content` under the rules `rules` (its
    /// [`rules_fingerprint`](crate::rules_fingerprint)).
    pub fn new(content: &Content, rules: u64, spec: FightSpec, record: FightResult) -> FightCode {
        let packs = content
            .packs
            .iter()
            .map(|p| (p.id.clone(), p.version.clone()))
            .collect();
        FightCode {
            packs,
            rules,
            spec,
            record,
        }
    }

    /// The code as text: `SGF1:` and the base64url of the zlib-compressed RON.
    pub fn encode(&self) -> String {
        let ron = data::ron_options()
            .to_string(self)
            .expect("a fight code always serialises");
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(ron.as_bytes(), 10);
        format!("{CODE_TAG}{CODE_VERSION}:{}", URL_SAFE_NO_PAD.encode(packed))
    }

    /// Reads a code (whitespace around it and inside it is ignored, so a wrapped line pastes whole).
    pub fn decode(text: &str) -> Result<FightCode, CodeError> {
        let text = text.trim();
        let rest = text
            .get(..CODE_TAG.len())
            .filter(|t| t.eq_ignore_ascii_case(CODE_TAG))
            .map(|_| &text[CODE_TAG.len()..])
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
}
