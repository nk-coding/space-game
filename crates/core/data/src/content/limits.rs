//! Limits on input from outside the build: shared designs (share codes, `.ron` files), stored documents and
//! content packs. They keep hostile or damaged input from making the game panic, hang or allocate without
//! bound. The content number limits (frame and part sizes, shot rates, ...) are in [`crate::content::validate`]; the
//! behavior text limits in [`crate::behavior`].

use serde::{Deserialize, Deserializer, de::Error as _};

/// Largest distance of a placement coordinate (`x`, `y`) from 0, in cells. A frame is at most
/// [`crate::content::validate::MAX_FRAME_SIZE`] cells wide, so real designs stay far inside; reading a blueprint refuses
/// coordinates beyond it, so the geometry never overflows.
pub const MAX_COORD: i32 = 1 << 16;

/// Deepest nesting of brackets in a RON document ([`crate::ron_options`]). The content schema needs fewer
/// than 10 levels.
pub const RON_RECURSION_LIMIT: usize = 32;

/// Largest RON document read from outside, in bytes: a blueprint ([`crate::parse_blueprint`]), a stored
/// document, the unpacked payload of a share code. A full 128 × 128 frame of 1×1 blocks is about 1.2 MB.
pub const MAX_DOCUMENT_BYTES: usize = 4 << 20;

/// A placement coordinate within ±[`MAX_COORD`] (serde `deserialize_with`).
pub(crate) fn coord<'de, D: Deserializer<'de>>(d: D) -> Result<i32, D::Error> {
    let v = i32::deserialize(d)?;
    if v.checked_abs().is_none_or(|a| a > MAX_COORD) {
        return Err(D::Error::custom(format!(
            "coordinate {v} is out of range (at most {MAX_COORD} cells from 0)"
        )));
    }
    Ok(v)
}
