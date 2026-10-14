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
        if !self.is_open(content, voyage) {
            return Err(Refusal::NotOpen(voyage.to_string()));
        }
        let seed = self.voyage_seed(voyage);
        let record = self.voyages.entry(voyage.to_string()).or_default();
        record.launched = record.launched.saturating_add(1);
        self.log.voyages = self.log.voyages.saturating_add(1);
        self.underway = Some(voyage.to_string());
        Ok(seed)
    }

    /// Ends the voyage underway. Recalled: the haul is unloaded ([`unload`]) into the stores, and a cleared goal
    /// marks the voyage cleared (and its best time). Lost: the haul is gone and the goal does not count. Either way
    /// the debrief becomes the last haul, which is returned.
    pub fn voyage_ended(
        &mut self,
        content: &Content,
        voyage: &str,
        end: VoyageEnd,
    ) -> Result<&LastHaul, Refusal> {
        if self.underway.as_deref() != Some(voyage) {
            return Err(Refusal::NotUnderway(voyage.to_string()));
        }
        self.underway = None;
        let last = match end {
            VoyageEnd::Recalled {
                haul,
                cleared,
                ticks,
            } => {
                let report = unload(content, &haul);
                for (family, &x) in &report.data {
                    add_data(&mut self.data, family, x);
                }
                for (material, &x) in &report.materials {
                    add_materials(&mut self.materials, material, x);
                }
                let record = self.voyages.entry(voyage.to_string()).or_default();
                if cleared {
                    record.cleared = true;
                    record.best_ticks = Some(record.best_ticks.map_or(ticks, |b| b.min(ticks)));
                }
                self.log.recalled = self.log.recalled.saturating_add(1);
                self.log.salvaged = self.log.salvaged.saturating_add(report.items());
                LastHaul {
                    voyage: voyage.to_string(),
                    outcome: if cleared {
                        Outcome::Cleared
                    } else {
                        Outcome::Recalled
                    },
                    ticks,
                    report,
                }
            }
            VoyageEnd::Lost { ticks } => {
                self.log.lost = self.log.lost.saturating_add(1);
                LastHaul {
                    voyage: voyage.to_string(),
                    outcome: Outcome::Lost,
                    ticks,
                    report: HaulReport::default(),
                }
            }
        };
        Ok(self.last_haul.insert(last))
    }
}

/// What a haul is dismantled into at home, through the content's `salvage` blocks (`docs/content-model.md`
/// "Salvage"): a whole part gives its block's data and materials; a hull part gives its armour material's data
/// per cell of area (slopes count half a cell); a fragment gives its part's `fragment` materials. Items the content
/// doesn't know or that have no block give nothing (but whole parts are still listed). Pure: the career adds the
/// result to its stores in [`Career::voyage_ended`].
pub fn unload(content: &Content, haul: &Haul) -> HaulReport {
    let mut r = HaulReport::default();
    for (item, &n) in &haul.items {
        if n == 0 {
            continue;
        }
        match item {
            HaulItem::Part { part, material } => {
                let count = r.parts.entry(part.clone()).or_default();
                *count = count.saturating_add(n);
                let Some(p) = content.parts.get(part) else {
                    continue;
                };
                if p.hull {
                    let spec = material
                        .as_deref()
                        .and_then(|m| content.materials.get(m))
                        .and_then(|m| m.salvage.as_ref());
                    if let Some(s) = spec {
                        add_data(&mut r.data, &s.family, s.data * p.area() * n as f32);
                    }
                } else if let Some(s) = &p.salvage {
                    add_data(&mut r.data, &s.family, s.data * n as f32);
                    for (m, &a) in &s.materials {
                        add_materials(&mut r.materials, m, a.saturating_mul(n));
                    }
                }
            }
            HaulItem::Fragment { part } => {
                let count = r.fragments.entry(part.clone()).or_default();
                *count = count.saturating_add(n);
                if let Some(s) = content.parts.get(part).and_then(|p| p.salvage.as_ref()) {
                    for (m, &a) in &s.fragment {
                        add_materials(&mut r.materials, m, a.saturating_mul(n));
                    }
                }
            }
        }
    }
    r
}

/// Is the fleet (the flagship and the escorts) voyage-ready with these unlocks and this mind capacity? Empty:
/// yes. Entries, in order: per ship (flagship first) a frame too big for every unlocked frame, then the locked
/// parts and materials it uses (each once, in placement order); then too many logic cores for the mind; then the
/// warning that no ship has cargo room ([`Readiness::blocks`] is false only for that). The drydock and the map show
/// the first entry. Flight-readiness (power, thrust: the design report) is the sim's question, not this one.
pub fn voyage_ready(
    content: &Content,
    unlocks: &Unlocks,
    mind_capacity: u32,
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
