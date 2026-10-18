//! The career as a save: a new game, reading and writing its RON, and reconciling it with the content it is loaded
//! with (unknown ids dropped, numbers clamped).

use super::*;

impl Career {
    /// A new game from the content's start `start`, with the game seed `seed`: the start's materials in store and
    /// the parts of its ship in stock (GDR-0108).
    pub fn new(content: &Content, start: &str, seed: u64) -> Result<Career, Refusal> {
        let Some(s) = content.starts.get(start) else {
            return Err(Refusal::UnknownStart(start.to_string()));
        };
        let stock = content.ships.get(&s.ship).map(|b| content.ship_parts(b)).unwrap_or_default();
        Ok(Career {
            packs: packs_of(content),
            start: start.to_string(),
            seed,
            generator: GENERATOR,
            researched: Vec::new(),
            data: BTreeMap::new(),
            materials: s.materials.iter().filter(|(_, n)| **n > 0).map(|(m, &n)| (m.clone(), n)).collect(),
            stock,
            intake: BTreeMap::new(),
            escorts: Vec::new(),
            seen: BTreeSet::new(),
            voyages: BTreeMap::new(),
            underway: None,
            log: Log::default(),
            history: Vec::new(),
            galaxy: None,
        })
    }

    /// Reads a career written by [`Career::to_ron`] (or by hand) with the pack loader's options: `Some(..)` may
    /// be left out, unknown fields are errors. Only the syntax and the schema are checked; call
    /// [`Career::reconcile`] next. Text from outside has a blueprint's limits: at most
    /// [`crate::content::limits::MAX_DOCUMENT_BYTES`], a leading byte order mark skipped. `Err` holds the parse error with its line and column (one problem, in a list
    /// like [`crate::parse_blueprint`]'s).
    ///
    /// A save of another voyage roller version than [`GENERATOR`] is refused too: this build would roll its voyages
    /// differently.
    pub fn from_ron(text: &str) -> Result<Career, Vec<Problem>> {
        if text.len() > MAX_DOCUMENT_BYTES {
            return Err(vec![Problem::error(format!(
                "the text is too long for a save ({} bytes, at most {MAX_DOCUMENT_BYTES})",
                text.len()
            ))]);
        }
        let career: Career = ron_options()
            .from_str(strip_bom(text))
            .map_err(|e| vec![parse_error(&e)])?;
        if career.generator != GENERATOR {
            return Err(vec![
                Problem::error(format!(
                    "the save rolls its voyages with version {} of the roller, this build with version {GENERATOR}",
                    career.generator
                ))
                .field("generator"),
            ]);
        }
        Ok(career)
    }

    /// The career as RON: deterministic, four-space indent, ending in a newline (like
    /// [`crate::blueprint_to_ron`]); reads back with [`Career::from_ron`] to an equal career.
    pub fn to_ron(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new()
            .indentor("    ")
            .new_line("\n");
        let mut text = ron_options()
            .to_string_pretty(self, pretty)
            .expect("a career is plain data and always serializes");
        text.push('\n');
        text
    }

