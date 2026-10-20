//! Ratings and part reports (GDR-0032 "ratings; reports on pick rates, win rates and unused parts", coop-arena track
//! A4; defaults GDR-0124, `docs/arena.md` "Ratings and part reports").
//!
//! A pool of designs ([`Entrant`]: a name, its build points and the parts it carries) fights one against one; every
//! fight goes into a [`Tally`] (row design against column design: fights, wins, score). [`bradley_terry`] fits one
//! rating per design to the whole tally (the maximum-likelihood Bradley–Terry fit, on the Elo scale: 400 points are
//! odds of 10 to 1, the pool's mean is 1500). [`report`] then ranks the designs and sums the parts up: how many
//! designs carry each part, its **pick rate** among the top designs, its **win rate when used** (the fights of the
//! designs that carry it) and the flags DOMINANT, UNDERUSED and NEVER PICKED by the [`Thresholds`] (*placeholders*).
//!
//! No I/O; the same tally gives the same report bit for bit (ordered containers, sums in order, own `ln` from `+ − × ÷`).
//! Running the fights, reading designs and writing files is the CLI's (`arena report`).

use std::collections::BTreeMap;

use data::{Blueprint, Content};
use serde::{Deserialize, Serialize};

use crate::fight::FightResult;

/// A design of the pool: what the report needs of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entrant {
    pub name: String,
    pub build_points: f32,
    /// Part id → how many of it the design carries.
    pub parts: BTreeMap<String, u32>,
}

impl Entrant {
    pub fn new(content: &Content, name: &str, bp: &Blueprint) -> Entrant {
        let mut parts = BTreeMap::new();
        for p in &bp.parts {
            *parts.entry(p.part.clone()).or_insert(0) += 1;
        }
        Entrant {
            name: name.to_string(),
            build_points: content.ship_totals(bp).build_points,
            parts,
        }
    }
}

/// Every pairing of `n` designs in both starting positions (no mirror matches), each on every seed:
/// `(side 0, side 1, seed)`, in a fixed order.
pub fn round_robin(n: usize, seeds: &[u64]) -> Vec<(usize, usize, u64)> {
    let mut jobs = Vec::new();
    for a in 0..n {
        for b in a + 1..n {
            for (x, y) in [(a, b), (b, a)] {
                jobs.extend(seeds.iter().map(|&s| (x, y, s)));
            }
        }
    }
    jobs
}

/// One design's fights against another.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cell {
    pub fights: u32,
    pub wins: u32,
    /// The sum of its arena scores.
    pub score: f64,
}

/// The fights of a pool, row design against column design. Mirror matches are not counted.
#[derive(Clone, Debug, PartialEq)]
pub struct Tally {
    n: usize,
    cells: Vec<Cell>,
}

impl Tally {
    pub fn new(n: usize) -> Tally {
        Tally {
            n,
            cells: vec![Cell::default(); n * n],
        }
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Design `row`'s fights against design `col`.
    pub fn cell(&self, row: usize, col: usize) -> Cell {
        self.cells[row * self.n + col]
    }

    /// A fight of design `a` (side 0) against design `b` (side 1).
    pub fn add(&mut self, a: usize, b: usize, r: &FightResult) {
        self.record(a, b, r.winner(), [r.sides[0].score, r.sides[1].score]);
    }

    /// A fight of design `a` (side 0) against design `b` (side 1) by its winning side and the sides' scores.
    pub fn record(&mut self, a: usize, b: usize, winner: Option<usize>, score: [f32; 2]) {
        if a == b {
            return;
        }
        for (side, (row, col)) in [(a, b), (b, a)].into_iter().enumerate() {
            let c = &mut self.cells[row * self.n + col];
            c.fights += 1;
            c.wins += (winner == Some(side)) as u32;
            c.score += score[side] as f64;
        }
    }

    /// Design `k`'s fights against the whole pool.
    pub fn total(&self, k: usize) -> Cell {
        (0..self.n).fold(Cell::default(), |mut t, col| {
            let c = self.cell(k, col);
            t.fights += c.fights;
            t.wins += c.wins;
            t.score += c.score;
            t
        })
    }
}

/// Where the flags start, and the fit's prior (GDR-0124; every number a *placeholder*).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thresholds {
    /// The top designs are this share of the pool by rating, rounded up (at least one).
    pub top_share: f32,
    /// DOMINANT: picked by at least this share of the top designs …
    pub dominant_pick: f32,
    /// … and its designs' mean arena score is at least this (the pool's mean is 0.5).
    pub dominant_score: f32,
    /// UNDERUSED: carried by a design of the pool, but by less than this share of the top designs.
    pub underused_pick: f32,
    /// Virtual drawn fights added to every pairing that met, so a design that won or lost everything still gets a
    /// finite rating.
    pub prior: f32,
}

