//! Seeded random number streams, implemented in-house so their output can never change with a dependency
//! update (saves, replays and the state hash depend on it).
//!
//! * Generator: **PCG32** (`data::rng`, shared with the galaxy's generator).
//! * One independent stream per system ([`Stream`]). A stream's generator is seeded with
//!   `splitmix64(world_seed ^ splitmix64(stream_id))` and uses `stream_id` as its PCG sequence, so streams
//!   never share a sequence and adding a draw in one system never shifts another system's numbers.
//! * Stream ids are part of the save/replay format: **append only, never renumber**.

/// SplitMix64 (one step of the generator, `data::hash`): derives the stream seeds. Part of the save and replay
/// format, defined once in `data` for the career and the sim.
pub use data::hash::splitmix64;
/// PCG32 (`data::rng`): the generator of every stream.
pub use data::rng::Pcg32;

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
    /// Encounter rolls made while a voyage runs (none yet). The voyage plan is rolled before the sim on a
    /// generator of its own (`VoyagePlan::roll`), and a wreck's missing parts come from its own seed
    /// (`Sim::spawn_wreck`), so neither ever shares draws with this stream.
    Encounter = 5,
}

impl Stream {
    pub const ALL: [Stream; 6] = [
        Stream::Damage,
        Stream::Weapons,
        Stream::Debris,
        Stream::Ai,
        Stream::Scenario,
        Stream::Encounter,
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
            streams: Stream::ALL
                .map(|s| Pcg32::new(splitmix64(world_seed ^ splitmix64(s.id())), s.id())),
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
    fn streams_are_independent_and_reproducible() {
        let mut a = RngStreams::new(7);
        let mut b = RngStreams::new(7);
        // Drawing from one stream does not shift another.
        for _ in 0..100 {
            a.get(Stream::Damage).next_u32();
        }
        assert_eq!(a.get(Stream::Ai).next_u64(), b.get(Stream::Ai).next_u64());
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
}