    /// Brings a loaded career in line with the current content (it may have changed since the save was
    /// written): ids the content no longer has are dropped (researched nodes, families and materials in store
    /// and in the history, parts in stock and in the intake, voyage records and the history's entries of such voyages), an unknown start is
    /// replaced by the content's first start, numbers are made finite, non-negative and bounded ([`MAX_DATA`],
    /// [`MAX_MATERIALS`]), a history past [`MAX_HISTORY`] loses its oldest, a voyage still underway counts as
    /// lost (the game was closed mid-voyage, GDR-0062), the galaxy drops what neither the content nor its skeleton
    /// has (`Career::reconcile_galaxy`), and the pack list is updated. Returns a warning for each change, for the
    /// status line. Never fails.
    pub fn reconcile(&mut self, content: &Content) -> Vec<Problem> {
        let mut notes = Vec::new();
        let now = packs_of(content);
        for (id, version) in &self.packs {
            if !now.iter().any(|(n, _)| n == id) {
                notes.push(
                    Problem::warning(format!(
                        "pack `{id}` {version} was used by this career and is not loaded now"
                    ))
                    .field("packs"),
                );
            }
        }
        self.packs = now;

        if !content.starts.contains(&self.start) {
            let old = std::mem::take(&mut self.start);
            let note = Problem::warning(match content.starts.iter().next() {
                Some(s) => {
                    self.start = s.id.clone();
                    format!(
                        "the content has no start `{old}` any more: the career continues from `{}`",
                        s.id
                    )
                }
                None => {
                    self.start = old.clone();
                    format!("the content has no start `{old}` and no other start")
                }
            });
            notes.push(note.entry(Kind::Start, &old).field("start"));
        }

        let mut kept: Vec<String> = Vec::new();
        for (k, id) in std::mem::take(&mut self.researched).into_iter().enumerate() {
            let field = format!("researched[{k}]");
            if !content.research.contains(&id) {
                notes.push(
                    Problem::warning(format!(
                        "the content has no research node `{id}` any more: dropped with its unlocks"
                    ))
                    .entry(Kind::Research, &id)
                    .field(field),
                );
            } else if kept.contains(&id) {
                notes.push(
                    Problem::warning(format!("`{id}` is listed twice: dropped"))
                        .entry(Kind::Research, &id)
                        .field(field),
                );
            } else {
                kept.push(id);
            }
        }
        self.researched = kept;
        // What the drydock has shown is no fact worth a note: parts the content no longer has just go.
        self.seen.retain(|id| content.parts.contains(id));

        clean_data(&mut self.data, content, "data", &mut notes);
        clean_materials(&mut self.materials, content, "materials", &mut notes);
        clean_parts(&mut self.stock, content, "stock", &mut notes);
        clean_parts(&mut self.intake, content, "intake", &mut notes);

        self.voyages.retain(|id, _| {
            let known = content.voyages.contains(id);
            if !known {
                notes.push(
                    Problem::warning(format!(
                        "the content has no voyage `{id}` any more: its record is dropped"
                    ))
                    .field(format!("voyages.{id}")),
                );
            }
            known
        });

        if let Some(id) = self.underway.clone() {
            if self.voyages.get(&id).is_some_and(|r| r.launched > 0) {
                notes.push(
                    Problem::warning(format!(
                        "the game was closed during voyage `{id}`: it counts as lost"
                    ))
                    .field("underway"),
                );
                // Underway with a launch on record, so it ends.
                let _ = self.voyage_ended(content, &id, VoyageEnd::Lost { ticks: 0 });
            } else {
                notes.push(
                    Problem::warning(format!(
                        "voyage `{id}` was underway but has no launch on record: dropped"
                    ))
                    .field("underway"),
                );
                self.underway = None;
            }
        }

        let mut kept = Vec::new();
        for (k, mut v) in std::mem::take(&mut self.history).into_iter().enumerate() {
            let field = format!("history[{k}]");
            let known = match v.cluster {
                Some(c) => self.galaxy.as_ref().is_some_and(|g| (c as usize) < g.skeleton().clusters.len()),
                None => content.voyages.contains(&v.voyage),
            };
            if !known {
                notes.push(
                    Problem::warning(format!(
                        "the content has no voyage `{}` any more: its entry in the log is dropped",
                        v.voyage
                    ))
                    .field(field),
                );
                continue;
            }
            clean_data(
                &mut v.report.data,
                content,
                &format!("{field}.report.data"),
                &mut notes,
            );
            clean_materials(
                &mut v.report.materials,
                content,
                &format!("{field}.report.materials"),
                &mut notes,
            );
            kept.push(v);
        }
        if kept.len() > MAX_HISTORY {
            let gone = kept.len() - MAX_HISTORY;
            notes.push(
                Problem::warning(format!(
                    "the log keeps the last {MAX_HISTORY} voyages: the oldest {gone} are dropped"
                ))
                .field("history"),
            );
            kept.drain(..gone);
        }
        self.history = kept;
        self.reconcile_galaxy(content, &mut notes);
        notes
    }
}