/// The defaults (GDR-0124).
pub const THRESHOLDS: Thresholds = Thresholds {
    top_share: 0.5,
    dominant_pick: 0.5,
    dominant_score: 0.6,
    underused_pick: 0.2,
    prior: 1.0,
};

impl Default for Thresholds {
    fn default() -> Thresholds {
        THRESHOLDS
    }
}

/// The pool's mean rating.
pub const MEAN_RATING: f64 = 1500.0;

/// One rating per design: the Bradley–Terry fit to the tally's scores (each fight's arena score counts as that share
/// of a win, so a draw is half a win each), by Hunter's MM iteration, on the Elo scale around [`MEAN_RATING`].
/// `prior` virtual fights, drawn, are added to every pairing that met.
pub fn bradley_terry(t: &Tally, prior: f64) -> Vec<f64> {
    let n = t.len();
    let fights = |i: usize, j: usize| {
        let f = t.cell(i, j).fights as f64;
        if f > 0.0 { f + prior } else { 0.0 }
    };
    let won: Vec<f64> = (0..n)
        .map(|i| {
            (0..n)
                .filter(|&j| j != i && t.cell(i, j).fights > 0)
                .map(|j| t.cell(i, j).score + prior / 2.0)
                .sum()
        })
        .collect();
    let mut p = vec![1.0f64; n];
    for _ in 0..10_000 {
        let mut next: Vec<f64> = (0..n)
            .map(|i| {
                let d: f64 = (0..n)
                    .filter(|&j| j != i)
                    .map(|j| fights(i, j) / (p[i] + p[j]))
                    .sum();
                if d > 0.0 && won[i] > 0.0 { won[i] / d } else { p[i] }
            })
            .collect();
        let sum: f64 = next.iter().sum();
        for x in &mut next {
            *x *= n as f64 / sum;
        }
        let moved = p
            .iter()
            .zip(&next)
            .fold(0.0f64, |m, (a, b)| m.max((a - b).abs() / b));
        p = next;
        if moved < 1e-12 {
            break;
        }
    }
    let logs: Vec<f64> = p.iter().map(|&x| ln(x) / LN_10).collect();
    let mean = logs.iter().sum::<f64>() / n.max(1) as f64;
    logs.iter()
        .map(|l| MEAN_RATING + 400.0 * (l - mean))
        .collect()
}

/// The expected score of a design rated `a` against one rated `b` (the Elo curve).
pub fn expected(a: f64, b: f64) -> f64 {
    1.0 / (1.0 + pow10((b - a) / 400.0))
}

const LN_2: f64 = std::f64::consts::LN_2;
const LN_10: f64 = std::f64::consts::LN_10;

/// The natural logarithm of `x > 0` from `+ − × ÷` only (the determinism rule: no platform libm), to full f64
/// precision: `x = m · 2^e` with `m` in [√½, √2), `ln m = 2 atanh((m − 1)/(m + 1))` by its series.
fn ln(x: f64) -> f64 {
    debug_assert!(x > 0.0 && x.is_finite());
    let bits = x.to_bits();
    let mut e = ((bits >> 52) & 0x7ff) as i64 - 1023;
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000);
    if m > std::f64::consts::SQRT_2 {
        m /= 2.0;
        e += 1;
    }
    let t = (m - 1.0) / (m + 1.0);
    let t2 = t * t;
    let (mut term, mut sum) = (t, 0.0);
    for k in 0..30 {
        sum += term / (2 * k + 1) as f64;
        term *= t2;
    }
    e as f64 * LN_2 + 2.0 * sum
}

/// `10^x` from `+ − × ÷` only: `e^(x ln 10)` by halving the argument until it is small, its series, then squaring.
fn pow10(x: f64) -> f64 {
    let mut y = x * LN_10;
    let mut halvings = 0;
    while y.abs() > 0.5 && halvings < 64 {
        y /= 2.0;
        halvings += 1;
    }
    let (mut term, mut sum) = (1.0f64, 1.0f64);
    for k in 1..20 {
        term *= y / k as f64;
        sum += term;
    }
    for _ in 0..halvings {
        sum *= sum;
    }
    sum
}

