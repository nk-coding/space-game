//! Evaluation jobs: one fight or one voyage run of a candidate. A job's result depends only on the job and the
//! content, so jobs may run in any order, on any thread, in one go or in slices, and give the same result.

use data::Content;
use serde::{Deserialize, Serialize};
use sim::voyage::runner::VoyageRunner;
use sim::{Owner, Sim, VesselState};

use crate::code::{Code, FightCode, VoyageCode};
use crate::fight::{Fight, FightResult, FightSetup, OWNERS, Outcome, SideResult};
use crate::score::arena_score;
use crate::spec::FightSpec;
use crate::voyage::{VoyageSpec, start_voyage, voyage_score};

use super::trial::GoalScores;

/// A fight's distance is sampled every this many ticks (half a second) for [`super::Measure::EngageDistance`].
pub const SAMPLE_TICKS: u64 = 15;

/// One fight or voyage run of candidate `candidate` of the current generation. Jobs and their results are plain
/// data with a RON form, so they can be stored with a run or sent to another thread or worker (GDR-0092).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Job {
    /// The candidate is side `side`: 0, or 1 for an enemy of the arms race (GDR-0092 "Co-evolution": your side is
    /// always side 0, so its replays look the same from either population).
    Fight {
        candidate: u32,
        #[serde(default, skip_serializing_if = "is_zero")]
        side: u32,
        spec: FightSpec,
    },
    Voyage { candidate: u32, spec: VoyageSpec },
}

/// What a job gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobResult {
    /// The candidate's score, 0..1 (0 if the job could not be built): its WIN goal.
    pub score: f32,
    /// Its score per goal (CHEAP is the design's, filled in by the run).
    #[serde(default)]
    pub goals: GoalScores,
    /// Sum and number of the distance samples (fights only).
    pub engage_sum: f32,
    pub engage_samples: u32,
    /// Ticks the job ran.
    pub ticks: u64,
    /// How it ended, kept with the run.
    pub end: JobEnd,
}

/// How a job ended, as a run keeps it (GDR-0092): with the job's spec (rebuilt from the run) it is a fight code, so
/// any fight of any child can be re-watched and checked against its end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum JobEnd {
    /// The job's world could not be built (the pre-filter should have caught it); it scored 0.
    #[default]
    Failed,
    /// A fight's record without what its spec gives again (each side's build points at the start) or what follows
    /// from the rest (the scores): [`JobRecord::fight_result`] makes the whole [`FightResult`]. Its sides are the
    /// spec's (the candidate's is the job's `side`).
    Fight {
        outcome: Outcome,
        /// Each side's build points lost, damage dealt and vessels left.
        lost: [f32; 2],
        dealt: [f32; 2],
        left: [u32; 2],
        /// The final state hash.
        hash: u64,
        /// The candidate's behavior rules that decided at least once (bit `i`: rule `i`; `Referee::decided` of its
        /// side).
        decided: u64,
        /// A gauntlet's waves cleared (`FightResult::waves`; written only for a gauntlet).
        #[serde(default, skip_serializing_if = "is_zero")]
        waves: u32,
    },
    /// A voyage run's final state hash.
    Voyage { hash: u64 },
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// One job of a child, as the run keeps it: its score, its length, how it ended and its score per goal.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobRecord {
    pub score: f32,
    pub ticks: u64,
    pub end: JobEnd,
    /// Its score per goal: kept by runs (GDR-0092 step 5); left out by the ladder's records, which score WIN alone
    /// (`score`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goals: Option<GoalScores>,
}

impl JobRecord {
    /// The record of a result; `goals` keeps its score per goal.
    pub fn of(r: &JobResult, goals: bool) -> JobRecord {
        JobRecord {
            score: r.score,
            ticks: r.ticks,
            end: r.end,
            goals: goals.then_some(r.goals),
        }
    }

    /// Its score per goal: the kept ones, else WIN alone.
    pub fn goal_scores(&self) -> GoalScores {
        self.goals.unwrap_or(GoalScores { win: self.score, ..GoalScores::default() })
    }
}

