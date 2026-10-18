//! Voyages in the career: launching one (its seed), its end (the haul unloaded into research data and materials, the
//! log, what it opens) and whether a fleet is ready for one ([`voyage_ready`]).

use super::*;

impl Career {
    /// The seed of the next launch of `voyage`: the game seed, the voyage id (64-bit FNV-1a of its bytes) and the
    /// number of launches so far, mixed with SplitMix64:
    /// `splitmix64(seed ^ splitmix64(fnv1a64(id) ^ launched))`. The same save rolls the same voyage; every launch
    /// rolls a new one. Part of the save format: changing it needs a new [`GENERATOR`].
    pub fn voyage_seed(&self, voyage: &str) -> u64 {
        let launched = self.voyages.get(voyage).map_or(0, |r| r.launched);
        splitmix64(self.seed ^ splitmix64(fnv1a64(voyage.as_bytes()) ^ u64::from(launched)))
    }

    /// Records the launch of an open voyage and returns the seed the sim rolls it from (the
    /// [`Career::voyage_seed`] before this launch). The voyage is underway until [`Career::voyage_ended`]; write
    /// the save now, so closing the game mid-voyage counts as a loss.
    pub fn voyage_launched(&mut self, content: &Content, voyage: &str) -> Result<u64, Refusal> {
        if let Some(other) = &self.underway {
            return Err(Refusal::Underway(other.clone()));
        }
        if let Some(k) = self.galaxy.as_ref().and_then(|g| g.underway()) {
            return Err(Refusal::Underway(k.to_string()));
        }
        if !self.is_open(content, voyage) {
            return Err(Refusal::NotOpen(voyage.to_string()));
        }
        let seed = self.voyage_seed(voyage);
        let record = self.voyages.entry(voyage.to_string()).or_default();
        record.launched = record.launched.saturating_add(1);
        self.log.voyages = self.log.voyages.saturating_add(1);
        self.underway = Some(voyage.to_string());
        self.sync_slice(content);
        Ok(seed)
    }

    /// Ends the voyage underway. Recalled: the haul is unloaded ([`unload`]) into the stores, and a cleared goal
    /// marks the voyage cleared (and its best time); the first clear also gives the voyage's `gives` (in store and in
    /// the result's report). Lost: the haul is gone and the goal does not count. Either way
    /// its result joins the history, new (the oldest goes past [`MAX_HISTORY`]), and is returned.
    pub fn voyage_ended(
        &mut self,
        content: &Content,
        voyage: &str,
        end: VoyageEnd,
    ) -> Result<&VoyageResult, Refusal> {
        if self.underway.as_deref() != Some(voyage) {
            return Err(Refusal::NotUnderway(voyage.to_string()));
        }
        self.underway = None;
        let mut last = match end {
            VoyageEnd::Recalled {
                haul,
                cleared,
                ticks,
            } => {
                let record = self.voyages.entry(voyage.to_string()).or_default();
                let first = cleared && !record.cleared;
                if cleared {
                    record.cleared = true;
                    record.best_ticks = Some(record.best_ticks.map_or(ticks, |b| b.min(ticks)));
                }
                let mut last = self.recalled(content, &haul, ticks, cleared);
                // The first clear's gift (the Warden's gate shard) goes into store and into the result's report.
                if let Some(gives) = content.voyages.get(voyage).map(|v| &v.gives).filter(|_| first) {
                    let gift = HaulReport { data: gives.data.clone(), materials: gives.materials.clone(), ..Default::default() };
                    self.credit(&gift);
                    for (family, &x) in &gift.data {
                        add_data(&mut last.report.data, family, x);
                    }
                    for (material, &n) in &gift.materials {
                        add_materials(&mut last.report.materials, material, n);
                    }
                }
                last
            }
            VoyageEnd::Lost { ticks } => self.lost(ticks),
        };
        last.voyage = voyage.to_string();
        self.sync_slice(content);
        Ok(self.push_history(last))
    }

