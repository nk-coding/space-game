//! A run between two generations as a document (GDR-0092 "Persistence"): a readable RON head (the format, the
//! rules' fingerprint, the configuration, the numbers per generation, the best genome) and one `payload` field, the
//! rest as `SGL1:` and the base64url text of the zlib-compressed RON (the run's random stream, the genome table, every
//! generation's opponents and children with their job records, the archive's elites), made like fight codes. The
//! game writes it after every generation (`lab/<run-id>`), the CLI with `arena evolve --save`. A restored run goes
//! on exactly as the run would have: the open generation is bred again from the same random state, so at most one
//! generation's work is lost.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use data::Content;
use serde::{Deserialize, Serialize};
use sim::Pcg32;

use super::lineage::rebuild;
use super::{Archive, Bred, Elite, GenerationRecord, GenerationStats, Run, RunConfig, RunError, Validated, scored_in};

/// The payload's tag (space game lab); the full prefix is `SGL<format>:`.
pub const SAVE_TAG: &str = "SGL";
/// The run file format this build writes (it reads every format up to this one).
pub const SAVE_FORMAT: u32 = 1;
/// The largest compressed payload a run file may carry (a run file is untrusted text, IMPORT RUN): a STANDARD run
/// is a few hundred kB.
pub const MAX_SAVE_PACKED: usize = 16 << 20;
/// The largest payload it may unpack to.
pub const MAX_SAVE_PAYLOAD: usize = 128 << 20;

/// A run as a document (see the module docs). [`Run::snapshot`] makes one, [`Run::restore`] goes on from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSave {
    pub format: u32,
    /// The run's name, when it was made (Unix seconds) and whether it computes or waits: the game's (its RUNS list,
    /// GDR-0092 step 4); a snapshot leaves them out, and a file without them reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<RunStatus>,
    /// The rules' fingerprint when the run started (`rules_fingerprint`).
    pub rules: u64,
    pub config: RunConfig,
    /// The generations done, with their numbers.
    pub history: Vec<GenerationStats>,
    /// The archive's best genome (its number in the genome table).
    pub best: Option<u32>,
    /// `SGL1:` and the rest of the run, packed.
    pub payload: String,
}

/// Whether a run computes when it can or waits for RESUME. A finished run is done whatever it says (its history
/// is as long as its generations), a stale one is read-only ([`Run::stale`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunStatus {
    Running,
    Paused,
}

/// What the payload holds.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    /// The run's random stream (`Pcg32::raw`) before the next generation's first draw.
    rng: (u64, u64),
    genomes: Vec<Bred>,
    generations: Vec<GenerationRecord>,
    /// The archive's elites, by genome number, in cell order.
    archive: Vec<u32>,
    /// A RE-SCORE is due (`Run::rescored`; written only then).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    rescore: bool,
    /// VALIDATE's results (written only when there are any).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    validated: Vec<Validated>,
}

impl RunSave {
    /// The document as RON text; reads back with [`RunSave::from_ron`] to an equal one. Equal runs give equal text
    /// on every platform.
    pub fn to_ron(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new().escape_strings(false);
        data::ron_options()
            .to_string_pretty(self, pretty)
            .expect("a run file always serialises")
    }

    /// Reads a document (comments are fine; unknown fields are errors).
    pub fn from_ron(text: &str) -> Result<RunSave, RunError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        if text.len() > MAX_SAVE_PACKED * 2 {
            return Err(RunError::Save("too large for a run file".into()));
        }
        let save: RunSave = data::ron_options()
            .from_str(text)
            .map_err(|e| RunError::Save(e.to_string()))?;
        if save.format > SAVE_FORMAT {
            return Err(RunError::Save(format!(
                "made by a newer version of the game (run file format {}; this one reads {SAVE_FORMAT})",
                save.format
            )));
        }
        Ok(save)
    }
}

fn pack(p: &Payload) -> String {
    let ron = data::ron_options()
        .to_string(p)
        .expect("a run's payload always serialises");
    let packed = miniz_oxide::deflate::compress_to_vec_zlib(ron.as_bytes(), 10);
    format!("{SAVE_TAG}{SAVE_FORMAT}:{}", URL_SAFE_NO_PAD.encode(packed))
}