impl JobRecord {
    /// The fight's whole result, given each side's build points at the start (`Referee::build_points` of the
    /// rebuilt fight): equal to what the fight gave. `None` for a voyage or a job that failed.
    pub fn fight_result(&self, build_points: [f32; 2]) -> Option<FightResult> {
        let JobEnd::Fight {
            outcome,
            lost,
            dealt,
            left,
            hash,
            waves,
            ..
        } = self.end
        else {
            return None;
        };
        let mut sides = [0, 1].map(|k| SideResult {
            build_points: build_points[k],
            build_points_lost: lost[k],
            damage_dealt: dealt[k],
            vessels_left: left[k],
            score: 0.0,
        });
        let shares = sides.map(|s| s.lost_share());
        for (k, s) in sides.iter_mut().enumerate() {
            s.score = arena_score(outcome, k, shares);
        }
        Some(FightResult {
            outcome,
            ticks: self.ticks,
            sides,
            state_hash: hash,
            waves,
        })
    }
}

/// A job and its record as a code to re-watch, under the rules `rules` it was fought under (their fingerprint): a
/// fight code, or a voyage code. `None` for a job that failed to build.
pub fn job_code(content: &Content, rules: u64, job: Job, record: &JobRecord) -> Option<Code> {
    match (job, record.end) {
        (Job::Fight { spec, .. }, JobEnd::Fight { .. }) => {
            let build_points = FightSetup::build(content, &spec).ok()?.referee.build_points();
            Some(Code::Fight(FightCode::new(content, rules, spec, record.fight_result(build_points)?)))
        }
        (Job::Voyage { spec, .. }, JobEnd::Voyage { hash }) => Some(Code::Voyage(VoyageCode::new(content, rules, spec, record.ticks, hash))),
        _ => None,
    }
}

impl Job {
    pub fn candidate(&self) -> u32 {
        match self {
            Job::Fight { candidate, .. } | Job::Voyage { candidate, .. } => *candidate,
        }
    }

    pub(crate) fn set_candidate(&mut self, k: u32) {
        match self {
            Job::Fight { candidate, .. } | Job::Voyage { candidate, .. } => *candidate = k,
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
            Job::Fight { spec, side, .. } => match Fight::new(content, spec) {
                Ok(fight) => Active::Fight {
                    fight: Box::new(fight),
                    side: (*side as usize).min(1),
                    waves: spec.waves.len() as u32 + 1,
                    gauntlet: !spec.waves.is_empty(),
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
        /// The candidate's side.
        side: usize,
        /// A gauntlet's waves, its own first one counted, and whether it is one (WIN is then the share cleared).
        waves: u32,
        gauntlet: bool,
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
                let limit = *limit;
                let done = runner.step(budget).map(|r| {
                    let score = voyage_score(r, limit);
                    let quick = if r.cleared_and_home() { 1.0 - (r.ticks as f32 / limit.max(1) as f32).clamp(0.0, 1.0) } else { 0.0 };
                    let goals = GoalScores { win: score, unhurt: 1.0 - r.hull_lost_share(), quick, cheap: 0.0 };
                    (score, goals, r.ticks)
                });
                let done = done.map(|(score, goals, ticks)| JobResult {
                    score,
                    goals,
                    ticks,
                    end: JobEnd::Voyage {
                        hash: runner.sim().state_hash(),
                    },
                    ..JobResult::default()
                });
                (runner.sim().tick() - t0, done)
            }
            Active::Fight {
                fight,
                side,
                waves,
                gauntlet,
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
                        if let Some(d) = engage_distance(fight.sim(), *side) {
                            *sum += d;
                            *samples += 1;
                        }
                    }
                    if let Some(r) = done {
                        let end = JobEnd::Fight {
                            outcome: r.outcome,
                            lost: r.sides.map(|s| s.build_points_lost),
                            dealt: r.sides.map(|s| s.damage_dealt),
                            left: r.sides.map(|s| s.vessels_left),
                            hash: r.state_hash,
                            decided: fight.referee().decided()[*side],
                            waves: r.waves,
                        };
                        let goals = fight_goals(&r, *side, fight.referee().limit(), (*gauntlet).then_some(*waves));
                        return (
                            used,
                            Some(JobResult {
                                score: goals.win,
                                goals,
                                engage_sum: *sum,
                                engage_samples: *samples,
                                ticks: r.ticks,
                                end,
                            }),
                        );
                    }
                }
                (used, None)
            }
        }
    }
}

