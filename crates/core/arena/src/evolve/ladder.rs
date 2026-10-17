//! THE LADDER as a rating on designs (GDR-0092 step 9; GDR-0083's benchmark rating for players): a design fights the
//! rungs of `lab.ron`'s ladder trial (the preset of kind `Ladder`), the weakest first, each on the trial's seeds; its
//! rating is the sum of its rungs' WIN means, 0 up to the number of rungs, and each rung's mean says how it fares
//! there. The jobs are a [`JobBatch`], so they run in slices, on threads or in web workers alike; with their records
//! each becomes a code to re-watch ([`job_code`](super::job_code)). In a run, THE LADDER is a trial like the others.

use data::Content;
use serde::{Deserialize, Serialize};

use super::trial::{Contestant, GenCtx, Sources, Trial, TrialSpec};
use super::{JobBatch, Job, JobRecord, RunError};
use crate::spec::Entry;

/// The ladder a design is rated on: the trial (its rungs, seeds and fight settings) as a run would keep it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ladder {
    pub trial: TrialSpec,
}

/// A design's standing on the ladder: each rung's WIN mean over its seeds, the weakest rung first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rating {
    pub rungs: Vec<f32>,
}

impl Rating {
    /// The rating: the sum of the rungs' means, 0 to [`Rating::of`]'s rungs.
    pub fn total(&self) -> f32 {
        self.rungs.iter().sum()
    }

    /// How many rungs it was rated on.
    pub fn of(&self) -> usize {
        self.rungs.len()
    }
}

impl Ladder {
    /// The ladder of a `lab.ron` preset of kind `Ladder` (its seeds and time limit); `None` for another kind.
    pub fn from_preset(p: &data::TrialPreset) -> Option<Ladder> {
        matches!(p.kind, data::TrialPresetKind::Ladder { .. })
            .then(|| TrialSpec::from_preset(p, 1.0, p.seeds, false, &Sources::default()).ok())
            .flatten()
            .map(|trial| Ladder { trial })
    }

    /// The content's ladder: its first trial preset of kind `Ladder`.
    pub fn of(content: &Content) -> Option<Ladder> {
        content.lab.trials.iter().find_map(Ladder::from_preset)
    }

    /// Its rungs, the weakest first.
    pub fn rungs(&self) -> &[Vec<Entry>] {
        match &self.trial.kind {
            Trial::Ladder { rungs } => rungs,
            _ => &[],
        }
    }

    /// A fingerprint of the ladder (FNV-1a 64 of its RON): a rating made on another ladder is not this one's.
    pub fn fingerprint(&self) -> u64 {
        data::hash::fnv1a64(data::ron_options().to_string(self).expect("a ladder serialises").as_bytes())
    }

    /// The fights of `design` (one ship, as a fight entry) on every rung, each rung's seeds in order.
    pub fn jobs(&self, content: &Content, design: &Entry) -> Result<Vec<Job>, RunError> {
        self.trial.check().map_err(RunError::Fitness)?;
        let kind = self.trial.resolve(content)?;
        let me = Contestant { entry: design.clone(), build_points: 0.0 };
        Ok(kind.jobs(&me, &GenCtx { starts: &[], hall: &[], enemies: &[], fresh: None }))
    }

    /// The fights of `design` as work to step or hand out.
    pub fn batch(&self, content: &Content, design: &Entry) -> Result<JobBatch, RunError> {
        Ok(JobBatch::new(content, self.jobs(content, design)?))
    }

    /// The rating the fights' records give (in job order).
    pub fn rating(&self, records: &[JobRecord]) -> Rating {
        let seeds = self.trial.seeds.max(1) as usize;
        let rungs = self.rungs().len();
        let rungs = (0..rungs)
            .map(|r| {
                let mine = records.iter().skip(r * seeds).take(seeds);
                let n = mine.clone().count().max(1) as f32;
                mine.map(|j| j.score).sum::<f32>() / n
            })
            .collect();
        Rating { rungs }
    }
}
