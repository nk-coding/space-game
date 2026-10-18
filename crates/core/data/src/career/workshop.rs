//! The workshop in the career (GDR-0108): the parts in stock that ships are fitted from, the haul's intact parts that
//! wait to be kept or dismantled (in the voyage's report; what it leaves is kept when it is dismissed), fabricating
//! parts from materials.
//!
//! Fitting takes nothing: a fleet launches when the stock holds every part its ships carry ([`voyage_ready`]), the
//! parts come back to stock when a ship comes home, and a lost ship's bonded parts re-form at home (GDR-0029). So the
//! stock grows by keeping intact salvage and by fabricating, and shrinks only when the player dismantles stock
//! parts the fleet does not carry ([`Career::dismantle_stock`]: the same share of the recipe as a hauled part, but no
//! research data, since nothing in the workshop's own stock is new to study).

use super::*;

/// The workshop's suggestion for one part of the intake ([`Career::suggested`]): how many to keep and how many to
/// dismantle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sort {
    pub keep: u32,
    pub dismantle: u32,
}

/// The non-hull parts a fleet carries, summed over its ships, by part id.
pub fn fleet_parts<'b>(content: &Content, ships: impl IntoIterator<Item = &'b Blueprint>) -> BTreeMap<String, u32> {
    let mut out: BTreeMap<String, u32> = BTreeMap::new();
    for ship in ships {
        for (part, n) in content.ship_parts(ship) {
            let x = out.entry(part).or_default();
            *x = x.saturating_add(n);
        }
    }
    out
}

/// What dismantling `n` of `part` gives (pure): its salvage block's research data, and the `share` of its recipe's
/// materials, rounded down over the `n` parts together. A part without a block or recipe gives that much less.
pub fn dismantle_yield(content: &Content, part: &str, n: u32, share: f32) -> HaulReport {
    let mut r = HaulReport::default();
    let Some(p) = content.parts.get(part) else {
        return r;
    };
    if let Some(s) = &p.salvage {
        add_data(&mut r.data, &s.family, s.data * n as f32);
    }
    if let Some(recipe) = &p.recipe {
        for (m, &a) in &recipe.materials {
            add_materials(&mut r.materials, m, (share * a as f32 * n as f32).round() as u32);
        }
    }
    r
}

impl Career {
    /// The content's workshop rules, or [`Refusal::NoWorkshop`].
    fn rules(content: &Content) -> Result<&WorkshopRules, Refusal> {
        content.workshop().map(|(_, r)| r).ok_or(Refusal::NoWorkshop)
    }

    /// Takes `n` of `part` out of the intake, or refuses with [`Refusal::NotInIntake`].
    fn take_intake(&mut self, part: &str, n: u32) -> Result<(), Refusal> {
        let have = self.intake.get(part).copied().unwrap_or(0);
        if n == 0 || have < n {
            return Err(Refusal::NotInIntake(part.to_string()));
        }
        if have == n {
            self.intake.remove(part);
        } else {
            self.intake.insert(part.to_string(), have - n);
        }
        Ok(())
    }

    /// Keeps `n` of the intake's `part`: into stock.
    pub fn keep(&mut self, part: &str, n: u32) -> Result<(), Refusal> {
        self.take_intake(part, n)?;
        add_materials(&mut self.stock, part, n);
        Ok(())
    }

    /// Keeps the whole intake (into stock) and returns how many parts that was: what the voyage's report leaves
    /// unsorted when it is dismissed goes to the parts list (Niklas, 18 Oct 2026).
    pub fn keep_intake(&mut self) -> u32 {
        let mut kept = 0u32;
        for (part, n) in std::mem::take(&mut self.intake) {
            add_materials(&mut self.stock, &part, n);
            kept = kept.saturating_add(n);
        }
        kept
    }

    /// Dismantles `n` of the intake's `part` into research data and the workshop's `dismantle` share of its
    /// recipe ([`dismantle_yield`]), and returns what it gave.
    pub fn dismantle(&mut self, content: &Content, part: &str, n: u32) -> Result<HaulReport, Refusal> {
        let share = Self::rules(content)?.dismantle;
        self.take_intake(part, n)?;
        let got = dismantle_yield(content, part, n, share);
        self.credit(&got);
        Ok(got)
    }

    /// How many of `part` the stock holds beyond what the fleet carries (`need`, by part id): what
    /// [`Career::dismantle_stock`] allows.
    pub fn spare(&self, part: &str, need: &BTreeMap<String, u32>) -> u32 {
        self.stock.get(part).copied().unwrap_or(0).saturating_sub(need.get(part).copied().unwrap_or(0))
    }

    /// Dismantles `n` of the stock's `part` for the workshop's `dismantle` share of its recipe (materials only: a
    /// stock part gives no research data) and returns what it gave. Refuses a part without a recipe (it would give
    /// nothing), more than the stock holds, and anything that would leave the stock short of what the fleet carries
    /// (`need`, by part id: [`Refusal::FleetNeeds`]).
    pub fn dismantle_stock(&mut self, content: &Content, part: &str, n: u32, need: &BTreeMap<String, u32>) -> Result<HaulReport, Refusal> {
        let share = Self::rules(content)?.dismantle;
        if content.parts.get(part).is_none_or(|p| p.recipe.is_none()) {
            return Err(Refusal::NoRecipe(part.to_string()));
        }
        let have = self.stock.get(part).copied().unwrap_or(0);
        if n == 0 || have < n {
            return Err(Refusal::NotInStock(part.to_string()));
        }
        let fleet = need.get(part).copied().unwrap_or(0);
        if have - n < fleet {
            return Err(Refusal::FleetNeeds { part: part.to_string(), need: fleet, stock: have });
        }
        if have == n {
            self.stock.remove(part);
        } else {
            self.stock.insert(part.to_string(), have - n);
        }
        let got = HaulReport { materials: dismantle_yield(content, part, n, share).materials, ..HaulReport::default() };
        self.credit(&got);
        Ok(got)
    }

