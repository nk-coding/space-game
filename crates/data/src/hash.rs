//! The two fixed integer hash functions of the save and replay formats, implemented once here so the career, the
//! sim's RNG streams and its state hash can never drift apart (Rust's `DefaultHasher` may change between
//! releases, these never do):
//!
//! * [`fnv1a64`]: 64-bit FNV-1a over bytes. Voyage seeds hash voyage ids with it (`career::Career::voyage_seed`);
//!   the sim's state hash is FNV-1a with the same constants ([`FNV_OFFSET`], [`FNV_PRIME`]).
//! * [`splitmix64`]: one step of the SplitMix64 generator (Steele, Lea, Flood 2014), a 64-bit mixer. Voyage seeds,
//!   the sim's RNG stream seeds and the voyage plan's generator are derived with it.
//!
//! Both are part of what saves and replays mean: **never change them**. Their values are pinned by tests.

/// FNV-1a 64 offset basis.
pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a 64 prime.
pub const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// 64-bit FNV-1a of a byte string.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(FNV_OFFSET, |h, &b| (h ^ u64::from(b)).wrapping_mul(FNV_PRIME))
}

/// One step of SplitMix64: adds the generator's increment to `x`, then mixes. `splitmix64(0)` is the first output
/// of the generator seeded with 0.
pub fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reference values of the FNV specification's test suite (and the offset basis for no bytes), plus a
    /// voyage id: saves depend on these.
    #[test]
    fn fnv1a64_is_pinned() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
        assert_eq!(fnv1a64(b"yard_approach"), 0x0b8b_872e_a288_fc52);
    }

    /// The reference outputs of SplitMix64 seeded with 0 (each step adds the increment first), plus two more:
    /// saves and replays depend on these.
    #[test]
    fn splitmix64_is_pinned() {
        assert_eq!(splitmix64(0), 0xe220_a839_7b1d_cdaf);
        assert_eq!(splitmix64(0x9E37_79B9_7F4A_7C15), 0x6e78_9e6a_a1b9_65f4);
        assert_eq!(splitmix64(5), 0x6303_3b0c_a389_c35a);
        assert_eq!(splitmix64(u64::MAX), 0xe4d9_7177_1b65_2c20);
    }
}