/// A fight's goals for side `side` (CHEAP aside): WIN is the arena score, or a gauntlet's waves cleared of `waves`
/// (side 0's); UNHURT one less the share of its build points lost; QUICK one less the share of the time limit a win
/// took.
fn fight_goals(r: &FightResult, side: usize, limit: u64, waves: Option<u32>) -> GoalScores {
    let win = match waves {
        Some(n) => (r.waves as f32 / n.max(1) as f32).clamp(0.0, 1.0),
        None => r.sides[side].score,
    };
    let quick = match r.outcome {
        Outcome::Won(s) if s == side => 1.0 - (r.ticks as f32 / limit.max(1) as f32).clamp(0.0, 1.0),
        _ => 0.0,
    };
    GoalScores { win, unhurt: 1.0 - r.sides[side].lost_share(), quick, cheap: 0.0 }
}

/// Where a vessel of `owner` stands: running vessels only, by ship id.
fn running(sim: &Sim, owner: Owner) -> impl Iterator<Item = sim::Vec2> + '_ {
    sim.ships()
        .iter()
        .filter(move |s| s.owner() == owner && s.vessel_state() == Some(VesselState::Running))
        .filter_map(|s| Some(s.piece(s.control_piece()?)?.com_world()))
}

/// Distance from side `side`'s first running vessel to the nearest running vessel of the other side.
fn engage_distance(sim: &Sim, side: usize) -> Option<f32> {
    let me = running(sim, OWNERS[side]).next()?;
    running(sim, OWNERS[1 - side])
        .map(|p| (p - me).length())
        .fold(None, |b: Option<f32>, d| Some(b.map_or(d, |b| b.min(d))))
}

/// Jobs as steppable work outside a generation (VALIDATE, a design's rating on THE LADDER): stepped in slices on one
/// thread ([`JobBatch::step`]), or handed out ([`JobBatch::jobs`]) and taken back in job order ([`JobBatch::submit`]); either
/// way the same results.
pub struct JobBatch {
    content: Content,
    jobs: Vec<Job>,
    results: Vec<Option<JobResult>>,
    next: usize,
    active: Option<(usize, Active)>,
}

impl JobBatch {
    pub fn new(content: &Content, jobs: Vec<Job>) -> JobBatch {
        let results = vec![None; jobs.len()];
        JobBatch { content: content.clone(), jobs, results, next: 0, active: None }
    }

    /// Does up to `budget` ticks of its jobs, one after the other; `true` once every job is done.
    pub fn step(&mut self, budget: u64) -> bool {
        let mut left = budget;
        while left > 0 {
            if self.active.is_none() {
                if self.next >= self.jobs.len() {
                    break;
                }
                self.active = Some((self.next, self.jobs[self.next].start(&self.content)));
                self.next += 1;
            }
            let (k, active) = self.active.as_mut().expect("a job is running");
            let (used, done) = active.advance(left);
            left -= used.min(left);
            if let Some(r) = done {
                self.results[*k] = Some(r);
                self.active = None;
            }
        }
        self.finished()
    }

    /// Its jobs, to run elsewhere ([`Job::run`]) and hand back with [`JobBatch::submit`] in the same order.
    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    /// Takes every job's result, in job order. Panics if the count differs.
    pub fn submit(&mut self, results: Vec<JobResult>) {
        assert_eq!(results.len(), self.jobs.len(), "one result per job");
        self.results = results.into_iter().map(Some).collect();
        self.active = None;
        self.next = self.jobs.len();
    }

    pub fn finished(&self) -> bool {
        self.results.iter().all(Option::is_some)
    }

    /// How far it is, 0..1.
    pub fn done(&self) -> f32 {
        self.results.iter().filter(|r| r.is_some()).count() as f32 / self.results.len().max(1) as f32
    }

    /// Every job's result, in job order. Panics unless it is finished.
    pub fn results(&self) -> Vec<JobResult> {
        self.results.iter().map(|r| r.expect("a finished batch")).collect()
    }
}