fn unpack(text: &str) -> Result<Payload, RunError> {
    let bad = |m: &str| RunError::Save(m.to_string());
    let data = text
        .strip_prefix(&format!("{SAVE_TAG}{SAVE_FORMAT}:"))
        .ok_or_else(|| bad("the payload is no run (it starts with SGL1:)"))?;
    if data.len() / 4 * 3 > MAX_SAVE_PACKED {
        return Err(bad("too large for a run file"));
    }
    let packed = URL_SAFE_NO_PAD
        .decode(data)
        .map_err(|_| bad("the payload is damaged"))?;
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&packed, MAX_SAVE_PAYLOAD)
        .map_err(|_| bad("the payload is damaged or too large"))?;
    let ron = String::from_utf8(bytes).map_err(|_| bad("the payload is damaged"))?;
    data::ron_options()
        .from_str(&ron)
        .map_err(|e| RunError::Save(format!("the payload doesn't read: {e}")))
}

impl Run {
    /// The run as a document, as it stood after its last closed generation (a generation in progress is left out:
    /// a restored run breeds it again from the same random state).
    pub fn snapshot(&self) -> RunSave {
        let (rng, genomes) = match &self.batch {
            Some(b) => (&b.from, &self.table[..b.first]),
            None => (&self.rng, &self.table[..]),
        };
        let payload = Payload {
            rng: rng.raw(),
            genomes: genomes.to_vec(),
            generations: self.records.clone(),
            archive: self.archive.cells().values().map(|e| e.id).collect(),
            rescore: self.rescore,
            validated: self.validated.clone(),
        };
        RunSave {
            format: SAVE_FORMAT,
            name: None,
            created: None,
            status: None,
            rules: self.rules,
            config: self.config.clone(),
            history: self.history.clone(),
            best: self.archive.best().map(|e| e.id),
            payload: pack(&payload),
        }
    }

    /// Goes on from a document: the run as it stood, every genome rebuilt from the table. It is stale (read-only,
    /// [`Run::stale`]) when today's rules differ from the ones it started under. Its configuration must still build
    /// on `content` (a run whose starts the content lost doesn't restore).
    pub fn restore(content: &Content, save: &RunSave) -> Result<Run, RunError> {
        let damaged = |m: &str| RunError::Save(format!("the run is damaged: {m}"));
        let p = unpack(&save.payload)?;
        let mut run = Run::new(content, save.config.clone())?;
        run.stale = run.rules != save.rules;
        run.rules = save.rules;
        // RE-SCOREs take none of the run's generations: done ones and a due one add theirs.
        run.generations += save.history.iter().filter(|h| h.rescore).count() as u32 + p.rescore as u32;
        if save.history.len() != p.generations.len() || save.history.len() > run.generations as usize {
            return Err(damaged("its generations don't match their numbers"));
        }
        let n = run.starts.len();
        if p.genomes.len() < n || p.genomes[..n].iter().any(|b| b.parent.is_some()) {
            return Err(damaged("its genome table doesn't begin with its starts"));
        }
        run.starts = (0..n as u32)
            .map(|k| rebuild(&p.genomes, &run.headers, k))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| damaged("a start doesn't rebuild"))?;
        if p.genomes.iter().any(|b| b.root as usize >= n) {
            return Err(damaged("a genome descends from no start"));
        }
        let mut archive = Archive::new(run.config.archive.clone());
        for &id in &p.archive {
            let bred = p
                .genomes
                .get(id as usize)
                .ok_or_else(|| damaged("an elite is no genome"))?;
            let child = scored_in(&save.history, &p.generations, bred.generation, id)
                .map(|(g, k)| &p.generations[g as usize].children[k])
                .ok_or_else(|| damaged("an elite was never scored"))?;
            let genome = rebuild(&p.genomes, &run.headers, id).ok_or_else(|| damaged("an elite doesn't rebuild"))?;
            archive.offer(Elite {
                genome,
                id,
                score: child.score,
                measures: child.measures.clone(),
                generation: bred.generation,
            });
        }
        run.archive = archive;
        run.rng = Pcg32::from_raw(p.rng.0, p.rng.1);
        run.table = p.genomes;
        run.records = p.generations;
        run.history = save.history.clone();
        run.rescore = p.rescore;
        run.validated = p.validated;
        Ok(run)
    }
}
