//! THE LADDER as a rating on YOUR DESIGNS (GDR-0092 step 9): RATE ON THE LADDER fights a saved design (its blueprint
//! and the behavior it flies, inlined as a run's starts are) on `lab.ron`'s ladder (`arena::evolve::ladder`), in the
//! run's place, and keeps the result in `ladder/<design id>` ([`RatingDoc`]): the rating, each rung's mean, every
//! fight's record (each a code to watch again) and what it was rated as, so a rating made before the design, its
//! behavior, the ladder or the rules changed says so ([`Standing`]). The main thread writes it, like every
//! document; the lab's workers only fight.

use arena::evolve::ladder::{Ladder, Rating as Rungs};
use arena::evolve::{JobBatch, JobRecord};
use arena::{Code, Entry};
use data::Content;
use serde::{Deserialize, Serialize};

use super::{StartPick, design_entry};
use crate::player::PlayerDesign;
use crate::player::behaviors::PlayerBehaviors;
use crate::storage::{Document, Storage, StorageError};

/// A design's rating as stored (`ladder/<design id>`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RatingDoc {
    /// The design's name when it was rated.
    pub name: String,
    /// What was rated: the design and its behavior as a fight entry, and its fingerprint ([`inputs`]).
    pub design: Entry,
    pub inputs: u64,
    /// The rules' and the ladder's fingerprints then.
    pub rules: u64,
    pub ladder: u64,
    pub rating: Rungs,
    /// Every fight's record, in the ladder's job order (rung by rung, each on every seed).
    pub records: Vec<JobRecord>,
}

impl Document for RatingDoc {
    const NAMESPACE: &'static str = "ladder";
    const FORMAT: u32 = 1;

    fn to_ron(&self) -> String {
        data::ron_options().to_string(self).expect("a rating serialises")
    }

    fn from_ron(text: &str, _format: u32) -> Result<Self, String> {
        data::ron_options().from_str(text).map_err(|e| e.to_string())
    }
}

/// A fight entry's fingerprint (FNV-1a 64 of its RON): the design and the behavior it flies.
pub fn inputs(design: &Entry) -> u64 {
    data::hash::fnv1a64(data::ron_options().to_string(design).expect("an entry serialises").as_bytes())
}

/// Where a saved design stands on the ladder.
#[derive(Clone, Debug, PartialEq)]
pub enum Standing {
    /// Never rated (or its rating doesn't read).
    Unrated,
    /// Rated as it is now, on today's ladder and rules.
    Rated(RatingDoc),
    /// Rated before the design, its behavior, the ladder or the rules changed.
    Changed(RatingDoc),
}

impl Standing {
    /// Saved design `id`'s standing now (`rules`: today's rules' fingerprint).
    pub fn of(storage: &Storage, content: &Content, behaviors: &PlayerBehaviors, working: &PlayerDesign, rules: u64, id: &str) -> Standing {
        let Ok(Some(doc)) = storage.read::<RatingDoc>(id) else { return Standing::Unrated };
        let doc = doc.doc;
        let now = StartPick::Design(id.to_string()).resolve(storage, content, behaviors, working).ok().map(|d| inputs(&design_entry(&d)));
        let ladder = Ladder::of(content).map(|l| l.fingerprint());
        if now == Some(doc.inputs) && ladder == Some(doc.ladder) && rules == doc.rules {
            Standing::Rated(doc)
        } else {
            Standing::Changed(doc)
        }
    }

    pub fn doc(&self) -> Option<&RatingDoc> {
        match self {
            Standing::Unrated => None,
            Standing::Rated(d) | Standing::Changed(d) => Some(d),
        }
    }
}

impl RatingDoc {
    /// Fight `k` (in job order) as a code to watch again, under the rules it was fought under.
    pub fn code(&self, content: &Content, k: usize) -> Option<Code> {
        let ladder = Ladder::of(content)?;
        let job = ladder.jobs(content, &self.design).ok()?.into_iter().nth(k)?;
        arena::evolve::job_code(content, self.rules, job, self.records.get(k)?)
    }
}

/// A rating underway: the design, the ladder it fights and the work.
pub struct Rating {
    /// The saved design's id and name.
    pub design: String,
    pub name: String,
    entry: Entry,
    ladder: Ladder,
    rules: u64,
    pub batch: JobBatch,
}

impl Rating {
    /// Saved design `id` on `ladder` (`rules`: today's rules' fingerprint).
    pub fn new(storage: &Storage, content: &Content, behaviors: &PlayerBehaviors, working: &PlayerDesign, ladder: &Ladder, rules: u64, id: &str) -> Result<Rating, String> {
        let d = StartPick::Design(id.to_string()).resolve(storage, content, behaviors, working)?;
        let entry = design_entry(&d);
        let batch = ladder.batch(content, &entry).map_err(|e| e.to_string())?;
        Ok(Rating { design: id.to_string(), name: d.name, entry, ladder: ladder.clone(), rules, batch })
    }

    /// How far it is, 0..1.
    pub fn done(&self) -> f32 {
        self.batch.done()
    }

    /// The finished rating as its document.
    pub fn doc(&self) -> RatingDoc {
        let records: Vec<JobRecord> = self.batch.results().iter().map(|r| JobRecord::of(r, false)).collect();
        RatingDoc {
            name: self.name.clone(),
            inputs: inputs(&self.entry),
            design: self.entry.clone(),
            rules: self.rules,
            ladder: self.ladder.fingerprint(),
            rating: self.ladder.rating(&records),
            records,
        }
    }

    /// Writes the finished rating to `ladder/<design id>`.
    pub fn write(&self, storage: &mut Storage, now: u64) -> Result<(), StorageError> {
        storage.write(&self.design, &self.doc(), now)
    }
}