/// A part's flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Flag {
    Dominant,
    Underused,
    NeverPicked,
}

impl Flag {
    pub fn label(self) -> &'static str {
        match self {
            Flag::Dominant => "DOMINANT",
            Flag::Underused => "UNDERUSED",
            Flag::NeverPicked => "NEVER PICKED",
        }
    }
}

/// A design's line of the report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DesignRow {
    pub name: String,
    /// Bradley–Terry on the Elo scale, rounded to a point.
    pub rating: f32,
    pub build_points: f32,
    pub fights: u32,
    pub wins: u32,
    /// Its mean arena score against the pool, 0..1.
    pub score: f32,
    /// Among the top designs (the pick rates' base).
    pub top: bool,
}

/// A part's line of the report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartRow {
    pub part: String,
    pub category: String,
    /// Designs of the pool that carry it.
    pub designs: u32,
    /// The share of the top designs that carry it, 0..1.
    pub pick_rate: f32,
    /// The fights of the designs that carry it, their wins and mean arena score (0 fights: none carries it).
    pub fights: u32,
    pub win_rate: f32,
    pub score: f32,
    #[serde(default)]
    pub flag: Option<Flag>,
}

/// The report: the designs best first, then the parts (carried ones by pick rate, then score; then the rest in
/// content order).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub thresholds: Thresholds,
    /// Fights counted (mirror matches left out).
    pub fights: u32,
    pub top: u32,
    pub designs: Vec<DesignRow>,
    pub parts: Vec<PartRow>,
}

/// The report of `entrants` (in the tally's order) over every part of the content.
pub fn report(content: &Content, entrants: &[Entrant], tally: &Tally, th: &Thresholds) -> Report {
    let n = entrants.len();
    assert_eq!(n, tally.len(), "one entrant per design of the tally");
    let ratings = bradley_terry(tally, th.prior as f64);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| ratings[b].total_cmp(&ratings[a]).then(a.cmp(&b)));
    let top_n = ((n as f32 * th.top_share).ceil() as usize).clamp(n.min(1), n);
    let top: Vec<usize> = order[..top_n].to_vec();
    let totals: Vec<Cell> = (0..n).map(|k| tally.total(k)).collect();
    let designs = order
        .iter()
        .map(|&k| DesignRow {
            name: entrants[k].name.clone(),
            rating: ratings[k].round() as f32,
            build_points: entrants[k].build_points,
            fights: totals[k].fights,
            wins: totals[k].wins,
            score: mean(totals[k].score, totals[k].fights),
            top: top.contains(&k),
        })
        .collect();
    let mut parts: Vec<PartRow> = content
        .parts
        .iter()
        .map(|part| {
            let users: Vec<usize> = (0..n).filter(|&k| entrants[k].parts.contains_key(&part.id)).collect();
            let picked = top.iter().filter(|k| users.contains(k)).count();
            let sum = users.iter().fold(Cell::default(), |mut s, &k| {
                s.fights += totals[k].fights;
                s.wins += totals[k].wins;
                s.score += totals[k].score;
                s
            });
            let pick_rate = picked as f32 / top_n.max(1) as f32;
            let score = mean(sum.score, sum.fights);
            let flag = if users.is_empty() {
                Some(Flag::NeverPicked)
            } else if pick_rate >= th.dominant_pick && score >= th.dominant_score {
                Some(Flag::Dominant)
            } else if pick_rate < th.underused_pick {
                Some(Flag::Underused)
            } else {
                None
            };
            PartRow {
                part: part.id.clone(),
                category: part.category.clone(),
                designs: users.len() as u32,
                pick_rate,
                fights: sum.fights,
                win_rate: mean(sum.wins as f64, sum.fights),
                score,
                flag,
            }
        })
        .collect();
    // Carried parts first, by pick rate and then score (a stable sort keeps content order among equals).
    parts.sort_by(|a, b| {
        (b.designs > 0)
            .cmp(&(a.designs > 0))
            .then(b.pick_rate.total_cmp(&a.pick_rate))
            .then(b.score.total_cmp(&a.score))
    });
    Report {
        thresholds: *th,
        fights: totals.iter().map(|c| c.fights).sum::<u32>() / 2,
        top: top_n as u32,
        designs,
        parts,
    }
}

