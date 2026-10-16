//! A run between two generations as a document (GDR-0092 "Persistence"): a readable RON head (the format, the
//! rules' fingerprint, the configuration, the numbers per generation, the best genome) and one `payload` field, the
//! rest as `SGL1:` and the base64url text of the zlib-compressed RON (the run's random stream, the genome table, every
//! generation's opponents and children with their job records, the archive's elites), made like fight codes. The
//! CLI writes it with `arena evolve --save`. A restored run goes on exactly as the run would have: the open
//! generation is bred again from the same random state, so at most one generation's work is lost.
//!
//! The same run in parts ([`RunParts`], 16 Oct 2026) is what the game's lab writes (`lab/<run-id>` and its parts,
//! ADR-0041): a small head written again after every generation, the configuration once, and one part per
//! generation written once when it closes, so a generation costs its own bytes, not the whole run's. A run restored
//! from its parts is the run restored from its file, and [`RunParts::of`] splits a run file.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use data::Content;
use serde::{Deserialize, Serialize};
use sim::Pcg32;

use super::lineage::rebuild;
use super::{Archive, Bred, Elite, GenerationRecord, GenerationStats, Genome, Run, RunConfig, RunError, Validated, headers_of, scored_in};

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
    /// THE ENEMY of an arms race (written only then).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    enemy: Option<EnemyState>,
}

/// What a run file keeps of THE ENEMY between generations: its random stream and its archive's elites (genome numbers,
/// in cell order; their scores and measures come from the records).
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnemyState {
    rng: (u64, u64),
    archive: Vec<u32>,
}

impl RunSave {
    /// The document as RON text; reads back with [`RunSave::from_ron`] to an equal one. Equal runs give equal text
    /// on every platform.
    pub fn to_ron(&self) -> String {
        to_text(self)
    }

    /// Reads a document (comments are fine; unknown fields are errors).
    pub fn from_ron(text: &str) -> Result<RunSave, RunError> {
        let save: RunSave = from_text(text)?;
        newer(save.format)?;
        Ok(save)
    }

    /// Genome `id` of the run, rebuilt from the file without building the run (a design of an earlier run, the lab's
    /// CHAMPIONS): its starts must still resolve on `content`.
    pub fn genome(&self, content: &Content, id: u32) -> Result<Genome, RunError> {
        let p: Payload = unpack(&self.payload)?;
        let headers = headers_of(content, &self.config)?;
        rebuild(&p.genomes, &headers, id).ok_or_else(|| RunError::Save(format!("genome {id} doesn't rebuild")))
    }
}

/// A run file in parts (see the module docs): the head, the configuration, and one part per generation done. Made by
/// [`Run::parts`] (every part) or [`RunParts::of`] (a run file split); a store that writes a run as it grows writes
/// [`Run::head`] and [`Run::part`] of the generation that closed. [`Run::restore_parts`] goes on from it.
#[derive(Clone, Debug, PartialEq)]
pub struct RunParts {
    pub head: RunHead,
    pub config: RunConfig,
    /// The generations done, in order.
    pub generations: Vec<RunPart>,
}

/// A run's head in parts: the run file's head without the configuration and the numbers per generation (they are
/// the parts'), and `SGL1:` with what changes between generations, packed (the random stream, the archive's elites,
/// a due RE-SCORE, VALIDATE's results, and the genomes no part holds yet: a run with no generation done keeps its
/// starts here). It stays small however long the run gets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunHead {
    pub format: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<RunStatus>,
    pub rules: u64,
    /// The generations done: the parts the run has.
    pub generations: u32,
    pub best: Option<u32>,
    pub payload: String,
}

/// One generation done in parts: its numbers, readable, and `SGL1:` with the genomes bred for it (generation 0's
/// begin with the starts) and its record, packed. Written once: nothing changes a closed generation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunPart {
    pub stats: GenerationStats,
    pub payload: String,
}

