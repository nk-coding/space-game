//! How the lab keeps its runs (GDR-0092 "Persistence", ADR-0041): each run in parts, so a generation writes its own
//! bytes and not the whole run's (a STANDARD run's file grows to about 0.7 MB; a generation's part is a few kB).
//!
//! * `lab/<id>`: the head ([`RunDoc`], format 2: `arena::RunHead` with the game's name, creation time and status;
//!   it stays small), written after every generation and on pause, resume, rename and validation.
//! * `lab/<id>/config`: the run's `RunConfig` ([`ConfigDoc`]), written once, when the run is made.
//! * `lab/<id>/g<n>`: generation n's part ([`PartDoc`]: its numbers, and its genomes and records packed), written
//!   once, when it closes; nothing changes it after.
//!
//! The parts go first and the head last, so a head never counts a part that isn't there (on the web what a frame
//! writes commits in one transaction anyway). A run reads back as `arena::RunParts`, which restores to the run its run
//! file would (`arena::Run::restore_parts`) and is that file split (`arena::RunParts::of`): IMPORT RUN takes a CLI
//! run file and writes its parts, and the CLI keeps its one file. Format 1 (until 16 Oct 2026) held the whole run
//! file in `lab/<id>`: it reads as its parts, and [`super::Lab::load`] writes it in parts.

use arena::RunConfig;
use arena::evolve::{RunHead, RunPart, RunParts, RunSave};

use crate::storage::{Document, Storage, StorageError};

/// A run's head as stored (`lab/<id>`).
#[derive(Clone, Debug, PartialEq)]
pub struct RunDoc {
    pub head: RunHead,
    /// Read from a format-1 document, the whole run file: its configuration and parts, to be written as their own
    /// documents (the head never writes them).
    held: Option<(RunConfig, Vec<RunPart>)>,
}

impl Document for RunDoc {
    const NAMESPACE: &'static str = "lab";
    const FORMAT: u32 = 2;

    fn to_ron(&self) -> String {
        self.head.to_ron()
    }

    fn from_ron(text: &str, format: u32) -> Result<Self, String> {
        if format == 1 {
            let save = RunSave::from_ron(text).map_err(|e| e.to_string())?;
            let RunParts { head, config, generations } = RunParts::of(&save).map_err(|e| e.to_string())?;
            return Ok(RunDoc { head, held: Some((config, generations)) });
        }
        RunHead::from_ron(text).map(|head| RunDoc { head, held: None }).map_err(|e| e.to_string())
    }
}

/// A run's configuration (`lab/<id>/config`).
struct ConfigDoc(RunConfig);

impl Document for ConfigDoc {
    const NAMESPACE: &'static str = "lab";
    const FORMAT: u32 = 1;

    fn to_ron(&self) -> String {
        self.0.to_ron()
    }

    fn from_ron(text: &str, _format: u32) -> Result<Self, String> {
        RunConfig::from_ron(text).map(ConfigDoc).map_err(|e| e.to_string())
    }
}

/// A generation's part (`lab/<id>/g<n>`).
struct PartDoc(RunPart);

impl Document for PartDoc {
    const NAMESPACE: &'static str = "lab";
    const FORMAT: u32 = 1;

    fn to_ron(&self) -> String {
        self.0.to_ron()
    }

    fn from_ron(text: &str, _format: u32) -> Result<Self, String> {
        RunPart::from_ron(text).map(PartDoc).map_err(|e| e.to_string())
    }
}

fn config_name(id: &str) -> String {
    format!("{id}/config")
}

fn part_name(id: &str, generation: u32) -> String {
    format!("{id}/g{generation}")
}

/// The stored runs' ids.
pub fn ids(storage: &Storage) -> Result<Vec<String>, StorageError> {
    storage.list(RunDoc::NAMESPACE)
}

/// A stored run, and whether it is still a format-1 document (the whole run in its head's place).
pub fn read(storage: &Storage, id: &str) -> Result<(RunParts, bool), String> {
    let gone = || format!("{id} is gone");
    let doc = storage.read::<RunDoc>(id).map_err(|e| e.to_string())?.ok_or_else(gone)?.doc;
    if let Some((config, generations)) = doc.held {
        return Ok((RunParts { head: doc.head, config, generations }, true));
    }
    let config = storage.read::<ConfigDoc>(&config_name(id)).map_err(|e| e.to_string())?.ok_or_else(|| format!("{id} has lost its configuration"))?.doc.0;
    let generations = (0..doc.head.generations)
        .map(|g| storage.read::<PartDoc>(&part_name(id, g)).map_err(|e| e.to_string())?.map(|d| d.doc.0).ok_or_else(|| format!("{id} has lost generation {g}")))
        .collect::<Result<Vec<_>, String>>()?;
    Ok((RunParts { head: doc.head, config, generations }, false))
}

/// Writes a run new to the storage (made, imported, or moved out of format 1): its configuration, every part, then
/// its head. The bytes written.
pub fn write_new(storage: &mut Storage, id: &str, parts: &RunParts, now: u64) -> Result<usize, StorageError> {
    let config = ConfigDoc(parts.config.clone());
    storage.write(&config_name(id), &config, now)?;
    let mut bytes = config.to_ron().len();
    for (g, part) in parts.generations.iter().enumerate() {
        bytes += write_part(storage, id, g as u32, part, now)?;
    }
    Ok(bytes + write_head(storage, id, &parts.head, now)?)
}

/// Writes generation `generation`'s part. The bytes written.
pub fn write_part(storage: &mut Storage, id: &str, generation: u32, part: &RunPart, now: u64) -> Result<usize, StorageError> {
    let doc = PartDoc(part.clone());
    storage.write(&part_name(id, generation), &doc, now)?;
    Ok(doc.to_ron().len())
}

/// Writes a run's head (its parts are all written). The bytes written.
pub fn write_head(storage: &mut Storage, id: &str, head: &RunHead, now: u64) -> Result<usize, StorageError> {
    let doc = RunDoc { head: head.clone(), held: None };
    storage.write(id, &doc, now)?;
    Ok(doc.to_ron().len())
}

/// Changes a stored run's head (a run not in memory: RENAME, PAUSE, RESUME); one still in format 1 is written in
/// parts with it, so its head never stands without them.
pub fn change_head(storage: &mut Storage, id: &str, now: u64, change: impl FnOnce(&mut RunHead)) -> Result<(), String> {
    let (mut parts, old) = read(storage, id)?;
    change(&mut parts.head);
    let written = match old {
        true => write_new(storage, id, &parts, now),
        false => write_head(storage, id, &parts.head, now),
    };
    written.map(|_| ()).map_err(|e| e.to_string())
}

/// Removes a run: its parts and configuration first, its head last (until then it is listed, and can be deleted
/// again).
pub fn delete(storage: &mut Storage, id: &str) -> Result<(), StorageError> {
    storage.delete_parts::<RunDoc>(id)?;
    storage.delete::<RunDoc>(id)
}
