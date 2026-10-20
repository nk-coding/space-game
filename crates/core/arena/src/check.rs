//! The balance check (GDR-0032 "a short run checks every data change", coop-arena track A3; `docs/arena.md`
//! "The balance check"): a fixed set of fight specs, each run on a fixed run of seeds, summed up per fight into a
//! [`CheckRow`] and compared with a stored [`Baseline`] within a [`Tolerance`]. The fights are deterministic, so the
//! same content gives the same rows bit for bit; a data change reshuffles the fights, and the tolerances are wide
//! enough that a reshuffle alone rarely leaves them (the seeds per fight are chosen for that), so what is flagged is a
//! real shift. The set and the baseline are RON; reading the files and running the fights is the CLI's
//! (`arena check`, `scripts/arena-check.sh`).

use serde::{Deserialize, Serialize};

use crate::fight::FightResult;
use crate::score::Summary;

/// How far a fight's numbers may move from the baseline before the check fails.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tolerance {
    /// Each side's win rate (wins per fight, 0..1), absolute.
    pub wins: f32,
    /// Side 0's mean arena score (0..1), absolute.
    pub score: f32,
    /// The mean fight length, as a share of the baseline's.
    pub time: f32,
}

/// One fight of the set: a fight spec file and how many seeds it runs (the spec's seed and the ones after it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckFight {
    /// The spec file, relative to the set file; its stem names the fight.
    pub fight: String,
    pub seeds: u64,
    /// Overrides the set's tolerance for this fight.
    #[serde(default)]
    pub tolerance: Option<Tolerance>,
}

/// The check's fight set (`crates/core/arena/check/set.ron`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckSet {
    pub tolerance: Tolerance,
    pub fights: Vec<CheckFight>,
}

impl CheckSet {
    pub fn from_ron(text: &str) -> Result<CheckSet, String> {
        from_ron(text)
    }
}

/// A fight's numbers over its seeds, as the baseline stores them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckRow {
    pub fight: String,
    pub seeds: u64,
    /// Fights won by side 0 and by side 1; the rest ended undecided.
    pub won: [u32; 2],
    /// Side 0's mean arena score, rounded to 3 decimals.
    pub score: f32,
    /// The mean fight length in seconds, rounded to 0.1 s.
    pub time: f32,
    /// Every fight's final state hash folded in seed order (hex): equal means the fights did not change at all.
    pub hashes: String,
}

impl CheckRow {
    /// The row of `results`, the fights of one set entry in seed order.
    pub fn of(fight: &str, results: &[FightResult]) -> CheckRow {
        let sum = Summary::of(results);
        let hash = results.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, r| {
            (h ^ r.state_hash).wrapping_mul(0x0000_0100_0000_01b3)
        });
        CheckRow {
            fight: fight.to_string(),
            seeds: results.len() as u64,
            won: sum.wins,
            score: (sum.mean_score[0] * 1000.0).round() / 1000.0,
            time: (sum.mean_ticks / sim::TICK_HZ as f32 * 10.0).round() / 10.0,
            hashes: format!("{hash:016x}"),
        }
    }

    /// Side `s`'s win rate, 0..1.
    pub fn win_rate(&self, s: usize) -> f32 {
        self.won[s] as f32 / self.seeds.max(1) as f32
    }

    /// Why this row is out of `tol` against the baseline's `base` (empty: within it). A fight without a baseline row
    /// or with another number of seeds is out.
    pub fn out_of(&self, base: Option<&CheckRow>, tol: &Tolerance) -> Vec<String> {
        let Some(base) = base else {
            return vec!["no baseline".into()];
        };
        if base.seeds != self.seeds {
            return vec![format!("{} seeds, baseline {}", self.seeds, base.seeds)];
        }
        let mut out = Vec::new();
        for s in 0..2 {
            let d = self.win_rate(s) - base.win_rate(s);
            if d.abs() > tol.wins {
                out.push(format!("side {s} wins {:+.0} %", d * 100.0));
            }
        }
        let d = self.score - base.score;
        if d.abs() > tol.score {
            out.push(format!("score {d:+.3}"));
        }
        let d = self.time - base.time;
        if d.abs() > tol.time * base.time {
            out.push(format!("time {:+.0} %", d / base.time.max(0.1) * 100.0));
        }
        out
    }
}

/// The stored results of the set (`crates/core/arena/check/baseline.ron`), one row per fight in set order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    /// The content and sim's rules fingerprint when it was written ([`crate::rules_fingerprint`], hex): only shown,
    /// never compared.
    pub rules: String,
    pub fights: Vec<CheckRow>,
}

impl Baseline {
    pub fn from_ron(text: &str) -> Result<Baseline, String> {
        from_ron(text)
    }

    /// The RON text, with a comment saying how it is made.
    pub fn to_ron(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new().struct_names(false);
        let body = data::ron_options()
            .to_string_pretty(self, pretty)
            .expect("a baseline always serialises");
        format!(
            "// The balance check's baseline (`docs/arena.md` \"The balance check\"): written by\n\
             // `scripts/arena-check.sh --update` after an intended change. Do not edit by hand.\n{body}\n"
        )
    }

    /// The row of the fight named `fight`.
    pub fn row(&self, fight: &str) -> Option<&CheckRow> {
        self.fights.iter().find(|r| r.fight == fight)
    }
}

/// Reads a set or a baseline (the content's RON options); the error is ron's message.
fn from_ron<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    data::ron_options()
        .from_str(text)
        .map_err(|e| e.to_string())
}
