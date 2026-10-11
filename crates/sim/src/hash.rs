//! Stable state hashing, implemented in-crate so the hash never changes with a dependency update or between
//! platforms: **FNV-1a 64 applied to 32-bit words** (`h = (h ^ word) * FNV_PRIME`). Feeding whole little-endian
//! words instead of bytes needs a quarter of the multiplies (the state of 20 big ships is ~1 MB). Every
//! value is split into 32-bit words (`u64` = low word, then high word; `u8`/`bool` = one word). Byte strings
//! ([`StateHasher::write_bytes`], [`fnv1a64`]) use the classic byte-wise FNV-1a 64.
//!
//! Floats are hashed by their exact bit pattern (`to_bits`), so any difference in the last bit shows up.
//! The only exception is NaN: all NaNs hash as one canonical pattern, because NaN payloads differ between
//! platforms (x86 and wasm produce different "default NaN" bits). A NaN in the state is a bug anyway; debug
//! builds assert that none is hashed.

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const CANONICAL_NAN: u32 = 0x7fc0_0000;

/// FNV-1a 64 hasher fed with typed values.
#[derive(Clone, Copy, Debug)]
pub struct StateHasher {
    h: u64,
}

impl Default for StateHasher {
    fn default() -> Self {
        StateHasher { h: FNV_OFFSET }
    }
}

impl StateHasher {
    pub fn new() -> StateHasher {
        StateHasher::default()
    }

    #[inline]
    fn byte(&mut self, b: u8) {
        self.h ^= b as u64;
        self.h = self.h.wrapping_mul(FNV_PRIME);
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.byte(b);
        }
    }

    /// Absorbs one 32-bit word.
    #[inline]
    fn word(&mut self, w: u32) {
        self.h ^= w as u64;
        self.h = self.h.wrapping_mul(FNV_PRIME);
    }

    #[inline]
    pub fn u8(&mut self, v: u8) {
        self.word(v as u32);
    }

    #[inline]
    pub fn bool(&mut self, v: bool) {
        self.word(v as u32);
    }

    #[inline]
    pub fn u32(&mut self, v: u32) {
        self.word(v);
    }

    #[inline]
    pub fn i32(&mut self, v: i32) {
        self.word(v as u32);
    }

    #[inline]
    pub fn u64(&mut self, v: u64) {
        self.word(v as u32);
        self.word((v >> 32) as u32);
    }

    /// Exact bit pattern; NaN canonicalised (and asserted against in debug builds).
    #[inline]
    pub fn f32(&mut self, v: f32) {
        debug_assert!(!v.is_nan(), "NaN in hashed sim state");
        self.u32(if v.is_nan() {
            CANONICAL_NAN
        } else {
            v.to_bits()
        });
    }

    #[inline]
    pub fn vec2(&mut self, v: crate::math::Vec2) {
        self.f32(v.x);
        self.f32(v.y);
    }

    #[inline]
    pub fn finish(&self) -> u64 {
        self.h
    }
}

/// Types that feed their simulation state into a [`StateHasher`] in a fixed order.
pub trait StateHash {
    fn hash_state(&self, h: &mut StateHasher);
}

/// FNV-1a 64 of a byte string (handy for tests and for hashing hash streams).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = StateHasher::new();
    h.write_bytes(bytes);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a64_reference_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn floats_hash_by_bits() {
        let h = |v: f32| {
            let mut s = StateHasher::new();
            s.f32(v);
            s.finish()
        };
        assert_ne!(h(0.0), h(-0.0));
        assert_ne!(h(1.0), h(f32::from_bits(1.0f32.to_bits() + 1)));
        assert_eq!(h(1.5), {
            let mut s = StateHasher::new();
            s.u32(1.5f32.to_bits());
            s.finish()
        });
        // One word: xor then multiply.
        let mut s = StateHasher::new();
        s.u32(7);
        assert_eq!(s.finish(), (FNV_OFFSET ^ 7).wrapping_mul(FNV_PRIME));
        // Sign flips in two different words do not cancel.
        let two = |a: f32, b: f32| {
            let mut s = StateHasher::new();
            s.f32(a);
            s.f32(b);
            s.finish()
        };
        assert_ne!(two(1.0, 2.0), two(-1.0, -2.0));
    }
}