/// What a head's payload holds.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HeadPayload {
    rng: (u64, u64),
    archive: Vec<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    rescore: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    validated: Vec<Validated>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    genomes: Vec<Bred>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    enemy: Option<EnemyState>,
}

/// What a part's payload holds.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartPayload {
    genomes: Vec<Bred>,
    record: GenerationRecord,
}

impl RunHead {
    /// The head as RON text; reads back with [`RunHead::from_ron`] to an equal one.
    pub fn to_ron(&self) -> String {
        to_text(self)
    }

    pub fn from_ron(text: &str) -> Result<RunHead, RunError> {
        let head: RunHead = from_text(text)?;
        newer(head.format)?;
        Ok(head)
    }
}

impl RunPart {
    /// The part as RON text; reads back with [`RunPart::from_ron`] to an equal one.
    pub fn to_ron(&self) -> String {
        to_text(self)
    }

    pub fn from_ron(text: &str) -> Result<RunPart, RunError> {
        from_text(text)
    }
}

impl RunParts {
    /// A run file split into its parts (a run file kept whole until now: the game's lab before 16 Oct 2026, or the
    /// CLI's); [`Run::restore_parts`] of them is [`Run::restore`] of the file.
    pub fn of(save: &RunSave) -> Result<RunParts, RunError> {
        let p: Payload = unpack(&save.payload)?;
        let n = save.history.len();
        if p.generations.len() != n {
            return Err(damaged("its generations don't match their numbers"));
        }
        let mut genomes = p.genomes.into_iter().peekable();
        let mut generations = Vec::with_capacity(n);
        for (g, (stats, record)) in save.history.iter().zip(p.generations).enumerate() {
            let mut bred = Vec::new();
            while let Some(b) = genomes.next_if(|b| b.generation as usize <= g) {
                bred.push(b);
            }
            generations.push(RunPart { stats: stats.clone(), payload: pack(&PartPayload { genomes: bred, record }) });
        }
        let head = HeadPayload { rng: p.rng, archive: p.archive, rescore: p.rescore, validated: p.validated, genomes: genomes.collect(), enemy: p.enemy };
        Ok(RunParts {
            head: RunHead {
                format: save.format,
                name: save.name.clone(),
                created: save.created,
                status: save.status,
                rules: save.rules,
                generations: n as u32,
                best: save.best,
                payload: pack(&head),
            },
            config: save.config.clone(),
            generations,
        })
    }

    /// Genome `id` of the run, rebuilt from its parts without building the run (as [`RunSave::genome`]).
    pub fn genome(&self, content: &Content, id: u32) -> Result<Genome, RunError> {
        let (_, p) = self.payload()?;
        let headers = headers_of(content, &self.config)?;
        rebuild(&p.genomes, &headers, id).ok_or_else(|| RunError::Save(format!("genome {id} doesn't rebuild")))
    }

    /// The numbers per generation and the payload a run file of the same run holds. Every part's genomes are of its
    /// own generation, the head's of later ones, so the genome table comes back in breeding order.
    fn payload(&self) -> Result<(Vec<GenerationStats>, Payload), RunError> {
        if self.head.generations as usize != self.generations.len() {
            return Err(damaged("its parts don't match its head"));
        }
        let head: HeadPayload = unpack(&self.head.payload)?;
        let (mut history, mut genomes, mut records) = (Vec::new(), Vec::new(), Vec::new());
        for (g, part) in self.generations.iter().enumerate() {
            let p: PartPayload = unpack(&part.payload)?;
            if part.stats.generation as usize != g || p.genomes.iter().any(|b| b.generation as usize != g) {
                return Err(damaged("a part is not of its generation"));
            }
            history.push(part.stats.clone());
            genomes.extend(p.genomes);
            records.push(p.record);
        }
        if head.genomes.iter().any(|b| (b.generation as usize) < history.len()) {
            return Err(damaged("its head holds a genome of a generation done"));
        }
        genomes.extend(head.genomes);
        let payload = Payload { rng: head.rng, genomes, generations: records, archive: head.archive, rescore: head.rescore, validated: head.validated, enemy: head.enemy };
        Ok((history, payload))
    }
}