fn mean(sum: f64, n: u32) -> f32 {
    if n == 0 { 0.0 } else { (sum / n as f64) as f32 }
}

impl Report {
    /// The parts with a flag.
    pub fn flagged(&self, flag: Flag) -> impl Iterator<Item = &PartRow> {
        self.parts.iter().filter(move |p| p.flag == Some(flag))
    }

    /// Two Markdown tables (designs, carried parts) and a line of the parts no design carries.
    pub fn to_markdown(&self) -> String {
        let th = &self.thresholds;
        let mut s = String::new();
        s.push_str(&format!(
            "{} designs, {} fights (mirror matches left out); ratings: Bradley–Terry on the Elo scale (mean {MEAN_RATING:.0}, \
             +400 = odds of 10 to 1); top designs: the best {} ({:.0} %).\n\n",
            self.designs.len(),
            self.fights,
            self.top,
            th.top_share * 100.0
        ));
        s.push_str("| # | design | rating | bp | fights | won | score | top |\n|---|---|---|---|---|---|---|---|\n");
        for (k, d) in self.designs.iter().enumerate() {
            s.push_str(&format!(
                "| {} | {} | {:.0} | {:.0} | {} | {:.0} % | {:.2} | {} |\n",
                k + 1,
                d.name,
                d.rating,
                d.build_points,
                d.fights,
                100.0 * d.wins as f32 / d.fights.max(1) as f32,
                d.score,
                if d.top { "yes" } else { "" }
            ));
        }
        s.push_str(&format!(
            "\nParts: pick rate among the top designs; win rate and mean score of the fights of the designs that carry it. \
             DOMINANT: picked by ≥ {:.0} % and scoring ≥ {:.2}; UNDERUSED: carried, but picked by < {:.0} % (*placeholders*).\n\n",
            th.dominant_pick * 100.0,
            th.dominant_score,
            th.underused_pick * 100.0
        ));
        s.push_str("| part | category | designs | pick rate | win rate when used | score when used | flag |\n|---|---|---|---|---|---|---|\n");
        for p in self.parts.iter().filter(|p| p.designs > 0) {
            s.push_str(&format!(
                "| {} | {} | {} | {:.0} % | {:.0} % | {:.2} | {} |\n",
                p.part,
                p.category,
                p.designs,
                p.pick_rate * 100.0,
                p.win_rate * 100.0,
                p.score,
                p.flag.map_or("", |f| f.label())
            ));
        }
        let never: Vec<&str> = self.flagged(Flag::NeverPicked).map(|p| p.part.as_str()).collect();
        s.push_str(&format!("\nNEVER PICKED ({}): {}\n", never.len(), never.join(", ")));
        s
    }

    pub fn to_ron(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new().struct_names(false);
        data::ron_options()
            .to_string_pretty(self, pretty)
            .expect("a report always serialises")
    }

    pub fn from_ron(text: &str) -> Result<Report, String> {
        data::ron_options().from_str(text).map_err(|e| e.to_string())
    }
}

/// A pool for `arena report` (`crates/core/arena/report/pool.ron`): the designs (content ship ids or blueprint files,
/// relative to the pool file) and the seeds per pairing and starting position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pool {
    pub designs: Vec<String>,
    pub seeds: u64,
}

impl Pool {
    pub fn from_ron(text: &str) -> Result<Pool, String> {
        data::ron_options().from_str(text).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ln_and_pow10_match_known_values() {
        use std::f64::consts::{E, SQRT_2};
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-13 * (1.0 + b.abs());
        for (x, want) in [(1.0, 0.0), (2.0, LN_2), (10.0, LN_10), (E, 1.0), (SQRT_2, LN_2 / 2.0), (1e-9, -9.0 * LN_10), (0.125, -3.0 * LN_2)] {
            assert!(close(ln(x), want), "ln {x}: {} vs {want}", ln(x));
        }
        for (x, want) in [(0.0, 1.0), (1.0, 10.0), (2.0, 100.0), (-1.0, 0.1), (0.5, 3.162_277_660_168_379_5), (-3.0, 0.001)] {
            assert!(close(pow10(x), want), "pow10 {x}: {} vs {want}", pow10(x));
        }
    }
}