    /// A voyage came home with `haul`: unloaded into the stores and counted in the log. The result's voyage is for
    /// the caller to name.
    pub(super) fn recalled(&mut self, content: &Content, haul: &Haul, ticks: u64, cleared: bool) -> VoyageResult {
        let report = unload(content, haul);
        self.credit(&report);
        // Intact parts that are not hull wait in the workshop to be kept or dismantled.
        for (part, &n) in &report.parts {
            if content.parts.get(part).is_some_and(|p| !p.hull) {
                add_materials(&mut self.intake, part, n);
            }
        }
        self.log.recalled = self.log.recalled.saturating_add(1);
        self.log.salvaged = self.log.salvaged.saturating_add(report.items());
        let outcome = if cleared { Outcome::Cleared } else { Outcome::Recalled };
        VoyageResult { voyage: String::new(), cluster: None, outcome, ticks, report, new: true }
    }

    /// A voyage was lost: counted in the log; nothing came home.
    pub(super) fn lost(&mut self, ticks: u64) -> VoyageResult {
        self.log.lost = self.log.lost.saturating_add(1);
        VoyageResult {
            voyage: String::new(),
            cluster: None,
            outcome: Outcome::Lost,
            ticks,
            report: HaulReport::default(),
            new: true,
        }
    }

    /// Adds a result to the history (the oldest goes past [`MAX_HISTORY`]) and returns it.
    pub(super) fn push_history(&mut self, last: VoyageResult) -> &VoyageResult {
        self.history.push(last);
        if self.history.len() > MAX_HISTORY {
            self.history.drain(..self.history.len() - MAX_HISTORY);
        }
        &self.history[self.history.len() - 1]
    }
}

/// What a haul gives at home (GDR-0063, GDR-0108), through the content's `salvage` blocks and recipes
/// (`docs/content-model.md` "Salvage") and the workshop's rules: an intact part that is not hull gives nothing yet (it
/// waits in the workshop's intake, [`Career::keep`], [`Career::dismantle`]); a broken part gives its block's data
/// and the rules' `broken` share of its recipe; a hull part (whole or broken) its armour material's data per cell of
/// area (slopes count half a cell) and the `broken` share of one unit of that material per cell; a fragment its
/// part's `fragment` materials; a data core is a log to decode (counted in `logs`; the career keeps which,
/// [`Career::cluster_ended`]). Shares are summed per material over the haul and rounded to the nearest unit once. Items the content
/// doesn't know or that have no block give nothing (but parts are still listed). Pure: the career adds the result
/// to its stores in [`Career::voyage_ended`].
pub fn unload(content: &Content, haul: &Haul) -> HaulReport {
    let share = content.workshop().map_or(0.0, |(_, r)| r.broken);
    let mut r = HaulReport::default();
    let mut shares: BTreeMap<String, f32> = BTreeMap::new();
    let mut share_of = |m: &str, x: f32| *shares.entry(m.to_string()).or_default() += share * x;
    for (item, &n) in &haul.items {
        if n == 0 {
            continue;
        }
        let (part, material, broken) = match item {
            HaulItem::Part { part, material } => (part, material, false),
            HaulItem::Broken { part, material } => (part, material, true),
            HaulItem::DataCore { .. } => {
                r.logs = r.logs.saturating_add(n);
                continue;
            }
            HaulItem::Fragment { part } => {
                let count = r.fragments.entry(part.clone()).or_default();
                *count = count.saturating_add(n);
                if let Some(s) = content.parts.get(part).and_then(|p| p.salvage.as_ref()) {
                    for (m, &a) in &s.fragment {
                        add_materials(&mut r.materials, m, a.saturating_mul(n));
                    }
                }
                continue;
            }
        };
        let list = if broken { &mut r.broken } else { &mut r.parts };
        let count = list.entry(part.clone()).or_default();
        *count = count.saturating_add(n);
        let Some(p) = content.parts.get(part) else {
            continue;
        };
        if p.hull {
            let armour = material.as_deref().and_then(|m| content.materials.get(m));
            if let Some((m, s)) = armour.and_then(|m| m.salvage.as_ref().map(|s| (m, s))) {
                add_data(&mut r.data, &s.family, s.data * p.area() * n as f32);
                share_of(&m.id, p.area() * n as f32);
            }
        } else if broken {
            if let Some(s) = &p.salvage {
                add_data(&mut r.data, &s.family, s.data * n as f32);
            }
            for (m, &a) in p.recipe.iter().flat_map(|x| &x.materials) {
                share_of(m, a as f32 * n as f32);
            }
        }
    }
    for (m, x) in shares {
        add_materials(&mut r.materials, &m, x.round() as u32);
    }
    r
}

