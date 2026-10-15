//! The archive (MAP-Elites, GDR-0032): a grid of niches over one to three measures of a genome (e.g. top speed ×
//! weapon reach); each cell keeps its best genome, so a run's results differ from each other.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::RunError;
use super::genome::Genome;

/// Most cells an archive may have, a sanity bound.
pub const MAX_CELLS: u64 = 65_536;

/// What a genome is measured by. Design measures come from the design report (`sim::design_report`); the
/// behavior measures from the program and, for `EngageDistance`, from the arena pool's fights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Measure {
    /// Top speed along the direction with the most thrust (m/s): the engines' rating, so it changes only with the
    /// engine type.
    TopSpeed,
    /// Acceleration along the direction with the most thrust (m/s²): thrust over mass.
    Acceleration,
    /// Reach of the longest clear gun (m): the gun type's.
    WeaponReach,
    BuildPoints,
    /// Mass of the main piece (t).
    Mass,
    /// Damage per second of all the main piece's guns at full power, as if each were clear and powered
    /// (`DesignReport::latent_firepower`): blocked and idle guns count.
    Firepower,
    /// Hit points.
    HitPoints,
    /// Parts in the design.
    Parts,
    /// Rules the behavior runs (at most the design's rule slots).
    Rules,
    /// Mean distance (m) from the genome's first running ship to the nearest enemy vessel during its fights,
    /// sampled every half second. Arena pool fitness only.
    EngageDistance,
}

impl Measure {
    /// Needs the arena pool's fights.
    pub fn needs_fights(self) -> bool {
        self == Measure::EngageDistance
    }
}

/// One axis of the grid: `cells` equal bins from `min` to `max` (values outside go to the end bins).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Axis {
    pub measure: Measure,
    pub min: f32,
    pub max: f32,
    pub cells: u32,
}

impl Axis {
    /// The bin of a value.
    pub fn bin(&self, v: f32) -> u32 {
        let t = (v - self.min) / (self.max - self.min);
        if t.is_nan() || t <= 0.0 {
            return 0;
        }
        ((t * self.cells as f32) as u32).min(self.cells - 1)
    }
}

/// The grid's axes (one to three).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveShape {
    pub axes: Vec<Axis>,
}

impl Default for ArchiveShape {
    /// Acceleration × firepower, 8 × 8. Not coop-arena 3.8's top speed × weapon reach: both are ratings of the
    /// part types (every base ship flies 20–22 m/s and reaches 45–75 m), so with the starter's parts every design
    /// would share one cell. PLACEHOLDER ranges: the base ships accelerate at 4–13 m/s² and fire 1–8 damage per
    /// second.
    fn default() -> ArchiveShape {
        ArchiveShape {
            axes: vec![
                Axis {
                    measure: Measure::Acceleration,
                    min: 0.0,
                    max: 24.0,
                    cells: 8,
                },
                Axis {
                    measure: Measure::Firepower,
                    min: 0.0,
                    max: 16.0,
                    cells: 8,
                },
            ],
        }
    }
}

impl ArchiveShape {
    pub(crate) fn check(&self) -> Result<(), RunError> {
        if !(1..=3).contains(&self.axes.len()) {
            return Err(RunError::Config("the archive needs one to three axes".into()));
        }
        let mut cells = 1u64;
        for a in &self.axes {
            if !(a.min.is_finite() && a.max.is_finite() && a.min < a.max) || a.cells == 0 {
                return Err(RunError::Config(format!(
                    "the archive axis {:?} needs min < max and at least one cell",
                    a.measure
                )));
            }
            cells *= a.cells as u64;
        }
        if cells > MAX_CELLS {
            return Err(RunError::Config(format!(
                "the archive has {cells} cells; at most {MAX_CELLS}"
            )));
        }
        Ok(())
    }
}

/// The best genome found for one cell.
#[derive(Clone, Debug, PartialEq)]
pub struct Elite {
    pub genome: Genome,
    /// Its number in the run's genome table (`Run::genome`, `Run::lineage`).
    pub id: u32,
    /// Fitness, 0..1.
    pub score: f32,
    /// The genome's value of each axis's measure, in axis order.
    pub measures: Vec<f32>,
    /// The generation it was bred in.
    pub generation: u32,
}

/// The grid of elites, keyed by cell (one bin per axis), in key order.
#[derive(Clone, Debug, PartialEq)]
pub struct Archive {
    shape: ArchiveShape,
    cells: BTreeMap<Vec<u32>, Elite>,
}

impl Archive {
    pub(crate) fn new(shape: ArchiveShape) -> Archive {
        Archive {
            shape,
            cells: BTreeMap::new(),
        }
    }

    pub fn shape(&self) -> &ArchiveShape {
        &self.shape
    }

    /// The occupied cells and their elites, in cell order.
    pub fn cells(&self) -> &BTreeMap<Vec<u32>, Elite> {
        &self.cells
    }

    /// The cell of a genome's measures.
    pub fn cell_of(&self, measures: &[f32]) -> Vec<u32> {
        self.shape
            .axes
            .iter()
            .zip(measures)
            .map(|(a, &v)| a.bin(v))
            .collect()
    }

    /// The best elite (the first in cell order among equal scores).
    pub fn best(&self) -> Option<&Elite> {
        let mut best: Option<&Elite> = None;
        for e in self.cells.values() {
            if best.is_none_or(|b| e.score > b.score) {
                best = Some(e);
            }
        }
        best
    }

    /// The `n` best elites, best first (equal scores in cell order).
    pub fn top(&self, n: usize) -> Vec<&Elite> {
        let mut all: Vec<&Elite> = self.cells.values().collect();
        all.sort_by(|a, b| b.score.total_cmp(&a.score));
        all.truncate(n);
        all
    }

    /// Puts the elite into its cell if the cell is empty or it scores higher than the one there. True if it went
    /// in.
    pub(crate) fn offer(&mut self, elite: Elite) -> bool {
        let cell = self.cell_of(&elite.measures);
        match self.cells.get(&cell) {
            Some(e) if e.score >= elite.score => false,
            _ => {
                self.cells.insert(cell, elite);
                true
            }
        }
    }
}