    /// Adds a report's data and materials to the stores.
    pub(super) fn credit(&mut self, r: &HaulReport) {
        for (family, &x) in &r.data {
            add_data(&mut self.data, family, x);
        }
        for (material, &x) in &r.materials {
            add_materials(&mut self.materials, material, x);
        }
    }

    /// The workshop's suggestion for every part of the intake, in id order: keep while the stock holds fewer than
    /// the rules' `keep` or than `need` asks for (the fleet's parts), dismantle the rest.
    pub fn suggested(&self, content: &Content, need: &BTreeMap<String, u32>) -> BTreeMap<String, Sort> {
        let keep = content.workshop().map_or(0, |(_, r)| r.keep);
        self.intake
            .iter()
            .map(|(part, &n)| {
                let want = keep.max(need.get(part).copied().unwrap_or(0));
                let k = want.saturating_sub(self.stock.get(part).copied().unwrap_or(0)).min(n);
                (part.clone(), Sort { keep: k, dismantle: n - k })
            })
            .collect()
    }

    /// Sorts the whole intake as [`Career::suggested`] says; returns what the dismantled parts gave.
    pub fn sort_suggested(&mut self, content: &Content, need: &BTreeMap<String, u32>) -> Result<HaulReport, Refusal> {
        let share = Self::rules(content)?.dismantle;
        let mut got = HaulReport::default();
        for (part, sort) in self.suggested(content, need) {
            if sort.keep > 0 {
                self.keep(&part, sort.keep)?;
            }
            if sort.dismantle > 0 {
                self.take_intake(&part, sort.dismantle)?;
                let y = dismantle_yield(content, &part, sort.dismantle, share);
                for (f, &x) in &y.data {
                    add_data(&mut got.data, f, x);
                }
                for (m, &x) in &y.materials {
                    add_materials(&mut got.materials, m, x);
                }
            }
        }
        self.credit(&got);
        Ok(got)
    }

    /// Can the workshop fabricate `n` of `part` now? Refusals in order: an unknown part, one not unlocked, one
    /// without a recipe, a workshop upgrade not open, then every material the store can't meet (in id order).
    pub fn can_fabricate(&self, content: &Content, part: &str, n: u32) -> Result<(), Refusal> {
        let Some(p) = content.parts.get(part) else {
            return Err(Refusal::UnknownPart(part.to_string()));
        };
        let unlocks = self.unlocks(content);
        if !unlocks.has_part(part) {
            return Err(Refusal::LockedPart(part.to_string()));
        }
        let Some(recipe) = &p.recipe else {
            return Err(Refusal::NoRecipe(part.to_string()));
        };
        if let Some(f) = recipe.needs.as_ref().filter(|f| !unlocks.has_facility(f)) {
            return Err(Refusal::NeedsFacility(f.clone()));
        }
        let short: Vec<Shortfall> = recipe
            .materials
            .iter()
            .filter_map(|(m, &a)| {
                let (have, cost) = (self.materials.get(m).copied().unwrap_or(0), a.saturating_mul(n));
                (have < cost).then(|| Shortfall::Material { material: m.clone(), have, cost })
            })
            .collect();
        if short.is_empty() { Ok(()) } else { Err(Refusal::CannotPay(short)) }
    }

    /// The most of `part` the workshop can fabricate now: 0 if [`Career::can_fabricate`] refuses one, else what the
    /// scarcest material of its recipe pays for (`u32::MAX` for a recipe without materials).
    pub fn fabricable(&self, content: &Content, part: &str) -> u32 {
        if self.can_fabricate(content, part, 1).is_err() {
            return 0;
        }
        content.parts.get(part).and_then(|p| p.recipe.as_ref()).map_or(0, |r| {
            r.materials.iter().map(|(m, &a)| self.materials.get(m).copied().unwrap_or(0) / a.max(1)).min().unwrap_or(u32::MAX)
        })
    }

    /// Fabricates `n` of `part` (see [`Career::can_fabricate`]): its recipe's materials are taken, the parts go into
    /// stock.
    pub fn fabricate(&mut self, content: &Content, part: &str, n: u32) -> Result<(), Refusal> {
        self.can_fabricate(content, part, n)?;
        if let Some(recipe) = content.parts.get(part).and_then(|p| p.recipe.as_ref()) {
            for (m, &a) in &recipe.materials {
                let left = self.materials.get(m).copied().unwrap_or(0) - a.saturating_mul(n);
                if left > 0 {
                    self.materials.insert(m.clone(), left);
                } else {
                    self.materials.remove(m);
                }
            }
        }
        add_materials(&mut self.stock, part, n);
        Ok(())
    }
}