fn damaged(m: &str) -> RunError {
    RunError::Save(format!("the run is damaged: {m}"))
}

fn newer(format: u32) -> Result<(), RunError> {
    match format > SAVE_FORMAT {
        true => Err(RunError::Save(format!("made by a newer version of the game (run file format {format}; this one reads {SAVE_FORMAT})"))),
        false => Ok(()),
    }
}

/// A document as pretty RON (strings as they are), the same text on every platform.
fn to_text<T: Serialize>(v: &T) -> String {
    let pretty = ron::ser::PrettyConfig::new().escape_strings(false);
    data::ron_options()
        .to_string_pretty(v, pretty)
        .expect("a run document always serialises")
}

/// Reads a document (a byte order mark and comments are fine; unknown fields are errors), refusing text larger than
/// a run file may be.
fn from_text<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, RunError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.len() > MAX_SAVE_PACKED * 2 {
        return Err(RunError::Save("too large for a run file".into()));
    }
    data::ron_options()
        .from_str(text)
        .map_err(|e| RunError::Save(e.to_string()))
}

fn pack<T: Serialize>(p: &T) -> String {
    let ron = data::ron_options()
        .to_string(p)
        .expect("a run's payload always serialises");
    let packed = miniz_oxide::deflate::compress_to_vec_zlib(ron.as_bytes(), 10);
    format!("{SAVE_TAG}{SAVE_FORMAT}:{}", URL_SAFE_NO_PAD.encode(packed))
}

