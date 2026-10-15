//! Evaluation jobs: one fight or one voyage run of a candidate. A job's result depends only on the job and the
//! content, so jobs may run in any order, on any thread, in one go or in slices, and give the same result.

use data::Content;
use sim::voyage::runner::VoyageRunner;
use sim::{Owner, Sim, VesselState};

use crate::fight::{Fight, OWNERS};
use crate::spec::FightSpec;
use crate::voyage::{VoyageSpec, start_voyage, voyage_score};

/// A fight's distance is sampled every this many ticks (half a second) for [`super::Measure::EngageDistance`].
pub const SAMPLE_TICKS: u64 = 15;

/// One fight or voyage run of candidate `candidate` of the current generation.
#[derive(Clone, Debug, PartialEq)]
pub enum Job {
    /// The candidate is side 0.
    Fight { candidate: u32, spec: FightSpec },
    Voyage { candidate: u32, spec: VoyageSpec },
}

/// What a job gives.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JobResult {
    /// The candidate's score, 0..1 (0 if the job could not be built).
    pub score: f32,
    /// Sum and number of the distance samples (fights only).
    pub engage_sum: f32,
    pub engage_samples: u32,
    /// Ticks the job ran.
    pub ticks: u64,
}

impl Job {
    pub fn candidate(&self) -> u32 {
        match self {
            Job::Fight { candidate, .. } | Job::Voyage { candidate, .. } => *candidate,
        }
    }

    /// Runs the job to its end.
    pub fn run(&self, content: &Content) -> JobResult {
        let mut a = self.start(content);
        loop {
            if let (_, Some(r)) = a.advance(u64::MAX) {
                return r;
            }
        }
    }

    /// Builds the job's world. A job that cannot be built (the pre-filter should have caught it) scores 0.
    pub(crate) fn start(&self, content: &Content) -> Active {
        match self {
            Job::Fight { spec, .. } => match Fight::new(content, spec) {
                Ok(fight) => Active::Fight {
                    fight: Box::new(fight),
                    phase: 0,
                    sum: 0.0,
                    samples: 0,
                },
                Err(_) => Active::Failed,
            },
            Job::Voyage { spec, .. } => match start_voyage(content, spec) {
                Ok(runner) => Active::Voyage {
                    runner: Box::new(runner),
                    limit: spec.limit_ticks(),
                },
                Err(_) => Active::Failed,
            },
        }
    }
}

/// A job in progress.
pub(crate) enum Active {
    Fight {
        fight: Box<Fight>,
        /// Ticks since the last distance sample.
        phase: u64,
        sum: f32,
        samples: u32,
    },
    Voyage {
        runner: Box<VoyageRunner>,
        limit: u64,
    },
    Failed,
}

impl Active {
    /// Runs up to `budget` ticks: the ticks run, and the result once the job has ended.
    pub fn advance(&mut self, budget: u64) -> (u64, Option<JobResult>) {
        match self {
            Active::Failed => (0, Some(JobResult::default())),
            Active::Voyage { runner, limit } => {
                let t0 = runner.sim().tick();
                let done = runner.step(budget).map(|r| JobResult {
                    score: voyage_score(r, *limit),
                    ticks: r.ticks,
                    ..JobResult::default()
                });
                (runner.sim().tick() - t0, done)
            }
            Active::Fight {
                fight,
                phase,
                sum,
                samples,
            } => {
                let mut used = 0u64;
                while used < budget {
                    let n = (budget - used).min(SAMPLE_TICKS - *phase);
                    let t0 = fight.sim().tick();
                    let done = fight.step(n);
                    let dt = fight.sim().tick() - t0;
                    used += dt;
                    *phase += dt;
                    if *phase == SAMPLE_TICKS {
                        *phase = 0;
                        if let Some(d) = engage_distance(fight.sim()) {
                            *sum += d;
                            *samples += 1;
                        }
                    }
                    if let Some(r) = done {
                        return (
                            used,
                            Some(JobResult {
                                score: r.sides[0].score,
                                engage_sum: *sum,
                                engage_samples: *samples,
                                ticks: r.ticks,
                            }),
                        );
                    }
                }
                (used, None)
            }
        }
    }
}

/// Where a vessel of `owner` stands: running vessels only, by ship id.
fn running(sim: &Sim, owner: Owner) -> impl Iterator<Item = sim::Vec2> + '_ {
    sim.ships()
        .iter()
        .filter(move |s| s.owner() == owner && s.vessel_state() == Some(VesselState::Running))
        .filter_map(|s| Some(s.piece(s.control_piece()?)?.com_world()))
}

/// Distance from side 0's first running vessel to the nearest running vessel of side 1.
fn engage_distance(sim: &Sim) -> Option<f32> {
    let me = running(sim, OWNERS[0]).next()?;
    running(sim, OWNERS[1])
        .map(|p| (p - me).length())
        .fold(None, |b: Option<f32>, d| Some(b.map_or(d, |b| b.min(d))))
}