/// Is the fleet (the flagship and the escorts) voyage-ready with these unlocks, this mind capacity and this stock
/// (`None`: unlimited, the free mode)? Empty: yes. Entries, in order: per ship (flagship first) a frame too big for
/// every unlocked frame, then the locked parts and materials it uses (each once, in placement order); then every
/// unlocked part the fleet carries more of than the stock holds (GDR-0108, in id order); then too many logic cores
/// for the mind; then the
/// warning that no ship has cargo room ([`Readiness::blocks`] is false only for that). The drydock and the map show
/// the first entry. Flight-readiness (power, thrust: the design report) is the sim's question, not this one.
pub fn voyage_ready(
    content: &Content,
    unlocks: &Unlocks,
    mind_capacity: u32,
    stock: Option<&BTreeMap<String, u32>>,
    flagship: &Blueprint,
    escorts: &[Blueprint],
) -> Vec<Readiness> {
    let mut out = Vec::new();
    let fleet = std::iter::once((FleetSlot::Flagship, flagship)).chain(
        escorts
            .iter()
            .enumerate()
            .map(|(k, e)| (FleetSlot::Escort(k), e)),
    );
    let (mut cores, mut cargo) = (0u32, 0u32);
    for (slot, ship) in fleet {
        let (w, h) = ship.size;
        let fits = |f: &&Frame| f.size.0 >= w && f.size.1 >= h;
        if !content
            .frames
            .iter()
            .filter(fits)
            .any(|f| unlocks.has_frame(&f.id))
        {
            out.push(Readiness::LockedFrame {
                ship: slot,
                size: ship.size,
                frame: content.frames.iter().find(fits).map(|f| f.id.clone()),
            });
        }
        let mut parts: Vec<&str> = Vec::new();
        let mut materials: Vec<&str> = Vec::new();
        for p in &ship.parts {
            if !unlocks.has_part(&p.part) && !parts.contains(&p.part.as_str()) {
                parts.push(&p.part);
            }
            if let Some(m) = p.material.as_deref()
                && !unlocks.has_material(m)
                && !materials.contains(&m)
            {
                materials.push(m);
            }
        }
        out.extend(parts.into_iter().map(|part| Readiness::LockedPart {
            ship: slot,
            part: part.to_string(),
        }));
        out.extend(
            materials
                .into_iter()
                .map(|material| Readiness::LockedMaterial {
                    ship: slot,
                    material: material.to_string(),
                }),
        );
        cores = cores.saturating_add(content.ship_totals(ship).logic_cores);
        cargo = cargo.saturating_add(content.ship_hold(ship));
    }
    if let Some(stock) = stock {
        for (part, need) in fleet_parts(content, std::iter::once(flagship).chain(escorts)) {
            let have = stock.get(&part).copied().unwrap_or(0);
            if need > have && unlocks.has_part(&part) {
                out.push(Readiness::MissingParts { part, need, have });
            }
        }
    }
    if cores > mind_capacity {
        out.push(Readiness::TooManyCores {
            cores,
            capacity: mind_capacity,
        });
    }
    if cargo == 0 {
        out.push(Readiness::NoCargo);
    }
    out
}
