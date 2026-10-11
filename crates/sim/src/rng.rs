//! Seeded random number streams, implemented in-crate so their output can never change with a dependency
//! update (saves, replays and the state hash depend on it).
//!
//! * Generator: **PCG32** (`pcg32_srandom_r` / XSH-RR 64→32 from pcg-random.org), 16 bytes of state.
//! * One independent stream per system ([`Stream`]). A stream's generator is seeded with
//!   `splitmix64(world_seed ^ splitmix64(stream_id))` and uses `stream_id` as its PCG sequence, so streams
//!   never share a sequence and adding a draw in one system never shifts another system's numbers.
//! * Stream ids are part of the save/replay format: **append only, never renumber**.

/// SplitMix64 finaliser (Steele, Lea, Flood 2014): a good 64-bit mixer, used to derive stream seeds.
pub fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

const PCG_MULT: u64 = 6_364_136_223_846_793_005;

/// PCG32 (XSH-RR): 64-bit LCG state, 32-bit output. Matches the reference implementation bit for bit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    /// `pcg32_srandom_r(initstate, initseq)`.
    pub fn new(seed: u64, sequence: u64) -> Pcg32 {
        let mut r = Pcg32 {
            state: 0,
            inc: (sequence << 1) | 1,
        };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r
    }

    /// Raw `(state, increment)` for hashing and saving.
    pub fn raw(&self) -> (u64, u64) {
        (self.state, self.inc)
    }

    /// Restores a generator from [`Pcg32::raw`].
    pub fn from_raw(state: u64, inc: u64) -> Pcg32 {
        Pcg32 {
            state,
            inc: inc | 1,
        }
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(PCG_MULT).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let hi = self.next_u32() as u64;
        (hi << 32) | self.next_u32() as u64
    }

    /// Uniform in `[0, 1)` with 24 random bits (exactly representable, no rounding).
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// Uniform in `[lo, hi)`.
    #[inline]
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// Uniform integer in `[0, n)` without modulo bias (Lemire's method). `n == 0` returns 0.
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let mut m = self.next_u32() as u64 * n as u64;
        if (m as u32) < n {
            let threshold = n.wrapping_neg() % n;
            while (m as u32) < threshold {
                m = self.next_u32() as u64 * n as u64;
            }
        }
        (m >> 32) as u32
    }

    /// True with probability `p` (24-bit resolution).
    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }
}

/// The random streams of the simulation, one per system. **Append only; ids never change.**
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Stream {
    /// Damage rolls (penetration, spread, chain reactions).
    Damage = 0,
    /// Weapon spread and timing jitter.
    Weapons = 1,
    /// Debris and salvage generation.
    Debris = 2,
    /// AI / behavior-tree randomness.
    Ai = 3,
    /// Test scenarios and scripted hazards (determinism check). Never used by game systems.
    Scenario = 4,
}

impl Stream {
    pub const ALL: [Stream; 5] = [
        Stream::Damage,
        Stream::Weapons,
        Stream::Debris,
        Stream::Ai,
        Stream::Scenario,
    ];
    pub const COUNT: usize = Self::ALL.len();

    /// Stable numeric id (part of the save/replay format).
    pub fn id(self) -> u64 {
        self as u64
    }
}

/// One [`Pcg32`] per [`Stream`], all derived from the world seed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RngStreams {
    streams: [Pcg32; Stream::COUNT],
}

impl RngStreams {
    pub fn new(world_seed: u64) -> RngStreams {
        RngStreams {
            streams: Stream::ALL.map(|s| {
                Pcg32::new(splitmix64(world_seed ^ splitmix64(s.id())), s.id())
            }),
        }
    }

    #[inline]
    pub fn get(&mut self, s: Stream) -> &mut Pcg32 {
        &mut self.streams[s as usize]
    }

    pub fn peek(&self, s: Stream) -> &Pcg32 {
        &self.streams[s as usize]
    }

    pub fn iter(&self) -> impl Iterator<Item = &Pcg32> {
        self.streams.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcg32_matches_reference_vector() {
        // pcg32-demo from pcg-random.org: pcg32_srandom_r(&rng, 42u, 54u), first six outputs.
        let mut r = Pcg32::new(42, 54);
        let got: Vec<u32> = (0..6).map(|_| r.next_u32()).collect();
        assert_eq!(
            got,
            vec![0xa15c02b7, 0x7b47f409, 0xba1d3330, 0x83d2f293, 0xbfa4784b, 0xcbed606e]
        );
    }

    #[test]
    fn splitmix64_matches_reference() {
        // First output of the SplitMix64 generator seeded with 0 is splitmix64(0).
        assert_eq!(splitmix64(0), 0xe220a8397b1dcdaf);
    }

    #[test]
    fn streams_are_independent_and_reproducible() {
        let mut a = RngStreams::new(7);
        let mut b = RngStreams::new(7);
        // Drawing from one stream does not shift another.
        for _ in 0..100 {
            a.get(Stream::Damage).next_u32();
        }
        assert_eq!(
            a.get(Stream::Ai).next_u64(),
            b.get(Stream::Ai).next_u64()
        );
        // Different streams differ.
        let mut c = RngStreams::new(7);
        let x = c.get(Stream::Weapons).next_u64();
        let y = c.get(Stream::Debris).next_u64();
        assert_ne!(x, y);
        // Different seeds differ.
        assert_ne!(
            RngStreams::new(1).get(Stream::Damage).next_u64(),
            RngStreams::new(2).get(Stream::Damage).next_u64()
        );
    }

    #[test]
    fn below_and_floats_stay_in_range() {
        let mut r = Pcg32::new(1, 2);
        for n in [1u32, 2, 3, 7, 1000, u32::MAX] {
            for _ in 0..200 {
                assert!(r.below(n) < n);
            }
        }
        for _ in 0..1000 {
            let f = r.next_f32();
            assert!((0.0..1.0).contains(&f));
        }
        let raw = r.raw();
        let mut q = Pcg32::from_raw(raw.0, raw.1);
        assert_eq!(q.next_u32(), r.next_u32());
    }
}
