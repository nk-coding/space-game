//! The arena score (coop-arena 3.7: win first, then build points) and averaging over seeds.

use data::Content;

use crate::error::ArenaError;
use crate::fight::{FightResult, Outcome, run_fight};
use crate::spec::FightSpec;

/// A side's score for one fight, 0..1: a win 1, a loss 0; otherwise (the time limit, or both sides out)
/// 0.5 + 0.5 × (share of the enemy's build points lost − share of its own lost), clamped to 0..1.
/// `lost_share[s]`: the share of side `s`'s build points lost, 0..1.
pub fn arena_score(outcome: Outcome, side: usize, lost_share: [f32; 2]) -> f32 {
    match outcome {
        Outcome::Won(w) => {
            if w == side {
                1.0
            } else {
                0.0
            }
        }
        Outcome::BothOut | Outcome::TimeLimit => {
            (0.5 + 0.5 * (lost_share[1 - side] - lost_share[side])).clamp(0.0, 1.0)
        }
    }
}

/// Several fights of one pairing together.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Summary {
    pub fights: u32,
    pub wins: [u32; 2],
    /// Fights that ended at the time limit or with both sides out.
    pub undecided: u32,
    /// Mean arena score per side.
    pub mean_score: [f32; 2],
    /// Mean fight length in ticks.
    pub mean_ticks: f32,
}

impl Summary {
    /// Counts and means over `results` (in order, so the same results give the same bits).
    pub fn of(results: &[FightResult]) -> Summary {
        let mut s = Summary::default();
        let mut score = [0.0f64; 2];
        let mut ticks = 0.0f64;
        for r in results {
            s.fights += 1;
            match r.winner() {
                Some(w) => s.wins[w] += 1,
                None => s.undecided += 1,
            }
            for (k, side) in r.sides.iter().enumerate() {
                score[k] += side.score as f64;
            }
            ticks += r.ticks as f64;
        }
        if s.fights > 0 {
            let n = s.fights as f64;
            s.mean_score = score.map(|x| (x / n) as f32);
            s.mean_ticks = (ticks / n) as f32;
        }
        s
    }
}

/// Runs the spec once per seed (the spec's own seed is replaced), in order. Average with [`Summary::of`].
pub fn run_seeds(
    content: &Content,
    spec: &FightSpec,
    seeds: &[u64],
) -> Result<Vec<FightResult>, ArenaError> {
    let mut spec = spec.clone();
    seeds
        .iter()
        .map(|&seed| {
            spec.seed = seed;
            run_fight(content, &spec)
        })
        .collect()
}