fn unpack<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, RunError> {
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
    /// The run's genome table as it stood after its last closed generation, and the random stream then (a
    /// generation in progress is left out: a restored run breeds it again from the same random state).
    fn closed(&self) -> (&Pcg32, &[Bred]) {
        match &self.batch {
            Some(b) => (&b.from, &self.table[..b.first]),
            None => (&self.rng, &self.table[..]),
        }
    }

    /// THE ENEMY as it stood after the last closed generation: its stream then and its archive's elites.
    fn enemy_state(&self) -> Option<EnemyState> {
        let e = self.enemy.as_ref()?;
        let rng = match self.batch.as_ref().and_then(|b| b.rivalry.as_ref()) {
            Some(r) => &r.from,
            None => &e.rng,
        };
        Some(EnemyState { rng: rng.raw(), archive: e.archive.cells().values().map(|e| e.id).collect() })
    }

    /// The run as a document, as it stood after its last closed generation.
    pub fn snapshot(&self) -> RunSave {
        let (rng, genomes) = self.closed();
        let payload = Payload {
            rng: rng.raw(),
            genomes: genomes.to_vec(),
            generations: self.records.clone(),
            archive: self.archive.cells().values().map(|e| e.id).collect(),
            rescore: self.rescore,
            validated: self.validated.clone(),
            enemy: self.enemy_state(),
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

    /// The run's head in parts, as it stood after its last closed generation (name, creation time and status left
    /// out, as in [`Run::snapshot`]).
    pub fn head(&self) -> RunHead {
        let (rng, genomes) = self.closed();
        let done = self.history.len();
        let payload = HeadPayload {
            rng: rng.raw(),
            archive: self.archive.cells().values().map(|e| e.id).collect(),
            rescore: self.rescore,
            validated: self.validated.clone(),
            genomes: genomes[genomes.partition_point(|b| (b.generation as usize) < done)..].to_vec(),
            enemy: self.enemy_state(),
        };
        RunHead {
            format: SAVE_FORMAT,
            name: None,
            created: None,
            status: None,
            rules: self.rules,
            generations: done as u32,
            best: self.archive.best().map(|e| e.id),
            payload: pack(&payload),
        }
    }

    /// Closed generation `generation`'s part: its numbers, the genomes bred for it and its record.
    pub fn part(&self, generation: u32) -> Option<RunPart> {
        let g = generation as usize;
        let (stats, record) = (self.history.get(g)?, self.records.get(g)?);
        let (_, genomes) = self.closed();
        let bred = &genomes[genomes.partition_point(|b| (b.generation as usize) < g)..genomes.partition_point(|b| b.generation as usize <= g)];
        Some(RunPart { stats: stats.clone(), payload: pack(&PartPayload { genomes: bred.to_vec(), record: record.clone() }) })
    }

    /// The run in parts, every generation done.
    pub fn parts(&self) -> RunParts {
        RunParts {
            head: self.head(),
            config: self.config.clone(),
            generations: (0..self.history.len() as u32).filter_map(|g| self.part(g)).collect(),
        }
    }

    /// Goes on from a document: the run as it stood, every genome rebuilt from the table. It is stale (read-only,
    /// [`Run::stale`]) when today's rules differ from the ones it started under. Its configuration must still build
    /// on `content` (a run whose starts the content lost doesn't restore).
    pub fn restore(content: &Content, save: &RunSave) -> Result<Run, RunError> {
        Run::restore_payload(content, &save.config, save.rules, save.history.clone(), unpack(&save.payload)?)
    }

    /// Goes on from a run's parts: the same run [`Run::restore`] makes of its file.
    pub fn restore_parts(content: &Content, parts: &RunParts) -> Result<Run, RunError> {
        let (history, payload) = parts.payload()?;
        Run::restore_payload(content, &parts.config, parts.head.rules, history, payload)
    }

    fn restore_payload(content: &Content, config: &RunConfig, rules: u64, history: Vec<GenerationStats>, p: Payload) -> Result<Run, RunError> {
        let mut run = Run::new(content, config.clone())?;
        run.stale = run.rules != rules;
        run.rules = rules;
        // RE-SCOREs take none of the run's generations: done ones and a due one add theirs.
        run.generations += history.iter().filter(|h| h.rescore).count() as u32 + p.rescore as u32;
        if history.len() != p.generations.len() || history.len() > run.generations as usize {
            return Err(damaged("its generations don't match their numbers"));
        }
        let yours = run.starts.len();
        let n = yours + run.enemy_starts().len();
        if p.genomes.len() < n || p.genomes[..n].iter().any(|b| b.parent.is_some()) {
            return Err(damaged("its genome table doesn't begin with its starts"));
        }
        let mut starts = (0..n as u32)
            .map(|k| rebuild(&p.genomes, &run.headers, k))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| damaged("a start doesn't rebuild"))?;
        if p.genomes.iter().any(|b| b.root as usize >= n) {
            return Err(damaged("a genome descends from no start"));
        }
        let elites = |ids: &[u32], shape: &super::ArchiveShape| -> Result<Archive, RunError> {
            let mut archive = Archive::new(shape.clone());
            for &id in ids {
                let bred = p
                    .genomes
                    .get(id as usize)
                    .ok_or_else(|| damaged("an elite is no genome"))?;
                let child = scored_in(&history, &p.generations, bred.generation, id)
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
            Ok(archive)
        };
        run.archive = elites(&p.archive, &run.config.archive)?;
        match (&mut run.enemy, &run.config.enemy, &p.enemy) {
            (Some(e), Some(pop), Some(state)) => {
                e.archive = elites(&state.archive, &pop.archive)?;
                e.rng = Pcg32::from_raw(state.rng.0, state.rng.1);
                e.starts = starts.split_off(yours);
            }
            (None, None, None) => {}
            _ => return Err(damaged("its enemy doesn't match its configuration")),
        }
        run.starts = starts;
        run.rng = Pcg32::from_raw(p.rng.0, p.rng.1);
        run.table = p.genomes;
        run.records = p.generations;
        run.history = history;
        run.rescore = p.rescore;
        run.validated = p.validated;
        Ok(run)
    }
}
